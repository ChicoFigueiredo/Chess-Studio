//! Configuração imutável de uma Partida.

use crate::time::TimeControl;
use chess_core::{CanonicalPosition, FenError};
use serde::{Deserialize, Serialize};

/// Política de arbitragem escolhida antes da Partida.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[non_exhaustive]
pub enum PolicyKind {
    Assisted,
}

/// Tudo o que, com o log de Eventos, reconstrói a Partida (ADR-0002).
/// Serializa com a Posição inicial em FEN.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(try_from = "SetupWire", into = "SetupWire")]
pub struct GameSetup {
    policy: PolicyKind,
    start: CanonicalPosition,
    time_control: Option<TimeControl>,
}

impl GameSetup {
    /// Posição inicial padrão.
    pub fn standard(policy: PolicyKind) -> Self {
        Self {
            policy,
            start: CanonicalPosition::start(),
            time_control: None,
        }
    }

    /// Posição inicial por FEN (FEN inválido não cria Partida).
    pub fn from_fen(policy: PolicyKind, fen: &str) -> Result<Self, FenError> {
        Ok(Self {
            policy,
            start: CanonicalPosition::from_fen(fen)?,
            time_control: None,
        })
    }

    /// Controle de tempo (guardado; o Assistido o ignora).
    pub fn with_time_control(mut self, time_control: TimeControl) -> Self {
        self.time_control = Some(time_control);
        self
    }

    pub fn policy(&self) -> PolicyKind {
        self.policy
    }

    pub fn start(&self) -> &CanonicalPosition {
        &self.start
    }

    pub fn time_control(&self) -> Option<TimeControl> {
        self.time_control
    }
}

#[derive(Serialize, Deserialize)]
struct SetupWire {
    policy: PolicyKind,
    start_fen: String,
    time_control: Option<TimeControl>,
}

impl From<GameSetup> for SetupWire {
    fn from(s: GameSetup) -> Self {
        Self {
            policy: s.policy,
            start_fen: s.start.to_fen(),
            time_control: s.time_control,
        }
    }
}

impl TryFrom<SetupWire> for GameSetup {
    type Error = FenError;

    fn try_from(w: SetupWire) -> Result<Self, FenError> {
        Ok(Self {
            policy: w.policy,
            start: CanonicalPosition::from_fen(&w.start_fen)?,
            time_control: w.time_control,
        })
    }
}
