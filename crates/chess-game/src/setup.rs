//! Configuração imutável de uma Partida.

use crate::online::{OnlineGame, OnlineSetupError, Variant};
use crate::time::TimeControl;
use crate::wire;
use chess_core::{CanonicalPosition, Color, FenError};
use serde::{Deserialize, Serialize};

/// Política de arbitragem escolhida antes da Partida.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[non_exhaustive]
pub enum PolicyKind {
    Assisted,
    /// Simulação FIDE (Leis de 1/1/2023, partidas clássicas).
    FideSimulation,
    /// Eliminação Imediata (`SuddenElimination`): regra **personalizada** de
    /// treinamento, nunca FIDE. A primeira Infração concluída encerra a Partida.
    SuddenElimination,
    /// Partida online (ADR-0007, CX-19): nenhuma Política local; valem as regras do
    /// Provedor, e só o que ele confirma entra no log. Nasce por [`GameSetup::online`].
    Online,
}

/// Rótulo exposto no snapshot da Eliminação Imediata (AC-06): nunca "FIDE".
pub const CUSTOM_RULES_LABEL: &str = "Regra personalizada de treinamento — Eliminação Imediata";

/// Versão das Leis do Xadrez simulada (RULE-01).
pub const FIDE_LAWS_VERSION: &str = "FIDE Laws of Chess, 1 Jan 2023";

/// Controle de tempo abaixo do qual a Simulação FIDE avisa que os apêndices de
/// rápido/blitz não são simulados (DEC-37, RULE-05): 60 minutos por jogador.
pub const CLASSICAL_MIN_BASE_MS: u64 = 60 * 60 * 1000;

/// Em que momento um Lance ilegal vira Infração (DEC-36). A Simulação FIDE é
/// sempre `Clock` (art. 7.5.1); a Eliminação Imediata escolhe antes da Partida.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum InfractionStage {
    /// Ao acionar o relógio.
    #[default]
    Clock,
    /// Ao soltar a peça.
    Release,
}

/// Quem joga contra o usuário: a Engine ocupa um dos lados com força por Elo
/// simples (DEC-29). Metadado imutável da Partida, gravado no `GameSetup`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct EngineSeat {
    /// Lado jogado pela Engine.
    #[serde(with = "wire::color")]
    pub color: Color,
    /// Elo pedido (`UCI_Elo`, ou `Skill Level` derivado quando a Engine não anuncia Elo).
    pub elo: u32,
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
    engine: Option<EngineSeat>,
    online: Option<OnlineGame>,
}

impl GameSetup {
    /// Posição inicial padrão.
    pub fn standard(policy: PolicyKind) -> Self {
        Self {
            policy,
            start: CanonicalPosition::start(),
            time_control: None,
            infraction_stage: InfractionStage::Clock,
            engine: None,
            online: None,
        }
    }

    /// Posição inicial por FEN (FEN inválido não cria Partida).
    pub fn from_fen(policy: PolicyKind, fen: &str) -> Result<Self, FenError> {
        Ok(Self {
            policy,
            start: CanonicalPosition::from_fen(fen)?,
            time_control: None,
            infraction_stage: InfractionStage::Clock,
            engine: None,
            online: None,
        })
    }

    /// Partida online (ADR-0007): o Provedor decide Lances, relógio e resultado. Só xadrez
    /// padrão; outra variante é recusada antes de criar a Partida. `start_fen` é a Posição
    /// inicial do Provedor (`None`: a padrão). O controle de tempo, se houver, vem por
    /// [`GameSetup::with_time_control`].
    pub fn online(
        game: OnlineGame,
        variant: Variant,
        start_fen: Option<&str>,
    ) -> Result<Self, OnlineSetupError> {
        if let Variant::Other(name) = variant {
            return Err(OnlineSetupError::UnsupportedVariant(name));
        }
        let start = match start_fen {
            Some(fen) => {
                CanonicalPosition::from_fen(fen.trim()).map_err(OnlineSetupError::InvalidFen)?
            }
            None => CanonicalPosition::start(),
        };
        Ok(Self {
            policy: PolicyKind::Online,
            start,
            time_control: None,
            infraction_stage: InfractionStage::Clock,
            engine: None,
            online: Some(game),
        })
    }

    /// O que o Provedor declarou, se a Partida é online.
    pub fn online_game(&self) -> Option<&OnlineGame> {
        self.online.as_ref()
    }

    /// Controle de tempo (base + incremento). Em qualquer Política o relógio é
    /// derivado dos instantes dos Eventos; só a Simulação FIDE aplica Infrações.
    pub fn with_time_control(mut self, time_control: TimeControl) -> Self {
        self.time_control = Some(time_control);
        self
    }

    /// Etapa em que o Lance ilegal vira Infração. Vale na Eliminação
    /// Imediata (DEC-36); a Simulação FIDE o ignora e usa sempre o relógio.
    pub fn with_infraction_stage(mut self, stage: InfractionStage) -> Self {
        self.infraction_stage = stage;
        self
    }

    /// Partida contra a Engine: ela joga `seat.color` com o Elo pedido (DEC-29).
    pub fn with_engine(mut self, seat: EngineSeat) -> Self {
        self.engine = Some(seat);
        self
    }

    /// A Engine adversária, se houver.
    pub fn engine(&self) -> Option<EngineSeat> {
        self.engine
    }

    pub fn infraction_stage(&self) -> InfractionStage {
        self.infraction_stage
    }

    /// Versão das Leis simulada pela Política, se ela simula alguma (RULE-01).
    pub fn rules_version(&self) -> Option<&'static str> {
        match self.policy {
            PolicyKind::FideSimulation => Some(FIDE_LAWS_VERSION),
            PolicyKind::SuddenElimination => Some(CUSTOM_RULES_LABEL),
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
    #[serde(default, skip_serializing_if = "Option::is_none")]
    engine: Option<EngineSeat>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    online: Option<OnlineGame>,
}

impl From<GameSetup> for SetupWire {
    fn from(s: GameSetup) -> Self {
        Self {
            policy: s.policy,
            start_fen: s.start.to_fen(),
            time_control: s.time_control,
            infraction_stage: s.infraction_stage,
            engine: s.engine,
            online: s.online,
        }
    }
}

impl TryFrom<SetupWire> for GameSetup {
    type Error = String;

    fn try_from(w: SetupWire) -> Result<Self, String> {
        // Dados de Provedor só existem numa Partida online: nenhum caminho do app grava o
        // contrário, então é log adulterado.
        if w.online.is_some() && w.policy != PolicyKind::Online {
            return Err("dados de Provedor numa Partida local".to_owned());
        }
        Ok(Self {
            policy: w.policy,
            start: CanonicalPosition::from_fen(&w.start_fen).map_err(|e| e.to_string())?,
            time_control: w.time_control,
            infraction_stage: w.infraction_stage,
            engine: w.engine,
            online: w.online,
        })
    }
}
