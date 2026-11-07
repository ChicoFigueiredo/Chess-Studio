//! Argumentos de linha de comando do modo benchmark (parser puro, sem Bevy).

use chess_uci::LoadMode;
use std::path::PathBuf;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Preset {
    Balanced,
    Economy,
}

impl Preset {
    /// Nome em pt-BR, como na linha de comando e no JSON.
    pub fn label(self) -> &'static str {
        match self {
            Self::Balanced => "equilibrado",
            Self::Economy => "economico",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EngineMode {
    Idle,
    Active,
}

impl EngineMode {
    pub fn label(self) -> &'static str {
        match self {
            Self::Idle => "ociosa",
            Self::Active => "ativa",
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct BenchmarkConfig {
    pub minutes: f64,
    pub width: u32,
    pub height: u32,
    pub preset: Preset,
    pub engine: EngineMode,
    pub out: Option<PathBuf>,
    /// `--engine-path` (repetível): Engines que analisam, uma fase cada. Vazio = as do
    /// ambiente (`CHESS_STUDIO_ENGINE`/PATH e `CHESS_STUDIO_LC0`).
    pub engine_paths: Vec<PathBuf>,
    /// `--network`: rede neural (`WeightsFile`) para as Engines que a pedem (Lc0).
    pub network: Option<PathBuf>,
    /// `--load`: modo do governador de carga durante a análise.
    pub load: LoadMode,
    /// `--backend`: backend do Lc0 (padrão: automático).
    pub backend: Option<String>,
}

impl Default for BenchmarkConfig {
    fn default() -> Self {
        Self {
            minutes: 5.0,
            width: 1920,
            height: 1080,
            preset: Preset::Balanced,
            engine: EngineMode::Idle,
            out: None,
            engine_paths: Vec::new(),
            network: None,
            load: LoadMode::Balanced,
            backend: None,
        }
    }
}

/// Interpreta os argumentos (sem o nome do programa).
/// `Ok(None)` = sem `--benchmark` (execução normal do app); `Err` = uso inválido.
pub fn parse_args<I, S>(args: I) -> Result<Option<BenchmarkConfig>, String>
where
    I: IntoIterator<Item = S>,
    S: AsRef<str>,
{
    let mut cfg = BenchmarkConfig::default();
    let mut enabled = false;
    let mut it = args.into_iter();
    while let Some(arg) = it.next() {
        let arg = arg.as_ref();
        let (name, inline) = match arg.split_once('=') {
            Some((n, v)) => (n, Some(v.to_owned())),
            None => (arg, None),
        };
        let mut value = |what: &str| -> Result<String, String> {
            inline
                .clone()
                .or_else(|| it.next().map(|s| s.as_ref().to_owned()))
                .ok_or_else(|| format!("{name} exige um valor ({what})"))
        };
        match name {
            "--benchmark" => enabled = true,
            "--minutes" => {
                let v = value("minutos")?;
                cfg.minutes = v
                    .parse::<f64>()
                    .ok()
                    .filter(|m| m.is_finite() && *m > 0.0)
                    .ok_or_else(|| format!("--minutes inválido: {v}"))?;
            }
            "--resolution" => {
                let v = value("LARGURAxALTURA")?;
                (cfg.width, cfg.height) = parse_resolution(&v)?;
            }
            "--preset" => {
                cfg.preset = match value("equilibrado|economico")?.as_str() {
                    "equilibrado" => Preset::Balanced,
                    "economico" => Preset::Economy,
                    other => return Err(format!("--preset inválido: {other}")),
                };
            }
            "--engine" => {
                cfg.engine = match value("ativa|ociosa")?.as_str() {
                    "ativa" => EngineMode::Active,
                    "ociosa" => EngineMode::Idle,
                    other => return Err(format!("--engine inválido: {other}")),
                };
            }
            "--out" => cfg.out = Some(PathBuf::from(value("diretório")?)),
            "--engine-path" => cfg.engine_paths.push(PathBuf::from(value("executável")?)),
            "--network" => cfg.network = Some(PathBuf::from(value("arquivo da rede")?)),
            "--backend" => cfg.backend = Some(value("nome do backend")?),
            "--load" => {
                cfg.load = match value("maximo|equilibrado|leve")?.as_str() {
                    "maximo" => LoadMode::Max,
                    "equilibrado" => LoadMode::Balanced,
                    "leve" => LoadMode::Light,
                    other => return Err(format!("--load inválido: {other}")),
                };
            }
            _ => {}
        }
    }
    Ok(enabled.then_some(cfg))
}

fn parse_resolution(text: &str) -> Result<(u32, u32), String> {
    let bad = || format!("--resolution inválido: {text} (use LARGURAxALTURA, ex.: 1920x1080)");
    let (w, h) = text.split_once(['x', 'X']).ok_or_else(bad)?;
    let (w, h): (u32, u32) = (w.parse().map_err(|_| bad())?, h.parse().map_err(|_| bad())?);
    if w == 0 || h == 0 {
        return Err(bad());
    }
    Ok((w, h))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn without_flag_runs_the_normal_app() {
        assert_eq!(parse_args(["--minutes", "3"]), Ok(None));
        assert_eq!(parse_args::<[&str; 0], &str>([]), Ok(None));
    }

    #[test]
    fn defaults_are_five_minutes_1080p_balanced_idle() {
        let cfg = parse_args(["--benchmark"]).unwrap().unwrap();
        assert_eq!(cfg, BenchmarkConfig::default());
        assert_eq!((cfg.minutes, cfg.width, cfg.height), (5.0, 1920, 1080));
        assert_eq!(cfg.preset, Preset::Balanced);
        assert_eq!(cfg.engine, EngineMode::Idle);
    }

    #[test]
    fn parses_all_options() {
        let cfg = parse_args([
            "--benchmark",
            "--minutes",
            "0.5",
            "--resolution=2560x1440",
            "--preset",
            "economico",
            "--engine",
            "ativa",
            "--out",
            "/tmp/x",
        ])
        .unwrap()
        .unwrap();
        assert_eq!(cfg.minutes, 0.5);
        assert_eq!((cfg.width, cfg.height), (2560, 1440));
        assert_eq!(cfg.preset, Preset::Economy);
        assert_eq!(cfg.engine, EngineMode::Active);
        assert_eq!(cfg.out, Some(PathBuf::from("/tmp/x")));
    }

    #[test]
    fn parses_the_analysis_options() {
        let cfg = parse_args([
            "--benchmark",
            "--engine",
            "ativa",
            "--engine-path",
            "/e/stockfish",
            "--engine-path=/e/lc0",
            "--network",
            "/n/net.pb.gz",
            "--load",
            "leve",
            "--backend",
            "cuda-fp16",
        ])
        .unwrap()
        .unwrap();
        assert_eq!(
            cfg.engine_paths,
            [PathBuf::from("/e/stockfish"), PathBuf::from("/e/lc0")]
        );
        assert_eq!(cfg.network, Some(PathBuf::from("/n/net.pb.gz")));
        assert_eq!(cfg.load, LoadMode::Light);
        assert_eq!(cfg.backend.as_deref(), Some("cuda-fp16"));
        let defaults = parse_args(["--benchmark"]).unwrap().unwrap();
        assert_eq!(defaults.load, LoadMode::Balanced);
        assert!(defaults.engine_paths.is_empty() && defaults.backend.is_none());
    }

    #[test]
    fn rejects_invalid_values() {
        for bad in [
            vec!["--benchmark", "--minutes", "0"],
            vec!["--benchmark", "--minutes", "abc"],
            vec!["--benchmark", "--resolution", "1920"],
            vec!["--benchmark", "--resolution", "0x1080"],
            vec!["--benchmark", "--preset", "ultra"],
            vec!["--benchmark", "--engine", "talvez"],
            vec!["--benchmark", "--load", "turbo"],
            vec!["--benchmark", "--engine-path"],
            vec!["--benchmark", "--out"],
        ] {
            assert!(parse_args(bad.clone()).is_err(), "{bad:?}");
        }
    }
}
