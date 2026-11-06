//! Eventos: a fonte de verdade da Partida (ADR-0002). Serializados com payload
//! versionado para o log do B5.
//!
//! Formato JSON: `{"version":1,"at":{...},"kind":{"type":"move_completed",...}}`.
//! Evolução: novas variantes entram em `EventKind` (`non_exhaustive`) sem mudar
//! as existentes; uma mudança incompatível de campo sobe [`PAYLOAD_VERSION`].

use crate::outcome::Outcome;
use crate::time::Timestamp;
use crate::wire;
use chess_core::{Color, Move, Square};
use serde::{Deserialize, Serialize};

/// Versão atual do payload dos Eventos.
pub const PAYLOAD_VERSION: u16 = 1;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Event {
    pub version: u16,
    pub at: Timestamp,
    pub kind: EventKind,
}

impl Event {
    pub fn new(at: Timestamp, kind: EventKind) -> Self {
        Self {
            version: PAYLOAD_VERSION,
            at,
            kind,
        }
    }
}

/// Por que um Lance foi tratado como ilegal (para a explicação conforme as
/// preferências, AC-03).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
#[non_exhaustive]
pub enum IllegalReason {
    EmptyOrigin,
    OpponentPiece,
    SameSquare,
    /// A peça não faz esse Lance ou ele deixaria o rei em xeque; `in_check` diz se
    /// o lado da vez estava em xeque.
    NotLegal { in_check: bool },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
#[non_exhaustive]
pub enum EventKind {
    /// Lance legal concluído (no Assistido, ao soltar). Entra no histórico legal.
    MoveCompleted {
        #[serde(with = "wire::color")]
        side: Color,
        #[serde(with = "wire::uci")]
        mv: Move,
        san: String,
    },
    /// Lance ilegal recusado: informativo, sem penalidade, fora do histórico legal (DB-04).
    IllegalMoveRejected {
        #[serde(with = "wire::color")]
        side: Color,
        #[serde(with = "wire::square")]
        from: Square,
        #[serde(with = "wire::square")]
        to: Square,
        reason: IllegalReason,
    },
    /// Peão chegou à última fileira; falta escolher a peça.
    PromotionPending {
        #[serde(with = "wire::color")]
        side: Color,
        #[serde(with = "wire::square")]
        from: Square,
        #[serde(with = "wire::square")]
        to: Square,
    },
    PromotionCancelled {
        #[serde(with = "wire::color")]
        side: Color,
    },
    DrawOffered {
        #[serde(with = "wire::color")]
        by: Color,
    },
    DrawDeclined {
        #[serde(with = "wire::color")]
        by: Color,
    },
    Resigned {
        #[serde(with = "wire::color")]
        side: Color,
    },
    /// Último Lance desfeito; marca a Partida "com desfazer" (DEC-39).
    MoveUndone {
        #[serde(with = "wire::color")]
        side: Color,
        #[serde(with = "wire::uci")]
        mv: Move,
    },
    MoveRedone {
        #[serde(with = "wire::color")]
        side: Color,
        #[serde(with = "wire::uci")]
        mv: Move,
        san: String,
    },
    /// Fim da Partida (resultado e motivo).
    GameEnded { outcome: Outcome },
}
