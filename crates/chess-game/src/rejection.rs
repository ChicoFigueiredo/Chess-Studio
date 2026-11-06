//! Por que um Comando não gerou Eventos. A Partida não muda quando há rejeição.
//!
//! Lance ilegal **não** é rejeição: é um Evento informativo (`IllegalMoveRejected`)
//! que fica no log (DB-04).

use crate::command::CommandKind;
use chess_core::{Piece, Square};
use std::fmt;

#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum Rejection {
    /// A Partida já terminou.
    GameOver,
    /// A Política ativa não tem esse Comando (ex.: Toque no Assistido).
    NotSupportedByPolicy(CommandKind),
    /// Previsto, mas ainda não implementado pela Política ativa.
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
    /// A Partida está em Pausa; só Retomar ou Abandonar (DEC-42).
    Paused,
    AlreadyPaused,
    NotPaused,
    /// Não há peça na casa tocada/ajustada/solta.
    NoPieceAt(Square),
    /// Obrigação de peça tocada (art. 4.3): o Lance precisa sair de uma destas casas
    /// (e ser compatível com o toque); o Comando foi recusado.
    TouchedPieceObligation {
        must_move_from: Vec<Square>,
    },
    /// Há uma peça solta numa casa ilegal: retrate ou acione o relógio.
    ProvisionalReleasePending,
    /// O Lance legal já foi feito (art. 4.7): só resta acionar o relógio.
    MoveAlreadyMade,
    /// Perdeu o direito de reclamar nesta vez ao tocar uma peça (art. 9.4).
    DrawClaimRightLost,
    /// O Lance anunciado na Reclamação não é legal.
    ClaimedMoveIllegal,
    /// O instante injetado é anterior ao do último Evento.
    TimeWentBackwards {
        last_ms: u64,
        got_ms: u64,
    },
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
            Self::Paused => f.write_str("a Partida está em Pausa"),
            Self::AlreadyPaused => f.write_str("a Partida já está em Pausa"),
            Self::NotPaused => f.write_str("a Partida não está em Pausa"),
            Self::NoPieceAt(sq) => write!(f, "não há peça em {sq}"),
            Self::TouchedPieceObligation { must_move_from } => {
                write!(f, "obrigação de peça tocada: mova a peça de ")?;
                for (i, sq) in must_move_from.iter().enumerate() {
                    write!(f, "{}{sq}", if i == 0 { "" } else { " ou " })?;
                }
                Ok(())
            }
            Self::ProvisionalReleasePending => {
                f.write_str("há peça solta numa casa ilegal: retrate ou acione o relógio")
            }
            Self::MoveAlreadyMade => f.write_str("o Lance já foi feito: acione o relógio"),
            Self::DrawClaimRightLost => {
                f.write_str("o direito de reclamar empate nesta vez se perdeu ao tocar uma peça")
            }
            Self::ClaimedMoveIllegal => f.write_str("o Lance anunciado na Reclamação é ilegal"),
            Self::TimeWentBackwards { last_ms, got_ms } => write!(
                f,
                "instante {got_ms} ms anterior ao último evento ({last_ms} ms)"
            ),
            Self::Inconsistent => f.write_str("estado interno incoerente"),
        }
    }
}

impl std::error::Error for Rejection {}
