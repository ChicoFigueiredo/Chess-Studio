//! Fases do benchmark com Engine ativa (`--engine ativa`, PERF-04, C10): a medição é dividida
//! em partes iguais, a primeira **sem** análise e uma por Engine **com** análise ligada, para
//! comparar FPS, frame time, CPU, VRAM e uso da GPU com e sem a carga da Engine. Parte pura
//! (sem Bevy): planejamento das fases e agregação das métricas de cada uma.

use std::collections::BTreeMap;
use std::path::PathBuf;

use serde::Serialize;

use super::report::Sample;
use super::stats::{self, FrameStats, SeriesStats};

/// Primeiros segundos de cada fase de análise (subida da Engine, carga da rede, o governador
/// ainda reagindo) ficam fora das estatísticas de quadro e de GPU; entram só na série bruta.
pub const SETTLE_S: f64 = 2.0;

/// Nomes dos níveis de redução, na ordem de `chess_uci::Level`.
pub const LEVEL_NAMES: [&str; 5] = ["pleno", "reduzido", "minimo", "intermitente", "pausado"];

/// Uma Engine que analisa durante a sua fase.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Candidate {
    pub name: String,
    pub path: PathBuf,
    pub options: Vec<(String, String)>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct PhaseSpec {
    pub name: String,
    /// `None` na fase de referência (sem análise).
    pub engine: Option<Candidate>,
    pub start_s: f64,
    pub end_s: f64,
}

/// Divide `[warmup_s, total_s]` em partes iguais: referência + uma por candidata.
pub fn plan(warmup_s: f64, total_s: f64, candidates: Vec<Candidate>) -> Vec<PhaseSpec> {
    let parts = candidates.len() + 1;
    let len = (total_s - warmup_s).max(0.0) / parts as f64;
    let mut phases = vec![PhaseSpec {
        name: "sem-analise".into(),
        engine: None,
        start_s: warmup_s,
        end_s: warmup_s + len,
    }];
    for (i, candidate) in candidates.into_iter().enumerate() {
        let start_s = warmup_s + len * (i + 1) as f64;
        phases.push(PhaseSpec {
            name: format!("com-analise-{}", slug(&candidate.name)),
            engine: Some(candidate),
            start_s,
            end_s: start_s + len,
        });
    }
    phases
}

fn slug(name: &str) -> String {
    let text: String = name
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() {
                c.to_ascii_lowercase()
            } else {
                '-'
            }
        })
        .collect();
    text.trim_matches('-').to_owned()
}

/// Índice da fase em `t` (segundos desde o início). Antes do aquecimento ou depois do fim:
/// `None`.
pub fn index_at(phases: &[PhaseSpec], t: f64) -> Option<usize> {
    phases.iter().position(|p| t >= p.start_s && t < p.end_s)
}

/// Resultado de uma fase.
#[derive(Debug, Clone, Serialize)]
pub struct PhaseReport {
    pub name: String,
    /// Nome da Engine analisando; `None` na referência.
    pub engine: Option<String>,
    pub engine_path: Option<String>,
    /// Backend do Lc0 em uso (quando a Engine o anuncia).
    pub backend: Option<String>,
    pub start_s: f64,
    pub end_s: f64,
    pub settle_seconds: f64,
    pub frame_time: Option<FrameStats>,
    pub process_cpu_percent: Option<SeriesStats>,
    pub gpu_vram_used_mib: Option<SeriesStats>,
    pub gpu_utilization_percent: Option<SeriesStats>,
    /// Maior profundidade e velocidade que a Engine de análise mostrou na fase.
    pub analysis_depth_max: Option<u32>,
    pub analysis_nps_avg: Option<f64>,
    /// Segundos em cada nível do governador de carga (só níveis visitados).
    pub load_level_seconds: BTreeMap<String, f64>,
}

/// Agrega os quadros e as amostras de uma fase.
pub fn summarize(
    spec: &PhaseSpec,
    frame_t: &[f32],
    frame_ms: &[f32],
    samples: &[Sample],
    backend: Option<String>,
) -> PhaseReport {
    let settle = if spec.engine.is_some() { SETTLE_S } else { 0.0 };
    let from = spec.start_s + settle;
    let inside = |t: f64| t >= from && t < spec.end_s;
    let frames: Vec<f64> = frame_t
        .iter()
        .zip(frame_ms)
        .filter(|(t, _)| inside(f64::from(**t)))
        .map(|(_, ms)| f64::from(*ms))
        .collect();
    let chosen: Vec<&Sample> = samples.iter().filter(|s| inside(s.t_s)).collect();
    let series = |f: fn(&Sample) -> Option<f64>| {
        stats::series_stats(&chosen.iter().filter_map(|s| f(s)).collect::<Vec<_>>())
    };
    let nps: Vec<f64> = chosen
        .iter()
        .filter_map(|s| s.analysis_nps)
        .map(|n| n as f64)
        .collect();
    let mut level_seconds = BTreeMap::new();
    for s in samples
        .iter()
        .filter(|s| s.t_s >= spec.start_s && s.t_s < spec.end_s)
    {
        if let Some(level) = s.load_level {
            let name = LEVEL_NAMES[usize::from(level).min(LEVEL_NAMES.len() - 1)];
            *level_seconds.entry(name.to_owned()).or_insert(0.0) += 1.0;
        }
    }
    PhaseReport {
        name: spec.name.clone(),
        engine: spec.engine.as_ref().map(|e| e.name.clone()),
        engine_path: spec
            .engine
            .as_ref()
            .map(|e| e.path.to_string_lossy().into_owned()),
        backend,
        start_s: spec.start_s,
        end_s: spec.end_s,
        settle_seconds: settle,
        frame_time: stats::summarize(&frames),
        process_cpu_percent: series(|s| Some(s.cpu_percent)),
        gpu_vram_used_mib: series(|s| s.vram_used_mib),
        gpu_utilization_percent: series(|s| s.gpu_util_percent),
        analysis_depth_max: chosen.iter().filter_map(|s| s.analysis_depth).max(),
        analysis_nps_avg: (!nps.is_empty()).then(|| nps.iter().sum::<f64>() / nps.len() as f64),
        load_level_seconds: level_seconds,
    }
}

/// Efeito de uma fase de análise em relação à referência (valores positivos de `*_delta`
/// = aumento; FPS negativo = perda).
#[derive(Debug, Clone, Serialize)]
pub struct Impact {
    pub phase: String,
    pub fps_avg_delta_percent: Option<f64>,
    pub fps_1pct_low_delta_percent: Option<f64>,
    pub frame_ms_p95_delta: Option<f64>,
    pub frame_ms_p99_delta: Option<f64>,
    pub process_cpu_percent_delta: Option<f64>,
    pub gpu_vram_used_mib_delta: Option<f64>,
    pub gpu_utilization_percent_delta: Option<f64>,
}

pub fn impact(base: &PhaseReport, other: &PhaseReport) -> Impact {
    let pct = |a: Option<f64>, b: Option<f64>| match (a, b) {
        (Some(a), Some(b)) if a > 0.0 => Some((b - a) / a * 100.0),
        _ => None,
    };
    let diff = |a: Option<f64>, b: Option<f64>| Some(b? - a?);
    let (bf, of) = (base.frame_time.as_ref(), other.frame_time.as_ref());
    let avg = |s: &Option<SeriesStats>| s.as_ref().map(|s| s.avg);
    Impact {
        phase: other.name.clone(),
        fps_avg_delta_percent: pct(bf.map(|f| f.fps_avg), of.map(|f| f.fps_avg)),
        fps_1pct_low_delta_percent: pct(bf.map(|f| f.fps_1pct_low), of.map(|f| f.fps_1pct_low)),
        frame_ms_p95_delta: diff(bf.map(|f| f.frame_ms_p95), of.map(|f| f.frame_ms_p95)),
        frame_ms_p99_delta: diff(bf.map(|f| f.frame_ms_p99), of.map(|f| f.frame_ms_p99)),
        process_cpu_percent_delta: diff(
            avg(&base.process_cpu_percent),
            avg(&other.process_cpu_percent),
        ),
        gpu_vram_used_mib_delta: diff(avg(&base.gpu_vram_used_mib), avg(&other.gpu_vram_used_mib)),
        gpu_utilization_percent_delta: diff(
            avg(&base.gpu_utilization_percent),
            avg(&other.gpu_utilization_percent),
        ),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn candidate(name: &str) -> Candidate {
        Candidate {
            name: name.into(),
            path: PathBuf::from(format!("/e/{name}")),
            options: Vec::new(),
        }
    }

    fn sample(t: f64, cpu: f64, vram: f64, level: Option<u8>) -> Sample {
        Sample {
            t_s: t,
            fps: 60.0,
            cpu_percent: cpu,
            ram_mib: 100.0,
            vram_used_mib: Some(vram),
            gpu_util_percent: Some(10.0),
            analysis_depth: level.map(|_| 12),
            analysis_nps: level.map(|_| 1000),
            load_level: level,
        }
    }

    #[test]
    fn the_run_is_split_into_equal_parts_baseline_first() {
        let phases = plan(3.0, 63.0, vec![candidate("Stockfish 19"), candidate("Lc0")]);
        assert_eq!(phases.len(), 3);
        assert_eq!(phases[0].name, "sem-analise");
        assert!(phases[0].engine.is_none());
        assert_eq!(phases[1].name, "com-analise-stockfish-19");
        assert_eq!(phases[2].name, "com-analise-lc0");
        for (i, p) in phases.iter().enumerate() {
            assert!((p.end_s - p.start_s - 20.0).abs() < 1e-9, "{i}");
        }
        assert!((phases[1].start_s - phases[0].end_s).abs() < 1e-9);
        assert!((phases[2].end_s - 63.0).abs() < 1e-9);
        // Sem candidatas: só a referência, ocupando tudo.
        let alone = plan(3.0, 33.0, Vec::new());
        assert_eq!(alone.len(), 1);
        assert!((alone[0].end_s - 33.0).abs() < 1e-9);
    }

    #[test]
    fn the_phase_at_a_time_follows_the_boundaries() {
        let phases = plan(3.0, 63.0, vec![candidate("a"), candidate("b")]);
        assert_eq!(index_at(&phases, 1.0), None, "aquecimento");
        assert_eq!(index_at(&phases, 3.0), Some(0));
        assert_eq!(index_at(&phases, 22.9), Some(0));
        assert_eq!(index_at(&phases, 23.0), Some(1));
        assert_eq!(index_at(&phases, 62.9), Some(2));
        assert_eq!(index_at(&phases, 63.5), None, "depois do fim");
    }

    #[test]
    fn a_phase_only_counts_its_own_frames_and_skips_the_settle_time() {
        let phases = plan(0.0, 20.0, vec![candidate("lc0")]);
        // 100 quadros de 10 ms na referência; na análise, 1 s lento (settle) e 1 s lento.
        let mut t = Vec::new();
        let mut ms = Vec::new();
        for i in 0..100 {
            t.push(i as f32 * 0.1);
            ms.push(10.0_f32);
        }
        for i in 0..100 {
            t.push(10.0 + i as f32 * 0.1);
            ms.push(if i < 20 { 100.0 } else { 20.0 }); // 2 s de settle em 100 ms
        }
        let base = summarize(&phases[0], &t, &ms, &[], None);
        let with = summarize(&phases[1], &t, &ms, &[], Some("cuda-auto".into()));
        let base_frames = base.frame_time.unwrap();
        assert_eq!(base_frames.frames, 100);
        assert!((base_frames.frame_ms_avg - 10.0).abs() < 1e-6);
        let with_frames = with.frame_time.unwrap();
        assert_eq!(with_frames.frames, 80, "os 2 s iniciais ficam de fora");
        assert!((with_frames.frame_ms_avg - 20.0).abs() < 1e-6);
        assert_eq!(with.backend.as_deref(), Some("cuda-auto"));
        assert_eq!(with.settle_seconds, SETTLE_S);
        assert_eq!(base.settle_seconds, 0.0);
    }

    #[test]
    fn gpu_cpu_and_analysis_series_are_aggregated_per_phase() {
        let phases = plan(0.0, 20.0, vec![candidate("lc0")]);
        let samples: Vec<Sample> = (0..20)
            .map(|i| {
                let t = f64::from(i) + 0.5;
                if i < 10 {
                    sample(t, 10.0, 1000.0, None)
                } else {
                    sample(t, 40.0, 3000.0, Some(if i < 15 { 0 } else { 2 }))
                }
            })
            .collect();
        let base = summarize(&phases[0], &[], &[], &samples, None);
        let with = summarize(&phases[1], &[], &[], &samples, None);
        assert_eq!(base.gpu_vram_used_mib.unwrap().avg, 1000.0);
        assert_eq!(with.gpu_vram_used_mib.unwrap().avg, 3000.0, "sem o settle");
        assert_eq!(with.analysis_depth_max, Some(12));
        assert_eq!(with.analysis_nps_avg, Some(1000.0));
        assert!(base.analysis_depth_max.is_none());
        // Segundos por nível usam a fase inteira (inclusive o settle).
        assert_eq!(with.load_level_seconds["pleno"], 5.0);
        assert_eq!(with.load_level_seconds["minimo"], 5.0);
        assert!(base.load_level_seconds.is_empty());
    }

    #[test]
    fn impact_is_measured_against_the_baseline() {
        let phases = plan(0.0, 20.0, vec![candidate("lc0")]);
        let mut t = Vec::new();
        let mut ms = Vec::new();
        for i in 0..100 {
            t.push(i as f32 * 0.1);
            ms.push(10.0_f32); // 100 FPS
        }
        for i in 0..100 {
            t.push(10.0 + i as f32 * 0.1);
            ms.push(20.0_f32); // 50 FPS
        }
        let samples: Vec<Sample> = (0..20)
            .map(|i| {
                let heavy = i >= 10;
                sample(
                    f64::from(i) + 0.5,
                    if heavy { 30.0 } else { 10.0 },
                    if heavy { 2500.0 } else { 1000.0 },
                    None,
                )
            })
            .collect();
        let base = summarize(&phases[0], &t, &ms, &samples, None);
        let with = summarize(&phases[1], &t, &ms, &samples, None);
        let impact = impact(&base, &with);
        assert_eq!(impact.phase, "com-analise-lc0");
        assert!((impact.fps_avg_delta_percent.unwrap() + 50.0).abs() < 1e-6);
        assert!((impact.frame_ms_p95_delta.unwrap() - 10.0).abs() < 1e-6);
        assert!((impact.process_cpu_percent_delta.unwrap() - 20.0).abs() < 1e-6);
        assert!((impact.gpu_vram_used_mib_delta.unwrap() - 1500.0).abs() < 1e-6);
    }
}
