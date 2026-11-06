//! Intenções enviadas à Partida. Cobre todo o P0; o que a Política ativa ou o
//! escopo do ticket ainda não suporta volta como [`crate::Rejection`] tipada.

use chess_core::{Color, Move, Piece, Square};

#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum Command {
    /// Botão sobre uma peça (Toque intencional nas Políticas competitivas).
    Touch {
        square: Square,
    },
    /// Ajuste (j'adoube).
    Adjust {
        square: Square,
    },
    /// Soltar a peça de `from` em `to`. Quem joga é sempre o lado da vez.
    Release {
        from: Square,
        to: Square,
    },
    /// Peça escolhida para a promoção pendente (D/T/B/C).
    ChoosePromotion {
        piece: Piece,
    },
    /// Retratar/cancelar o Lance em curso (no Assistido, a promoção pendente).
    RetractMove,
    PressClock,
    /// Reclamação de empate pelo lado da vez sobre a Posição atual (arts. 9.2.2 e
    /// 9.3.2). Para a Engine (B9) é a mesma via.
    ClaimDraw,
    /// Reclamação com Lance anunciado (arts. 9.2.1 e 9.3.1): o Lance ainda não foi
    /// feito; precisa ser legal na Posição atual.
    ClaimDrawWithMove {
        mv: Move,
    },
    OfferDraw {
        side: Color,
    },
    /// O adversário de quem propôs aceita/recusa.
    AcceptDraw,
    DeclineDraw,
    Resign {
        side: Color,
    },
    Pause,
    Resume,
    Undo,
    Redo,
    /// Lance da Engine em UCI.
    EngineMove {
        uci: String,
    },
}

/// Nome do Comando, sem payload (usado em [`crate::Rejection`]).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum CommandKind {
    Touch,
    Adjust,
    Release,
    ChoosePromotion,
    RetractMove,
    PressClock,
    ClaimDraw,
    ClaimDrawWithMove,
    OfferDraw,
    AcceptDraw,
    DeclineDraw,
    Resign,
    Pause,
    Resume,
    Undo,
    Redo,
    EngineMove,
}

impl Command {
    pub fn kind(&self) -> CommandKind {
        match self {
            Self::Touch { .. } => CommandKind::Touch,
            Self::Adjust { .. } => CommandKind::Adjust,
            Self::Release { .. } => CommandKind::Release,
            Self::ChoosePromotion { .. } => CommandKind::ChoosePromotion,
            Self::RetractMove => CommandKind::RetractMove,
            Self::PressClock => CommandKind::PressClock,
            Self::ClaimDraw => CommandKind::ClaimDraw,
            Self::ClaimDrawWithMove { .. } => CommandKind::ClaimDrawWithMove,
            Self::OfferDraw { .. } => CommandKind::OfferDraw,
            Self::AcceptDraw => CommandKind::AcceptDraw,
            Self::DeclineDraw => CommandKind::DeclineDraw,
            Self::Resign { .. } => CommandKind::Resign,
            Self::Pause => CommandKind::Pause,
            Self::Resume => CommandKind::Resume,
            Self::Undo => CommandKind::Undo,
            Self::Redo => CommandKind::Redo,
            Self::EngineMove { .. } => CommandKind::EngineMove,
        }
    }
}
