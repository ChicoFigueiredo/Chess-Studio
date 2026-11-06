//! Modo benchmark (`--benchmark`): roteiro de câmera determinístico + coleta de métricas.
//!
//! O app roda por `--minutes` (padrão 5), grava `benchmark.json`, `frame_times.csv` e
//! `samples.csv` no diretório de saída, imprime um resumo e encerra sozinho.
//!
//! Limitações conhecidas (documentadas no PR da issue #23):
//! - `--engine ativa` só registra o campo; a Engine real (UCI) chega no B9;
//! - VRAM/utilização da GPU vêm do `nvidia-smi` (sem dependência obrigatória) e medem a
//!   GPU inteira, não só este processo; sem `nvidia-smi` os campos ficam nulos;
//! - CPU do processo: 100% = um núcleo lógico inteiro (pode passar de 100%).

mod config;
mod report;
mod route;
mod stats;

pub use config::{BenchmarkConfig, parse_args};

use crate::camera::{CameraRig, CameraSet, MainCamera};
use bevy::prelude::*;
use bevy::render::{ExtractSchedule, RenderApp, renderer::RenderAdapterInfo, view::Msaa};
use bevy::window::PrimaryWindow;
use report::{GpuInfo, Report, RunInfo, Sample, SystemInfo};
use std::path::PathBuf;
use std::process::Command;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};
use sysinfo::{Pid, ProcessRefreshKind, ProcessesToUpdate, System};

/// Os primeiros segundos (compilação de shaders, abertura da janela) ficam fora da estatística.
const WARMUP_S: f64 = 3.0;
const NVIDIA_POLL: Duration = Duration::from_secs(3);
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
        spawn_nvidia_poller(nvidia.clone());

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
            notes: Vec::new(),
        })
        .add_systems(PostStartup, apply_preset)
        .add_systems(Update, drive_camera.before(CameraSet))
        .add_systems(Last, collect);
    }
}

/// Lê `nvidia-smi` a cada poucos segundos numa thread própria; ausente = nada é gravado.
fn spawn_nvidia_poller(slot: Arc<Mutex<Option<NvidiaSample>>>) {
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
                std::thread::sleep(NVIDIA_POLL);
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
    mut exit: MessageWriter<AppExit>,
) {
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
        take_sample(&mut bench, t);
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

fn take_sample(bench: &mut Bench, t: f64) {
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
    if bench.cfg.engine == config::EngineMode::Active {
        notes.push("Engine ativa apenas registrada: a Engine real chega no B9.".into());
    }
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
            preset: bench.cfg.preset.label().into(),
            engine: bench.cfg.engine.label().into(),
            engine_integrated: false,
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
