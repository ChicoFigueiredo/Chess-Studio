//! Estatística pura do benchmark (sem Bevy): percentis, "1% low" e agregação de frame times.
//!
//! Convenções:
//! - frame time em milissegundos;
//! - percentil pelo método do posto mais próximo (nearest-rank): `p` em (0, 100] devolve o
//!   menor valor tal que ao menos `p`% das amostras são menores ou iguais a ele;
//! - "1% low" = FPS médio dos 1% frames mais lentos (`ceil(n/100)` frames, ao menos 1).

use serde::Serialize;

/// Resumo da distribuição de frame times de uma execução.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct FrameStats {
    pub frames: usize,
    pub total_seconds: f64,
    pub fps_avg: f64,
    pub frame_ms_avg: f64,
    pub frame_ms_min: f64,
    pub frame_ms_max: f64,
    pub frame_ms_p1: f64,
    pub frame_ms_p5: f64,
    pub frame_ms_p50: f64,
    pub frame_ms_p95: f64,
    pub frame_ms_p99: f64,
    /// FPS médio dos 1% frames mais lentos.
    pub fps_1pct_low: f64,
}

/// Percentil (nearest-rank) de uma fatia **já ordenada** de forma crescente.
/// `None` se a fatia é vazia ou `p` está fora de (0, 100].
pub fn percentile_sorted(sorted: &[f64], p: f64) -> Option<f64> {
    if sorted.is_empty() || !(p > 0.0 && p <= 100.0) {
        return None;
    }
    let rank = ((p / 100.0) * sorted.len() as f64).ceil() as usize;
    Some(sorted[rank.clamp(1, sorted.len()) - 1])
}

/// Agrega frame times (ms). `None` se não há amostras ou alguma é inválida (NaN/não positiva).
pub fn summarize(frame_ms: &[f64]) -> Option<FrameStats> {
    if frame_ms.is_empty() || frame_ms.iter().any(|v| !v.is_finite() || *v <= 0.0) {
        return None;
    }
    let mut sorted = frame_ms.to_vec();
    sorted.sort_by(f64::total_cmp);
    let n = sorted.len();
    let total_ms: f64 = sorted.iter().sum();
    let worst = n.div_ceil(100);
    let worst_avg_ms = sorted[n - worst..].iter().sum::<f64>() / worst as f64;
    let pct = |p| percentile_sorted(&sorted, p).unwrap_or(f64::NAN);
    Some(FrameStats {
        frames: n,
        total_seconds: total_ms / 1000.0,
        fps_avg: n as f64 / (total_ms / 1000.0),
        frame_ms_avg: total_ms / n as f64,
        frame_ms_min: sorted[0],
        frame_ms_max: sorted[n - 1],
        frame_ms_p1: pct(1.0),
        frame_ms_p5: pct(5.0),
        frame_ms_p50: pct(50.0),
        frame_ms_p95: pct(95.0),
        frame_ms_p99: pct(99.0),
        fps_1pct_low: 1000.0 / worst_avg_ms,
    })
}

/// Mínimo, média e máximo de uma série de amostras (RAM, CPU, VRAM...).
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct SeriesStats {
    pub samples: usize,
    pub min: f64,
    pub avg: f64,
    pub max: f64,
}

/// `None` se a série é vazia ou tem valor não finito.
pub fn series_stats(values: &[f64]) -> Option<SeriesStats> {
    if values.is_empty() || values.iter().any(|v| !v.is_finite()) {
        return None;
    }
    Some(SeriesStats {
        samples: values.len(),
        min: values.iter().copied().fold(f64::INFINITY, f64::min),
        avg: values.iter().sum::<f64>() / values.len() as f64,
        max: values.iter().copied().fold(f64::NEG_INFINITY, f64::max),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn approx(a: f64, b: f64) -> bool {
        (a - b).abs() < 1e-9
    }

    #[test]
    fn percentile_uses_nearest_rank() {
        let v: Vec<f64> = (1..=100).map(f64::from).collect();
        assert_eq!(percentile_sorted(&v, 1.0), Some(1.0));
        assert_eq!(percentile_sorted(&v, 50.0), Some(50.0));
        assert_eq!(percentile_sorted(&v, 99.0), Some(99.0));
        assert_eq!(percentile_sorted(&v, 100.0), Some(100.0));
    }

    #[test]
    fn percentile_of_single_sample_is_that_sample() {
        assert_eq!(percentile_sorted(&[7.0], 99.0), Some(7.0));
        assert_eq!(percentile_sorted(&[7.0], 1.0), Some(7.0));
    }

    #[test]
    fn percentile_rejects_empty_and_out_of_range() {
        assert_eq!(percentile_sorted(&[], 50.0), None);
        assert_eq!(percentile_sorted(&[1.0], 0.0), None);
        assert_eq!(percentile_sorted(&[1.0], 101.0), None);
    }

    #[test]
    fn summarize_constant_60fps() {
        let frames = vec![1000.0 / 60.0; 600];
        let s = summarize(&frames).expect("amostras válidas");
        assert_eq!(s.frames, 600);
        assert!(approx(s.fps_avg, 60.0));
        assert!(approx(s.total_seconds, 10.0));
        assert!(approx(s.fps_1pct_low, 60.0));
        assert!(approx(s.frame_ms_p99, 1000.0 / 60.0));
    }

    #[test]
    fn summarize_is_independent_of_input_order() {
        let a = [5.0, 1.0, 3.0, 2.0, 4.0];
        let b = [1.0, 2.0, 3.0, 4.0, 5.0];
        assert_eq!(summarize(&a), summarize(&b));
    }

    #[test]
    fn one_percent_low_averages_the_slowest_one_percent() {
        // 100 frames: 99 de 10 ms e 1 de 100 ms -> o pior 1% é 1 frame de 100 ms = 10 FPS.
        let mut frames = vec![10.0; 99];
        frames.push(100.0);
        let s = summarize(&frames).expect("amostras válidas");
        assert!(approx(s.fps_1pct_low, 10.0));
        assert!(approx(s.frame_ms_max, 100.0));
        assert!(approx(s.frame_ms_p99, 10.0));
        assert!(approx(s.frame_ms_p50, 10.0));
        // média: (990 + 100) / 100 = 10.9 ms
        assert!(approx(s.frame_ms_avg, 10.9));
    }

    #[test]
    fn one_percent_low_uses_at_least_one_frame() {
        let s = summarize(&[10.0, 20.0, 40.0]).expect("amostras válidas");
        assert!(approx(s.fps_1pct_low, 25.0)); // 1 frame de 40 ms
    }

    #[test]
    fn summarize_rejects_empty_and_invalid_samples() {
        assert!(summarize(&[]).is_none());
        assert!(summarize(&[10.0, f64::NAN]).is_none());
        assert!(summarize(&[10.0, -1.0]).is_none());
        assert!(summarize(&[0.0]).is_none());
    }

    #[test]
    fn series_stats_min_avg_max() {
        let s = series_stats(&[2.0, 4.0, 9.0]).expect("série não vazia");
        assert_eq!(s.samples, 3);
        assert!(approx(s.min, 2.0));
        assert!(approx(s.avg, 5.0));
        assert!(approx(s.max, 9.0));
        assert!(series_stats(&[]).is_none());
    }
}
