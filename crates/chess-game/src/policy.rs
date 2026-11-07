//! Política de arbitragem: decide, para um Comando no estado atual, quais Eventos
//! acontecem. Pura: lê o estado, não o muda (quem aplica é `Game::handle`).
//!
//! Políticas: Assistido (aqui) e Simulação FIDE (`fide.rs`). A Eliminação Imediata
//! usa o mesmo núcleo competitivo com outra etapa de Infração e outra
//! consequência. O que é comum (encerramentos automáticos, Pausa, abandono,
//! propostas de empate, Engine) fica neste módulo.

use crate::command::{Command, CommandKind};
use crate::event::{EventKind, IllegalReason};
use crate::fide;
use crate::outcome::{EndReason, GameResult, Outcome};
use crate::rejection::Rejection;
use crate::setup::PolicyKind;
use crate::state::{GameState, Played};
use chess_core::{CanonicalPosition, Color, GameStatus, Move, Piece, Square};

pub(crate) trait ArbitrationPolicy {
    fn decide(&self, state: &GameState, command: &Command) -> Result<Vec<EventKind>, Rejection>;
}

pub(crate) fn policy_for(kind: PolicyKind) -> &'static dyn ArbitrationPolicy {
    match kind {
        PolicyKind::Assisted => &Assisted,
        PolicyKind::FideSimulation => &fide::FIDE_SIMULATION,
        PolicyKind::SuddenElimination => &fide::SUDDEN_ELIMINATION,
    }
}

/// Quais empates a Política aplica sozinha (os de 5 repetições e 75 lances valem sempre).
#[derive(Debug, Clone, Copy)]
pub(crate) struct AutoDraws {
    pub threefold: bool,
    pub fifty_move: bool,
}

/// Fim da Partida decorrente só da Posição e das ocorrências dela (`occurrences`
/// já conta a Posição atual). Precedência: mate, afogamento, 75 lances,
/// 5 repetições, material insuficiente, e então tripla e 50 se automáticas.
pub(crate) fn automatic_outcome(
    position: &CanonicalPosition,
    occurrences: usize,
    auto: AutoDraws,
) -> Option<Outcome> {
    let draw = |reason| {
        Some(Outcome {
            result: GameResult::Draw,
            reason,
        })
    };
    match position.status() {
        GameStatus::Checkmate { winner } => {
            return Some(Outcome {
                result: match winner {
                    Color::White => GameResult::WhiteWins,
                    Color::Black => GameResult::BlackWins,
                },
                reason: EndReason::Checkmate,
            });
        }
        GameStatus::Stalemate => return draw(EndReason::Stalemate),
        GameStatus::Ongoing => {}
    }
    if position.is_seventy_five_move_draw() {
        draw(EndReason::SeventyFiveMoveRule)
    } else if occurrences >= 5 {
        draw(EndReason::FivefoldRepetition)
    } else if position.is_insufficient_material() {
        draw(EndReason::InsufficientMaterial)
    } else if position.is_dead_position() {
        draw(EndReason::DeadPosition)
    } else if auto.threefold && occurrences >= 3 {
        draw(EndReason::ThreefoldRepetition)
    } else if auto.fifty_move && position.can_claim_fifty_move() {
        draw(EndReason::FiftyMoveRule)
    } else {
        None
    }
}

/// Assistido (DEC-34/38/39): Lance legal solto já é concluído; o ilegal é recusado
/// sem penalidade; tripla e 50 lances automáticos; desfazer/refazer permitidos.
struct Assisted;

pub(crate) const ASSISTED_AUTO: AutoDraws = AutoDraws {
    threefold: true,
    fifty_move: true,
};

impl ArbitrationPolicy for Assisted {
    fn decide(&self, state: &GameState, command: &Command) -> Result<Vec<EventKind>, Rejection> {
        let kind = command.kind();
        match command {
            Command::Touch { .. }
            | Command::Adjust { .. }
            | Command::PressClock
            | Command::ClaimDraw
            | Command::ClaimDrawWithMove { .. } => {
                return Err(Rejection::NotSupportedByPolicy(kind));
            }
            _ => {}
        }
        // Só desfazer reabre uma Partida terminada (e apenas por fim posicional).
        let reopens = kind == CommandKind::Undo;
        if state.outcome.is_some() && !reopens {
            return Err(Rejection::GameOver);
        }
        gate_pause(state, command)?;
        if let Some(result) = common(state, command, ASSISTED_AUTO) {
            return result;
        }
        match command {
            Command::Release { from, to } => release(state, *from, *to),
            Command::ChoosePromotion { piece } => choose_promotion(state, *piece),
            Command::RetractMove => match state.pending_promotion {
                Some(p) => Ok(vec![EventKind::PromotionCancelled { side: p.side }]),
                None => Err(Rejection::NothingToRetract),
            },
            Command::Undo => undo(state),
            Command::Redo => redo(state),
            _ => Err(Rejection::Inconsistent),
        }
    }
}

/// Em Pausa só valem Retomar e Abandonar (DEC-41/42); Pausar de novo é `AlreadyPaused`.
pub(crate) fn gate_pause(state: &GameState, command: &Command) -> Result<(), Rejection> {
    if state.paused
        && !matches!(
            command,
            Command::Resume | Command::Pause | Command::Resign { .. }
        )
    {
        return Err(Rejection::Paused);
    }
    Ok(())
}

/// Comandos que se comportam igual em toda Política: Pausa, abandono, propostas de
/// empate e Engine. `None` se o Comando é específico da Política.
pub(crate) fn common(
    state: &GameState,
    command: &Command,
    auto: AutoDraws,
) -> Option<Result<Vec<EventKind>, Rejection>> {
    Some(match command {
        Command::Pause => {
            if state.paused {
                Err(Rejection::AlreadyPaused)
            } else {
                Ok(vec![EventKind::Paused])
            }
        }
        Command::Resume => {
            if state.paused {
                Ok(vec![EventKind::Resumed])
            } else {
                Err(Rejection::NotPaused)
            }
        }
        Command::OfferDraw { side } => match state.draw_offer {
            Some(_) => Err(Rejection::DrawOfferPending),
            None => Ok(vec![EventKind::DrawOffered { by: *side }]),
        },
        Command::AcceptDraw => state.draw_offer.ok_or(Rejection::NoDrawOffer).map(|_| {
            vec![EventKind::GameEnded {
                outcome: Outcome {
                    result: GameResult::Draw,
                    reason: EndReason::DrawAgreement,
                },
            }]
        }),
        Command::DeclineDraw => state
            .draw_offer
            .ok_or(Rejection::NoDrawOffer)
            .map(|by| vec![EventKind::DrawDeclined { by: by.opposite() }]),
        Command::Resign { side } => Ok(vec![
            EventKind::Resigned { side: *side },
            EventKind::GameEnded {
                outcome: Outcome {
                    result: match side {
                        Color::White => GameResult::BlackWins,
                        Color::Black => GameResult::WhiteWins,
                    },
                    reason: EndReason::Resignation,
                },
            },
        ]),
        Command::EngineMove { uci } => engine_move(state, uci, auto),
        _ => return None,
    })
}

/// Lance da Engine: o núcleo valida; ilegal vira `EngineMoveRejected` (logado). Se
/// legal, conclui o Lance e aciona o relógio da Engine automaticamente (DEC-33).
fn engine_move(state: &GameState, uci: &str, auto: AutoDraws) -> Result<Vec<EventKind>, Rejection> {
    if state.pending_promotion.is_some() {
        return Err(Rejection::PromotionPending);
    }
    if state.made.is_some() {
        return Err(Rejection::MoveAlreadyMade);
    }
    if state.provisional_release.is_some() {
        return Err(Rejection::ProvisionalReleasePending);
    }
    let position = state.position();
    let side = position.side_to_move();
    let legal = uci
        .parse::<Move>()
        .ok()
        .filter(|m| position.legal_moves().contains(m));
    match legal {
        Some(mv) => completed(state, mv, false, auto),
        None => Ok(vec![EventKind::EngineMoveRejected {
            side,
            uci: uci.to_owned(),
        }]),
    }
}

/// Motivo pelo qual soltar `from` em `to` é ilegal na Posição atual.
pub(crate) fn illegal_reason(
    position: &CanonicalPosition,
    from: Square,
    to: Square,
) -> IllegalReason {
    let side = position.side_to_move();
    match position.piece_at(from) {
        None => IllegalReason::EmptyOrigin,
        Some((color, _)) if color != side => IllegalReason::OpponentPiece,
        Some(_) if from == to => IllegalReason::SameSquare,
        Some(_) => IllegalReason::NotLegal {
            in_check: position.is_check(),
        },
    }
}

fn release(state: &GameState, from: Square, to: Square) -> Result<Vec<EventKind>, Rejection> {
    if state.pending_promotion.is_some() {
        return Err(Rejection::PromotionPending);
    }
    let position = state.position();
    let side = position.side_to_move();
    let candidates: Vec<Move> = position
        .legal_moves()
        .into_iter()
        .filter(|m| m.from() == from && m.to() == to)
        .collect();
    match candidates.as_slice() {
        [] => Ok(vec![EventKind::IllegalMoveRejected {
            side,
            from,
            to,
            reason: illegal_reason(position, from, to),
        }]),
        [mv] if mv.promotion().is_none() => completed(state, *mv, false, ASSISTED_AUTO),
        _ => Ok(vec![EventKind::PromotionPending { side, from, to }]),
    }
}

/// Valida a peça de promoção (D/T/B/C).
pub(crate) fn promotion_piece(piece: Piece) -> Result<Piece, Rejection> {
    if matches!(
        piece,
        Piece::Queen | Piece::Rook | Piece::Bishop | Piece::Knight
    ) {
        Ok(piece)
    } else {
        Err(Rejection::InvalidPromotionPiece(piece))
    }
}

fn choose_promotion(state: &GameState, piece: Piece) -> Result<Vec<EventKind>, Rejection> {
    let pending = state
        .pending_promotion
        .ok_or(Rejection::NoPromotionPending)?;
    promotion_piece(piece)?;
    completed(
        state,
        Move::new(pending.from, pending.to, Some(piece)),
        false,
        ASSISTED_AUTO,
    )
}

/// `MoveCompleted` (ou `MoveRedone`) e, se a Posição resultante encerra a Partida, `GameEnded`.
pub(crate) fn completed(
    state: &GameState,
    mv: Move,
    redo: bool,
    auto: AutoDraws,
) -> Result<Vec<EventKind>, Rejection> {
    let position = state.position();
    let side = position.side_to_move();
    let san = position.san(mv).map_err(|_| Rejection::Inconsistent)?;
    let next = position.play(mv).map_err(|_| Rejection::Inconsistent)?;
    let occurrences = state.occurrences(&next.repetition_key()) + 1;
    let mut events = vec![if redo {
        EventKind::MoveRedone { side, mv, san }
    } else {
        EventKind::MoveCompleted { side, mv, san }
    }];
    if let Some(outcome) = automatic_outcome(&next, occurrences, auto) {
        events.push(EventKind::GameEnded { outcome });
    }
    Ok(events)
}

fn undo(state: &GameState) -> Result<Vec<EventKind>, Rejection> {
    if state.pending_promotion.is_some() {
        return Err(Rejection::PromotionPending);
    }
    if state.outcome.is_some_and(|o| !o.reason.is_positional()) {
        return Err(Rejection::GameOver);
    }
    let Played { side, mv, .. } = state.played.last().ok_or(Rejection::NothingToUndo)?;
    Ok(vec![EventKind::MoveUndone {
        side: *side,
        mv: *mv,
    }])
}

fn redo(state: &GameState) -> Result<Vec<EventKind>, Rejection> {
    if state.pending_promotion.is_some() {
        return Err(Rejection::PromotionPending);
    }
    let next = state.redo.last().ok_or(Rejection::NothingToRedo)?;
    completed(state, next.mv, true, ASSISTED_AUTO)
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;

    fn pos(fen: &str) -> CanonicalPosition {
        CanonicalPosition::from_fen(fen).unwrap()
    }

    const BARE: &str = "4k3/8/8/8/8/8/8/R3K3 w - - 0 1";
    const NEITHER: AutoDraws = AutoDraws {
        threefold: false,
        fifty_move: false,
    };

    fn reason_of(o: Option<Outcome>) -> Option<EndReason> {
        o.map(|o| o.reason)
    }

    #[test]
    fn cinco_repeticoes_encerram_mesmo_sem_empates_automaticos() {
        assert_eq!(reason_of(automatic_outcome(&pos(BARE), 4, NEITHER)), None);
        assert_eq!(
            reason_of(automatic_outcome(&pos(BARE), 5, NEITHER)),
            Some(EndReason::FivefoldRepetition)
        );
    }

    #[test]
    fn tripla_e_cinquenta_dependem_da_politica() {
        let fifty = pos("4k3/8/8/8/8/8/8/R3K3 w - - 100 80");
        assert_eq!(reason_of(automatic_outcome(&fifty, 1, NEITHER)), None);
        assert_eq!(
            reason_of(automatic_outcome(&fifty, 1, ASSISTED_AUTO)),
            Some(EndReason::FiftyMoveRule)
        );
        assert_eq!(reason_of(automatic_outcome(&pos(BARE), 3, NEITHER)), None);
        assert_eq!(
            reason_of(automatic_outcome(&pos(BARE), 3, ASSISTED_AUTO)),
            Some(EndReason::ThreefoldRepetition)
        );
    }

    #[test]
    fn setenta_e_cinco_lances_valem_em_qualquer_politica() {
        let p = pos("4k3/8/8/8/8/8/8/R3K3 w - - 150 80");
        assert_eq!(
            reason_of(automatic_outcome(&p, 1, NEITHER)),
            Some(EndReason::SeventyFiveMoveRule)
        );
    }
}
