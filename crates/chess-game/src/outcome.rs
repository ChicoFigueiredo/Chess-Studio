//! Resultado e motivo de encerramento da Partida.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum GameResult {
    WhiteWins,
    BlackWins,
    Draw,
}

impl GameResult {
    /// Token de resultado do PGN (`1-0`, `0-1`, `1/2-1/2`).
    pub fn pgn_token(self) -> &'static str {
        match self {
            Self::WhiteWins => "1-0",
            Self::BlackWins => "0-1",
            Self::Draw => "1/2-1/2",
        }
    }
}

/// Por que a Partida terminou. O B6 acrescenta bandeira, Infração e Reclamação.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[non_exhaustive]
pub enum EndReason {
    Checkmate,
    Stalemate,
    InsufficientMaterial,
    ThreefoldRepetition,
    FivefoldRepetition,
    FiftyMoveRule,
    SeventyFiveMoveRule,
    Resignation,
    DrawAgreement,
}

impl EndReason {
    /// Encerramentos que decorrem só da Posição e do histórico (desfazer os reabre).
    pub fn is_positional(self) -> bool {
        !matches!(self, Self::Resignation | Self::DrawAgreement)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Outcome {
    pub result: GameResult,
    pub reason: EndReason,
}
