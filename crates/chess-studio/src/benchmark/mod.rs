//! Modo benchmark (`--benchmark`): roteiro de câmera determinístico + coleta de métricas.
//!
//! O app roda por `--minutes` (padrão 5), grava `benchmark.json`, `frame_times.csv` e
//! `samples.csv` no diretório de saída, imprime um resumo e encerra sozinho.
//!
//! Engine ativa (`--engine ativa`, C10, PERF-04): a medição é dividida em fases iguais, a
//! primeira sem análise e uma por Engine (Stockfish e, se houver, Lc0) com o painel Análise
//! ligado de verdade, e o relatório traz o impacto de cada uma em FPS, frame time, CPU, VRAM
//! e uso da GPU (`phases.rs`). `--load maximo|equilibrado|leve` escolhe o governador de carga,
//! `--engine-path`, `--network` e `--backend` indicam as Engines.
//!
//! Limitações conhecidas (documentadas no PR da issue #23):
//! - VRAM/utilização da GPU vêm do `nvidia-smi` (sem dependência obrigatória) e medem a
//!   GPU inteira, não só este processo; sem `nvidia-smi` os campos ficam nulos;
//! - CPU do processo: 100% = um núcleo lógico inteiro (pode passar de 100%).

mod config;
mod phases;
mod report;
mod route;
mod stats;

pub use config::{BenchmarkConfig, parse_args};

use crate::analysis::Analysis;
use crate::camera::{CameraRig, CameraSet, MainCamera};
use bevy::prelude::*;
use bevy::render::{ExtractSchedule, RenderApp, renderer::RenderAdapterInfo, view::Msaa};
use bevy::window::PrimaryWindow;
use report::{AnalysisReport, GpuInfo, MachineState, Report, RunInfo, Sample, SystemInfo};
use std::path::PathBuf;
use std::process::Command;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};
use sysinfo::{Pid, ProcessRefreshKind, ProcessesToUpdate, System};

/// Os primeiros segundos (compilação de shaders, abertura da janela) ficam fora da estatística.
const WARMUP_S: f64 = 3.0;
const NVIDIA_POLL: Duration = Duration::from_secs(3);
/// Com Engine ativa as fases são curtas: a GPU é lida a cada segundo.
const NVIDIA_POLL_ACTIVE: Duration = Duration::from_secs(1);
const MIB: f64 = 1024.0 * 1024.0;

#[derive(Debug, Clone, Copy)]
struct NvidiaSample {
    used_mib: f64,
    total_mib: f64,
    util_percent: f64,
}

#[derive(Resource)]
struct Bench {
    cfg: BenchmarkConfig,
    vsync: bool,
    started_wall: SystemTime,
    start: Option<Instant>,
    /// Frame times (ms) e instantes (s) após o aquecimento; capacidade reservada de início.
    frame_ms: Vec<f32>,
    frame_t: Vec<f32>,
    samples: Vec<Sample>,
    next_sample_s: f64,
    frames_in_second: u32,
    sys: System,
    pid: Pid,
    gpu: Arc<Mutex<Option<GpuInfo>>>,
    nvidia: Arc<Mutex<Option<NvidiaSample>>>,
    nvidia_total_mib: Option<f64>,
    notes: Vec<String>,
    /// `--engine ativa`: fases (a primeira sem análise) e o backend visto em cada uma.
    phases: Vec<phases::PhaseSpec>,
    phase_now: Option<usize>,
    phase_backend: Vec<Option<String>>,
    load_avg_start: Option<f64>,
    vram_before_mib: Option<f64>,
    /// Tempo até a cena ficar pronta (B22), fora da medição.
    scene_load_s: Option<f64>,
}

/// Engines que analisam no benchmark: as indicadas por `--engine-path` ou as do ambiente.
fn analysis_candidates(cfg: &BenchmarkConfig, notes: &mut Vec<String>) -> Vec<phases::Candidate> {
    let mut paths = cfg.engine_paths.clone();
    if paths.is_empty() {
        paths.extend(crate::engine::resolve_engine());
        paths.extend(
            std::env::var_os(chess_uci::LC0_ENV)
                .map(PathBuf::from)
                .filter(|p| p.is_file()),
        );
    }
    let network = cfg.network.clone().or_else(|| {
        std::env::var_os("CHESS_STUDIO_LC0_NET")
            .map(PathBuf::from)
            .filter(|p| p.is_file())
    });
    let mut out = Vec::new();
    for path in paths {
        let stem = path
            .file_stem()
            .map_or_else(String::new, |s| s.to_string_lossy().into_owned());
        let mut options = Vec::new();
        let name = if stem.to_lowercase().starts_with("lc0") {
            let Some(net) = &network else {
                notes.push(format!(
                    "{}: Lc0 sem rede neural (use --network ou CHESS_STUDIO_LC0_NET); fase omitida.",
                    path.display()
                ));
                continue;
            };
            options.push(("WeightsFile".to_owned(), net.to_string_lossy().into_owned()));
            if let Some(backend) = &cfg.backend {
                options.push(("Backend".to_owned(), backend.clone()));
            }
            "Lc0".to_owned()
        } else {
            let mut name = stem;
            if let Some(first) = name.get_mut(..1) {
                first.make_ascii_uppercase();
            }
            name
        };
        if !path.is_file() {
            notes.push(format!(
                "{}: Engine não encontrada; fase omitida.",
                path.display()
            ));
            continue;
        }
        out.push(phases::Candidate {
            name,
            path,
            options,
        });
    }
    out
}

/// Uma leitura única do `nvidia-smi` (VRAM em uso antes de o app abrir a janela).
fn nvidia_now() -> Option<NvidiaSample> {
    let out = Command::new("nvidia-smi")
        .args([
            "--query-gpu=memory.used,memory.total,utilization.gpu",
            "--format=csv,noheader,nounits",
        ])
        .output()
        .ok()
        .filter(|o| o.status.success())?;
    parse_nvidia(&String::from_utf8_lossy(&out.stdout))
}

pub struct BenchmarkPlugin {
    pub config: BenchmarkConfig,
    pub vsync: bool,
}

impl Plugin for BenchmarkPlugin {
    fn build(&self, app: &mut App) {
        let cfg = self.config.clone();
        let duration_s = cfg.minutes * 60.0;
        // 1000 FPS de teto razoável; evita realocar no laço de frames.
        let cap = (duration_s * 1000.0) as usize + 1024;
        let gpu = Arc::new(Mutex::new(None));
        let nvidia = Arc::new(Mutex::new(None));
        let active = cfg.engine == config::EngineMode::Active;
        spawn_nvidia_poller(
            nvidia.clone(),
            if active {
                NVIDIA_POLL_ACTIVE
            } else {
                NVIDIA_POLL
            },
        );
        let mut notes = Vec::new();
        let phases = if active {
            phases::plan(WARMUP_S, duration_s, analysis_candidates(&cfg, &mut notes))
        } else {
            Vec::new()
        };
        let phase_backend = vec![None; phases.len()];
        let vram_before_mib = active.then(nvidia_now).flatten().map(|n| n.used_mib);

        if let Some(render_app) = app.get_sub_app_mut(RenderApp) {
            let slot = gpu.clone();
            render_app.add_systems(
                ExtractSchedule,
                move |info: Option<Res<RenderAdapterInfo>>, mut done: Local<bool>| {
                    if *done {
                        return;
                    }
                    if let Some(info) = info {
                        let i: &wgpu_types::AdapterInfo = &info;
                        if let Ok(mut guard) = slot.lock() {
                            *guard = Some(GpuInfo {
                                name: i.name.clone(),
                                vendor_id: i.vendor,
                                device_id: i.device,
                                device_type: format!("{:?}", i.device_type),
                                driver: i.driver.clone(),
                                driver_info: i.driver_info.clone(),
                                backend: format!("{:?}", i.backend),
                                vram_source: None,
                                vram_total_mib: None,
                            });
                        }
                        *done = true;
                    }
                },
            );
        }

        let mut sys = System::new();
        sys.refresh_memory();
        sys.refresh_cpu_all();
        app.insert_resource(Bench {
            cfg,
            vsync: self.vsync,
            started_wall: SystemTime::now(),
            start: None,
            frame_ms: Vec::with_capacity(cap),
            frame_t: Vec::with_capacity(cap),
            samples: Vec::with_capacity(duration_s as usize + 16),
            next_sample_s: WARMUP_S + 1.0,
            frames_in_second: 0,
            sys,
            pid: sysinfo::get_current_pid().unwrap_or(Pid::from_u32(std::process::id())),
            gpu,
            nvidia,
            nvidia_total_mib: None,
            notes,
            phases,
            phase_now: None,
            phase_backend,
            load_avg_start: System::load_average().one.into(),
            vram_before_mib,
            scene_load_s: None,
        })
        .add_systems(PostStartup, apply_preset)
        .add_systems(Update, drive_camera.before(CameraSet))
        .add_systems(Update, drive_phases)
        .add_systems(Last, collect);
    }
}

/// Lê `nvidia-smi` a cada poucos segundos numa thread própria; ausente = nada é gravado.
fn spawn_nvidia_poller(slot: Arc<Mutex<Option<NvidiaSample>>>, every: Duration) {
    let _ = std::thread::Builder::new()
        .name("benchmark-nvidia-smi".into())
        .spawn(move || {
            loop {
                let out = Command::new("nvidia-smi")
                    .args([
                        "--query-gpu=memory.used,memory.total,utilization.gpu",
                        "--format=csv,noheader,nounits",
                    ])
                    .output();
                let Ok(out) = out else { return }; // não instalado: encerra a thread
                if !out.status.success() {
                    return;
                }
                if let Some(sample) = parse_nvidia(&String::from_utf8_lossy(&out.stdout))
                    && let Ok(mut guard) = slot.lock()
                {
                    *guard = Some(sample);
                }
                std::thread::sleep(every);
            }
        });
}

fn parse_nvidia(text: &str) -> Option<NvidiaSample> {
    let mut fields = text
        .lines()
        .next()?
        .split(',')
        .map(|f| f.trim().parse::<f64>());
    Some(NvidiaSample {
        used_mib: fields.next()?.ok()?,
        total_mib: fields.next()?.ok()?,
        util_percent: fields.next()?.ok()?,
    })
}

/// Preset `economico`: sem MSAA e sem sombras. `equilibrado` mantém os padrões da cena.
fn apply_preset(
    bench: Res<Bench>,
    mut commands: Commands,
    cameras: Query<Entity, With<MainCamera>>,
    mut lights: Query<&mut DirectionalLight>,
) {
    if bench.cfg.preset == config::Preset::Economy {
        for cam in &cameras {
            commands.entity(cam).insert(Msaa::Off);
        }
        for mut light in &mut lights {
            light.shadow_maps_enabled = false;
        }
    }
}

/// Liga e desliga a análise real conforme a fase (só com `--engine ativa`).
fn drive_phases(mut bench: ResMut<Bench>, mut analysis: ResMut<Analysis>) {
    if bench.phases.is_empty() {
        return;
    }
    let Some(start) = bench.start else { return };
    let t = start.elapsed().as_secs_f64();
    let now = phases::index_at(&bench.phases, t);
    if now != bench.phase_now {
        bench.phase_now = now;
        match now.and_then(|i| bench.phases[i].engine.clone()) {
            Some(engine) => {
                analysis.bench_use(engine.path, engine.options, bench.cfg.load);
                analysis.set_enabled(true);
            }
            None => analysis.set_enabled(false),
        }
    }
    if let Some(i) = now
        && let Some(backend) = analysis.backend_active()
    {
        bench.phase_backend[i] = Some(backend.to_owned());
    }
}

/// Câmera = função pura do tempo decorrido (mesmo percurso em qualquer FPS).
fn drive_camera(bench: Res<Bench>, mut rig: ResMut<CameraRig>) {
    let Some(start) = bench.start else { return };
    let t = start.elapsed().as_secs_f32();
    rig.pose = route::pose_at(t);
    rig.view = Some(route::view_at(t));
}

#[allow(clippy::too_many_arguments)]
fn collect(
    mut bench: ResMut<Bench>,
    time: Res<Time<Real>>,
    window: Query<&Window, With<PrimaryWindow>>,
    analysis: Res<Analysis>,
    mut exit: MessageWriter<AppExit>,
    ready: Res<crate::ready::SceneReady>,
) {
    // B22: a medição só começa com a cena pronta; o carregamento é registrado à parte.
    if let Some(err) = ready.timeout_error() {
        eprintln!("Benchmark abortado: {err}");
        exit.write(AppExit::from_code(2));
        return;
    }
    if !ready.is_ready() {
        return;
    }
    if bench.scene_load_s.is_none() {
        bench.scene_load_s = ready.load_time().map(|d| d.as_secs_f64());
    }
    let now = Instant::now();
    let start = *bench.start.get_or_insert(now);
    let t = now.duration_since(start).as_secs_f64();
    let dt_ms = time.delta_secs_f64() * 1000.0;

    if t >= WARMUP_S && dt_ms > 0.0 {
        let b = &mut *bench;
        b.frame_ms.push(dt_ms as f32);
        b.frame_t.push(t as f32);
        b.frames_in_second += 1;
    }

    if t >= bench.next_sample_s {
        take_sample(&mut bench, t, &analysis);
    }

    if t >= bench.cfg.minutes * 60.0 {
        let (w, h) = window
            .single()
            .map_or((0, 0), |w| (w.physical_width(), w.physical_height()));
        match finish(&mut bench, w, h) {
            Ok(dir) => println!("Arquivos gravados em {}", dir.display()),
            Err(e) => eprintln!("Falha ao gravar o benchmark: {e}"),
        }
        exit.write(AppExit::Success);
    }
}

fn take_sample(bench: &mut Bench, t: f64, analysis: &Analysis) {
    let pid = bench.pid;
    bench.sys.refresh_processes_specifics(
        ProcessesToUpdate::Some(&[pid]),
        true,
        ProcessRefreshKind::nothing().with_cpu().with_memory(),
    );
    let (cpu, ram) = bench.sys.process(pid).map_or((0.0, 0.0), |p| {
        (f64::from(p.cpu_usage()), p.memory() as f64 / MIB)
    });
    let nv = bench.nvidia.lock().ok().and_then(|g| *g);
    if let Some(nv) = nv {
        bench.nvidia_total_mib = Some(nv.total_mib);
    }
    let fps = f64::from(bench.frames_in_second) / (t - (bench.next_sample_s - 1.0)).max(1e-3);
    bench.samples.push(Sample {
        t_s: t,
        fps,
        cpu_percent: cpu,
        ram_mib: ram,
        vram_used_mib: nv.map(|n| n.used_mib),
        gpu_util_percent: nv.map(|n| n.util_percent),
        analysis_depth: analysis.latest_metrics().map(|m| m.0),
        analysis_nps: analysis.latest_metrics().and_then(|m| m.1),
        load_level: analysis.is_active().then(|| analysis.load_level_index()),
    });
    bench.frames_in_second = 0;
    bench.next_sample_s = t + 1.0;
}

fn finish(bench: &mut Bench, width: u32, height: u32) -> std::io::Result<PathBuf> {
    let unix = bench
        .started_wall
        .duration_since(UNIX_EPOCH)
        .map_or(0, |d| d.as_secs());
    let dir = bench
        .cfg
        .out
        .clone()
        .unwrap_or_else(|| PathBuf::from("benchmarks/out").join(report::compact_utc(unix)));
    std::fs::create_dir_all(&dir)?;

    let frame_ms: Vec<f64> = bench.frame_ms.iter().map(|v| f64::from(*v)).collect();
    let col = |f: fn(&Sample) -> f64| bench.samples.iter().map(f).collect::<Vec<_>>();
    let opt_col =
        |f: fn(&Sample) -> Option<f64>| bench.samples.iter().filter_map(f).collect::<Vec<_>>();
    let mut gpu = bench.gpu.lock().ok().and_then(|g| g.clone());
    let vram = opt_col(|s| s.vram_used_mib);
    if let Some(g) = gpu.as_mut()
        && !vram.is_empty()
    {
        g.vram_source = Some("nvidia-smi".into());
        g.vram_total_mib = bench.nvidia_total_mib;
    }
    let mut notes = std::mem::take(&mut bench.notes);
    if vram.is_empty() {
        notes.push("VRAM indisponível (nvidia-smi ausente ou sem GPU NVIDIA).".into());
    }
    let analysis = analysis_report(bench);
    if gpu.is_none() {
        notes.push("Informações do adaptador wgpu não foram capturadas.".into());
    }

    let mut sys = System::new();
    sys.refresh_memory();
    sys.refresh_cpu_all();
    let report = Report {
        schema_version: 1,
        run: RunInfo {
            app_version: env!("CARGO_PKG_VERSION").into(),
            git_commit: env!("CHESS_STUDIO_GIT_COMMIT").into(),
            started_utc: report::iso_utc(unix),
            requested_minutes: bench.cfg.minutes,
            measured_seconds: frame_ms.iter().sum::<f64>() / 1000.0,
            warmup_seconds: WARMUP_S,
            scene_load_seconds: bench.scene_load_s,
            preset: bench.cfg.preset.label().into(),
            engine: bench.cfg.engine.label().into(),
            engine_integrated: bench.cfg.engine == config::EngineMode::Active,
            present_mode: if bench.vsync { "vsync" } else { "sem-vsync" }.into(),
            window_width: width,
            window_height: height,
            route: format!(
                "órbita de {} s por volta; Vista a cada {} s (Padrão, Oblíqua, Superior, Invertida)",
                route::ORBIT_PERIOD_S,
                route::VIEW_PERIOD_S
            ),
        },
        system: SystemInfo {
            os: System::long_os_version().unwrap_or_else(|| "desconhecido".into()),
            kernel: System::kernel_version().unwrap_or_else(|| "desconhecido".into()),
            cpu_brand: sys
                .cpus()
                .first()
                .map_or_else(|| "desconhecido".into(), |c| c.brand().to_owned()),
            cpu_logical_cores: sys.cpus().len(),
            ram_total_mib: sys.total_memory() as f64 / MIB,
        },
        gpu,
        frame_time: stats::summarize(&frame_ms),
        process_ram_mib: stats::series_stats(&col(|s| s.ram_mib)),
        process_cpu_percent: stats::series_stats(&col(|s| s.cpu_percent)),
        gpu_vram_used_mib: stats::series_stats(&vram),
        gpu_utilization_percent: stats::series_stats(&opt_col(|s| s.gpu_util_percent)),
        analysis,
        notes,
    };

    let json = serde_json::to_string_pretty(&report).map_err(std::io::Error::other)?;
    std::fs::write(dir.join("benchmark.json"), json)?;
    std::fs::write(
        dir.join("frame_times.csv"),
        report::frame_times_csv(&bench.frame_t, &bench.frame_ms),
    )?;
    std::fs::write(dir.join("samples.csv"), report::samples_csv(&bench.samples))?;
    println!("{}", report::summary_text(&report));
    Ok(dir)
}

/// Relatório por fase (só com `--engine ativa`).
fn analysis_report(bench: &Bench) -> Option<AnalysisReport> {
    if bench.phases.is_empty() {
        return None;
    }
    let reports: Vec<phases::PhaseReport> = bench
        .phases
        .iter()
        .enumerate()
        .map(|(i, spec)| {
            phases::summarize(
                spec,
                &bench.frame_t,
                &bench.frame_ms,
                &bench.samples,
                bench.phase_backend[i].clone(),
            )
        })
        .collect();
    let impact = reports
        .iter()
        .skip(1)
        .map(|r| phases::impact(&reports[0], r))
        .collect();
    let target = crate::load_control::target_for(crate::presentation::GraphicsQuality::default());
    Some(AnalysisReport {
        load_mode: match bench.cfg.load {
            chess_uci::LoadMode::Max => "maximo",
            chess_uci::LoadMode::Balanced => "equilibrado",
            chess_uci::LoadMode::Light => "leve",
        }
        .into(),
        target_fps: target.fps(),
        phases: reports,
        impact,
        machine: MachineState {
            load_average_1m_start: bench.load_avg_start,
            load_average_1m_end: Some(System::load_average().one),
            cpu_logical_cores: bench.sys.cpus().len().max(1),
            vram_used_before_mib: bench.vram_before_mib,
        },
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_nvidia_smi_csv_line() {
        let s = parse_nvidia("1234, 16376, 7\n").expect("linha válida");
        assert_eq!(
            (s.used_mib, s.total_mib, s.util_percent),
            (1234.0, 16376.0, 7.0)
        );
        assert!(parse_nvidia("").is_none());
        assert!(parse_nvidia("N/A, N/A, N/A").is_none());
    }
}
