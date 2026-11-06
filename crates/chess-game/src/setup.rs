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
    /// Simulação FIDE (Leis de 1/1/2023, partidas clássicas).
    FideSimulation,
}

/// Versão das Leis do Xadrez simulada (RULE-01).
pub const FIDE_LAWS_VERSION: &str = "FIDE Laws of Chess, 1 Jan 2023";

/// Controle de tempo abaixo do qual a Simulação FIDE avisa que os apêndices de
/// rápido/blitz não são simulados (DEC-37, RULE-05): 60 minutos por jogador.
pub const CLASSICAL_MIN_BASE_MS: u64 = 60 * 60 * 1000;

/// Em que momento um Lance ilegal vira Infração (DEC-36). A Simulação FIDE é
/// sempre `Clock` (art. 7.5.1); a Eliminação Imediata (B8) poderá escolher.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum InfractionStage {
    /// Ao acionar o relógio.
    #[default]
    Clock,
    /// Ao soltar a peça.
    Release,
}

/// Tudo o que, com o log de Eventos, reconstrói a Partida (ADR-0002).
/// Serializa com a Posição inicial em FEN.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(try_from = "SetupWire", into = "SetupWire")]
pub struct GameSetup {
    policy: PolicyKind,
    start: CanonicalPosition,
    time_control: Option<TimeControl>,
    infraction_stage: InfractionStage,
}

impl GameSetup {
    /// Posição inicial padrão.
    pub fn standard(policy: PolicyKind) -> Self {
        Self {
            policy,
            start: CanonicalPosition::start(),
            time_control: None,
            infraction_stage: InfractionStage::Clock,
        }
    }

    /// Posição inicial por FEN (FEN inválido não cria Partida).
    pub fn from_fen(policy: PolicyKind, fen: &str) -> Result<Self, FenError> {
        Ok(Self {
            policy,
            start: CanonicalPosition::from_fen(fen)?,
            time_control: None,
            infraction_stage: InfractionStage::Clock,
        })
    }

    /// Controle de tempo (base + incremento). Em qualquer Política o relógio é
    /// derivado dos instantes dos Eventos; só a Simulação FIDE aplica Infrações.
    pub fn with_time_control(mut self, time_control: TimeControl) -> Self {
        self.time_control = Some(time_control);
        self
    }

    /// Etapa em que o Lance ilegal vira Infração. Parâmetro para a Eliminação
    /// Imediata (B8, DEC-36); a Simulação FIDE o ignora e usa sempre o relógio.
    pub fn with_infraction_stage(mut self, stage: InfractionStage) -> Self {
        self.infraction_stage = stage;
        self
    }

    pub fn infraction_stage(&self) -> InfractionStage {
        self.infraction_stage
    }

    /// Versão das Leis simulada pela Política, se ela simula alguma (RULE-01).
    pub fn rules_version(&self) -> Option<&'static str> {
        match self.policy {
            PolicyKind::FideSimulation => Some(FIDE_LAWS_VERSION),
            _ => None,
        }
    }

    /// Aviso de que o controle de tempo não é clássico e os apêndices de
    /// rápido/blitz não são simulados (DEC-37, RULE-05).
    pub fn non_classical_notice(&self) -> bool {
        self.policy == PolicyKind::FideSimulation
            && self
                .time_control
                .is_some_and(|tc| tc.base_ms < CLASSICAL_MIN_BASE_MS)
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
    #[serde(default)]
    infraction_stage: InfractionStage,
}

impl From<GameSetup> for SetupWire {
    fn from(s: GameSetup) -> Self {
        Self {
            policy: s.policy,
            start_fen: s.start.to_fen(),
            time_control: s.time_control,
            infraction_stage: s.infraction_stage,
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
            infraction_stage: w.infraction_stage,
        })
    }
}
