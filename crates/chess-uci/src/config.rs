//! Configuração do processo filho e dos prazos de cada etapa.

use std::ffi::OsString;
use std::path::PathBuf;
use std::time::Duration;

/// Prazos de cada etapa. Todos configuráveis; os testes usam valores curtos.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Timeouts {
    /// `uci` até `uciok`.
    pub handshake: Duration,
    /// `isready` até `readyok`.
    pub ready: Duration,
    /// Folga somada ao `movetime` (ou ao maior tempo de relógio) antes de enviar `stop`.
    pub search_slack: Duration,
    /// Prazo total de uma busca sem limite de tempo declarado (`depth`, `infinite`).
    pub search_default: Duration,
    /// Espera por `bestmove` depois de um `stop`, antes de matar o processo.
    pub stop_grace: Duration,
    /// Espera pelo término do processo depois de `quit`, antes de matá-lo.
    pub quit: Duration,
}

impl Default for Timeouts {
    fn default() -> Self {
        Self {
            handshake: Duration::from_secs(10),
            ready: Duration::from_secs(10),
            search_slack: Duration::from_secs(2),
            search_default: Duration::from_secs(60),
            stop_grace: Duration::from_secs(2),
            quit: Duration::from_secs(2),
        }
    }
}

/// Como iniciar a Engine.
#[derive(Debug, Clone)]
pub struct EngineConfig {
    pub program: PathBuf,
    pub args: Vec<OsString>,
    pub envs: Vec<(OsString, OsString)>,
    pub timeouts: Timeouts,
    /// Quantas linhas trocadas o log guarda (as mais recentes).
    pub log_capacity: usize,
}

impl EngineConfig {
    pub fn new(program: impl Into<PathBuf>) -> Self {
        Self {
            program: program.into(),
            args: Vec::new(),
            envs: Vec::new(),
            timeouts: Timeouts::default(),
            log_capacity: 200,
        }
    }

    pub fn arg(mut self, arg: impl Into<OsString>) -> Self {
        self.args.push(arg.into());
        self
    }

    pub fn timeouts(mut self, timeouts: Timeouts) -> Self {
        self.timeouts = timeouts;
        self
    }

    pub fn log_capacity(mut self, capacity: usize) -> Self {
        self.log_capacity = capacity;
        self
    }
}
