//! Estrutura do relatório (JSON), séries (CSV), resumo legível e data/hora UTC.

use super::stats::{FrameStats, SeriesStats};
use serde::Serialize;
use std::fmt::Write as _;

/// Relatório completo gravado em `benchmark.json`.
#[derive(Debug, Serialize)]
pub struct Report {
    /// Versão do formato; incrementar ao mudar campos.
    pub schema_version: u32,
    pub run: RunInfo,
    pub system: SystemInfo,
    pub gpu: Option<GpuInfo>,
    pub frame_time: Option<FrameStats>,
    pub process_ram_mib: Option<SeriesStats>,
    pub process_cpu_percent: Option<SeriesStats>,
    pub gpu_vram_used_mib: Option<SeriesStats>,
    pub gpu_utilization_percent: Option<SeriesStats>,
    /// Só com `--engine ativa`: fases com e sem análise e o impacto de cada uma (PERF-04).
    pub analysis: Option<AnalysisReport>,
    pub notes: Vec<String>,
}

/// Comparação com e sem a Engine analisando (PERF-04).
#[derive(Debug, Serialize)]
pub struct AnalysisReport {
    /// Modo do governador de carga (`maximo`, `equilibrado`, `leve`).
    pub load_mode: String,
    /// Meta de FPS do governador.
    pub target_fps: u32,
    pub phases: Vec<super::phases::PhaseReport>,
    /// Cada fase de análise contra a de referência.
    pub impact: Vec<super::phases::Impact>,
    /// A máquina estava ocupada? (para interpretar os números)
    pub machine: MachineState,
}

/// Estado da máquina durante a medição.
#[derive(Debug, Serialize)]
pub struct MachineState {
    /// Média de carga de 1 min no início e no fim (Linux/macOS); 1,0 por núcleo = saturada.
    pub load_average_1m_start: Option<f64>,
    pub load_average_1m_end: Option<f64>,
    pub cpu_logical_cores: usize,
    /// VRAM já usada por outros processos antes do app abrir a janela.
    pub vram_used_before_mib: Option<f64>,
}

#[derive(Debug, Serialize)]
pub struct RunInfo {
    pub app_version: String,
    pub git_commit: String,
    pub started_utc: String,
    pub requested_minutes: f64,
    pub measured_seconds: f64,
    pub warmup_seconds: f64,
    pub preset: String,
    /// `ativa` ou `ociosa`. Hoje só registra o campo (a Engine real chega no B9).
    pub engine: String,
    pub engine_integrated: bool,
    pub present_mode: String,
    pub window_width: u32,
    pub window_height: u32,
    pub route: String,
}

#[derive(Debug, Serialize)]
pub struct SystemInfo {
    pub os: String,
    pub kernel: String,
    pub cpu_brand: String,
    pub cpu_logical_cores: usize,
    pub ram_total_mib: f64,
}

#[derive(Debug, Clone, Serialize)]
pub struct GpuInfo {
    pub name: String,
    pub vendor_id: u32,
    pub device_id: u32,
    pub device_type: String,
    pub driver: String,
    pub driver_info: String,
    pub backend: String,
    /// Origem da VRAM/utilização: `nvidia-smi` quando disponível, senão nula.
    pub vram_source: Option<String>,
    pub vram_total_mib: Option<f64>,
}

/// Amostra por segundo (linha de `samples.csv`).
#[derive(Debug, Clone)]
pub struct Sample {
    pub t_s: f64,
    pub fps: f64,
    pub cpu_percent: f64,
    pub ram_mib: f64,
    pub vram_used_mib: Option<f64>,
    pub gpu_util_percent: Option<f64>,
    /// Análise ativa (`--engine ativa`): profundidade e nós/s mostrados, e o nível do
    /// governador de carga (0 = pleno … 4 = pausado).
    pub analysis_depth: Option<u32>,
    pub analysis_nps: Option<u64>,
    pub load_level: Option<u8>,
}

pub fn samples_csv(samples: &[Sample]) -> String {
    let mut out = String::from(
        "t_s,fps,cpu_percent,ram_mib,vram_used_mib,gpu_util_percent,analysis_depth,analysis_nps,load_level\n",
    );
    let opt = |v: Option<f64>| v.map_or_else(String::new, |v| format!("{v:.1}"));
    for s in samples {
        let _ = writeln!(
            out,
            "{:.1},{:.1},{:.1},{:.1},{},{},{},{},{}",
            s.t_s,
            s.fps,
            s.cpu_percent,
            s.ram_mib,
            opt(s.vram_used_mib),
            opt(s.gpu_util_percent),
            s.analysis_depth.map_or_else(String::new, |v| v.to_string()),
            s.analysis_nps.map_or_else(String::new, |v| v.to_string()),
            s.load_level.map_or_else(String::new, |v| v.to_string()),
        );
    }
    out
}

pub fn frame_times_csv(t_s: &[f32], frame_ms: &[f32]) -> String {
    let mut out = String::from("frame,t_s,frame_ms\n");
    for (i, (t, ms)) in t_s.iter().zip(frame_ms).enumerate() {
        let _ = writeln!(out, "{i},{t:.4},{ms:.4}");
    }
    out
}

/// Resumo legível impresso no fim da execução.
pub fn summary_text(r: &Report) -> String {
    let mut s = String::new();
    let _ = writeln!(s, "=== Benchmark Chess Studio ===");
    let _ = writeln!(
        s,
        "Versão {} ({}), {} UTC",
        r.run.app_version, r.run.git_commit, r.run.started_utc
    );
    let _ = writeln!(
        s,
        "{}x{} | preset {} | engine {} | present {} | {:.0} s medidos",
        r.run.window_width,
        r.run.window_height,
        r.run.preset,
        r.run.engine,
        r.run.present_mode,
        r.run.measured_seconds
    );
    let _ = writeln!(s, "SO: {} ({})", r.system.os, r.system.kernel);
    let _ = writeln!(
        s,
        "CPU: {} ({} threads), RAM total {:.0} MiB",
        r.system.cpu_brand, r.system.cpu_logical_cores, r.system.ram_total_mib
    );
    match &r.gpu {
        Some(g) => {
            let _ = writeln!(
                s,
                "GPU: {} [{}] driver {} {} ({})",
                g.name, g.device_type, g.driver, g.driver_info, g.backend
            );
        }
        None => {
            let _ = writeln!(s, "GPU: indisponível");
        }
    }
    if let Some(f) = &r.frame_time {
        let _ = writeln!(
            s,
            "FPS médio {:.1} | 1% low {:.1} | frames {}",
            f.fps_avg, f.fps_1pct_low, f.frames
        );
        let _ = writeln!(
            s,
            "Frame time (ms): p1 {:.2} | p5 {:.2} | p50 {:.2} | p95 {:.2} | p99 {:.2} | máx {:.2}",
            f.frame_ms_p1,
            f.frame_ms_p5,
            f.frame_ms_p50,
            f.frame_ms_p95,
            f.frame_ms_p99,
            f.frame_ms_max
        );
    } else {
        let _ = writeln!(s, "Frame time: sem amostras");
    }
    let line = |label: &str, v: &Option<SeriesStats>, unit: &str| match v {
        Some(v) => format!(
            "{label}: média {:.1}{unit} | máx {:.1}{unit}\n",
            v.avg, v.max
        ),
        None => format!("{label}: indisponível\n"),
    };
    s += &line("RAM do processo", &r.process_ram_mib, " MiB");
    s += &line("CPU do processo", &r.process_cpu_percent, "%");
    s += &line("VRAM usada (GPU inteira)", &r.gpu_vram_used_mib, " MiB");
    s += &line("Utilização da GPU", &r.gpu_utilization_percent, "%");
    for n in &r.notes {
        let _ = writeln!(s, "Nota: {n}");
    }
    s
}

/// Data/hora UTC ISO-8601 (`2026-11-06T12:30:00Z`) a partir de segundos Unix.
pub fn iso_utc(unix_secs: u64) -> String {
    let (y, m, d, hh, mm, ss) = civil(unix_secs);
    format!("{y:04}-{m:02}-{d:02}T{hh:02}:{mm:02}:{ss:02}Z")
}

/// Mesmo instante, formato seguro para nome de diretório (`20261106T123000Z`).
pub fn compact_utc(unix_secs: u64) -> String {
    let (y, m, d, hh, mm, ss) = civil(unix_secs);
    format!("{y:04}{m:02}{d:02}T{hh:02}{mm:02}{ss:02}Z")
}

/// Algoritmo `civil_from_days` (Howard Hinnant).
fn civil(unix_secs: u64) -> (i64, u32, u32, u32, u32, u32) {
    let days = (unix_secs / 86_400) as i64;
    let rem = unix_secs % 86_400;
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = (doy - (153 * mp + 2) / 5 + 1) as u32;
    let m = if mp < 10 { mp + 3 } else { mp - 9 } as u32;
    let y = yoe + era * 400 + i64::from(m <= 2);
    (
        y,
        m,
        d,
        (rem / 3600) as u32,
        (rem % 3600 / 60) as u32,
        (rem % 60) as u32,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn iso_utc_known_instants() {
        assert_eq!(iso_utc(0), "1970-01-01T00:00:00Z");
        assert_eq!(iso_utc(951_782_400), "2000-02-29T00:00:00Z"); // bissexto
        assert_eq!(iso_utc(1_793_968_200), "2026-11-06T12:30:00Z");
        assert_eq!(compact_utc(1_793_968_200), "20261106T123000Z");
    }

    #[test]
    fn samples_csv_leaves_missing_vram_empty() {
        let csv = samples_csv(&[
            Sample {
                t_s: 1.0,
                fps: 60.0,
                cpu_percent: 12.5,
                ram_mib: 300.0,
                vram_used_mib: None,
                gpu_util_percent: None,
                analysis_depth: None,
                analysis_nps: None,
                load_level: None,
            },
            Sample {
                t_s: 2.0,
                fps: 59.0,
                cpu_percent: 10.0,
                ram_mib: 301.0,
                vram_used_mib: Some(900.0),
                gpu_util_percent: Some(30.0),
                analysis_depth: Some(14),
                analysis_nps: Some(20_000),
                load_level: Some(1),
            },
        ]);
        let lines: Vec<&str> = csv.lines().collect();
        assert_eq!(
            lines[0],
            "t_s,fps,cpu_percent,ram_mib,vram_used_mib,gpu_util_percent,analysis_depth,analysis_nps,load_level"
        );
        assert_eq!(lines[1], "1.0,60.0,12.5,300.0,,,,,");
        assert_eq!(lines[2], "2.0,59.0,10.0,301.0,900.0,30.0,14,20000,1");
    }

    #[test]
    fn frame_times_csv_has_one_row_per_frame() {
        let csv = frame_times_csv(&[0.0, 0.0167], &[16.7, 16.6]);
        assert_eq!(csv.lines().count(), 3);
        assert!(csv.starts_with("frame,t_s,frame_ms\n0,0.0000,16.7000\n"));
    }
}
