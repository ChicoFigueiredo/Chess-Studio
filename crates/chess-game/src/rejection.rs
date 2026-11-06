//! Por que um Comando não gerou Eventos. A Partida não muda quando há rejeição.
//!
//! Lance ilegal **não** é rejeição: é um Evento informativo (`IllegalMoveRejected`)
//! que fica no log (DB-04).

use crate::command::CommandKind;
use chess_core::Piece;
use std::fmt;

#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum Rejection {
    /// A Partida já terminou.
    GameOver,
    /// A Política ativa não tem esse Comando (ex.: Toque no Assistido).
    NotSupportedByPolicy(CommandKind),
    /// Previsto para o P0, mas ainda não implementado (Pausa, Engine).
    NotYetImplemented(CommandKind),
    /// Há promoção pendente: escolha a peça ou cancele antes.
    PromotionPending,
    NoPromotionPending,
    /// Só dama, torre, bispo ou cavalo.
    InvalidPromotionPiece(Piece),
    NothingToRetract,
    NothingToUndo,
    NothingToRedo,
    NoDrawOffer,
    DrawOfferPending,
    /// O instante injetado é anterior ao do último Evento.
    TimeWentBackwards { last_ms: u64, got_ms: u64 },
    /// Estado interno incoerente (bug); a Partida não mudou.
    Inconsistent,
}

impl fmt::Display for Rejection {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::GameOver => f.write_str("a Partida já terminou"),
            Self::NotSupportedByPolicy(c) => {
                write!(f, "comando {c:?} não existe na Política ativa")
            }
            Self::NotYetImplemented(c) => write!(f, "comando {c:?} ainda não implementado"),
            Self::PromotionPending => f.write_str("há promoção pendente"),
            Self::NoPromotionPending => f.write_str("não há promoção pendente"),
            Self::InvalidPromotionPiece(p) => write!(f, "promoção inválida para {p:?}"),
            Self::NothingToRetract => f.write_str("não há Lance em curso para retratar"),
            Self::NothingToUndo => f.write_str("não há Lance para desfazer"),
            Self::NothingToRedo => f.write_str("não há Lance para refazer"),
            Self::NoDrawOffer => f.write_str("não há proposta de empate pendente"),
            Self::DrawOfferPending => f.write_str("já há proposta de empate pendente"),
            Self::TimeWentBackwards { last_ms, got_ms } => write!(
                f,
                "instante {got_ms} ms anterior ao último evento ({last_ms} ms)"
            ),
            Self::Inconsistent => f.write_str("estado interno incoerente"),
        }
    }
}

impl std::error::Error for Rejection {}
