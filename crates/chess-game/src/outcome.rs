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

/// Por que a Partida terminou. Inclui bandeira e Infração (B6); Reclamação reaproveita tripla/50 lances.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[non_exhaustive]
pub enum EndReason {
    Checkmate,
    Stalemate,
    InsufficientMaterial,
    /// Posição morta além do material: bloqueio total de peões (art. 5.2.2).
    DeadPosition,
    ThreefoldRepetition,
    FivefoldRepetition,
    FiftyMoveRule,
    SeventyFiveMoveRule,
    Resignation,
    DrawAgreement,
    /// Bandeira caiu (art. 6.9).
    Timeout,
    /// Bandeira caiu, mas o adversário não podia dar mate (art. 6.9).
    TimeoutDraw,
    /// Segundo Lance ilegal concluído (art. 7.5.5).
    SecondIllegalMove,
    /// Segundo Lance ilegal, mas o adversário não podia dar mate (art. 7.5.5).
    SecondIllegalMoveDraw,
    /// Eliminação Imediata: a primeira Infração concluída encerrou a Partida.
    EliminatedByInfraction,
    /// Eliminação Imediata, mas o adversário não podia dar mate (DEC-35).
    EliminatedByInfractionDraw,
    /// Partida online: o adversário saiu e o Provedor deu a vitória (ou o empate) a quem
    /// ficou.
    Disconnection,
    /// Partida online encerrada pelo Provedor por outro motivo (fair play, sem início,
    /// fim desconhecido), com resultado.
    EndedByProvider,
}

impl EndReason {
    /// Encerramentos que decorrem só da Posição e do histórico (desfazer os reabre).
    pub fn is_positional(self) -> bool {
        !matches!(
            self,
            Self::Resignation
                | Self::DrawAgreement
                | Self::Timeout
                | Self::TimeoutDraw
                | Self::SecondIllegalMove
                | Self::SecondIllegalMoveDraw
                | Self::EliminatedByInfraction
                | Self::EliminatedByInfractionDraw
                | Self::Disconnection
                | Self::EndedByProvider
        )
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Outcome {
    pub result: GameResult,
    pub reason: EndReason,
}
