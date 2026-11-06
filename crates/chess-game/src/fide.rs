//! Núcleo das Políticas competitivas e a Simulação FIDE (Leis de 1/1/2023).
//!
//! Etapas de uma vez do jogador: **toque** (art. 4.3, Obrigação de peça tocada),
//! **soltura** (Lance feito, art. 4.7; ou peça provisória em casa ilegal, DEC-32) e
//! **conclusão** pelo relógio (art. 6.2.1), exceto quando o Lance encerra a Partida
//! (art. 6.2.1.1). Lance ilegal concluído é Infração (art. 7.5).
//!
//! A Eliminação Imediata (B8) reaproveitará este núcleo: `configurable_stage`
//! habilita a etapa de Infração do setup (relógio | soltura, DEC-36) e a
//! consequência da Infração é a única outra peça a trocar. A Simulação FIDE tem a
//! etapa fixa no relógio e as consequências do art. 7.5.5.
//!
//! Simplificação assumida: o árbitro é a própria Política, que detecta a Infração
//! no instante em que o relógio é acionado (não depende de reclamação do adversário).

use crate::clock::TWO_MINUTES_MS;
use crate::command::Command;
use crate::event::{DrawBasis, EventKind, InfractionCause, TimePenalty};
use crate::outcome::{EndReason, GameResult, Outcome};
use crate::policy::{
    ArbitrationPolicy, AutoDraws, automatic_outcome, common, completed, gate_pause, illegal_reason,
    promotion_piece,
};
use crate::rejection::Rejection;
use crate::setup::InfractionStage;
use crate::state::{GameState, Touched};
use crate::touch::obligation;
use chess_core::{Color, GameStatus, Move, Piece, Square};

/// Na Simulação FIDE, tripla e 50 lances exigem Reclamação (DEC-38); 5 repetições,
/// 75 lances, mate, afogamento e posição morta encerram sozinhos.
const FIDE_AUTO: AutoDraws = AutoDraws {
    threefold: false,
    fifty_move: false,
};

pub(crate) struct Competitive {
    /// Se verdadeiro, a etapa de Infração vem do setup (Eliminação Imediata, B8).
    pub configurable_stage: bool,
}

pub(crate) static FIDE_SIMULATION: Competitive = Competitive {
    configurable_stage: false,
};

impl ArbitrationPolicy for Competitive {
    fn decide(&self, state: &GameState, command: &Command) -> Result<Vec<EventKind>, Rejection> {
        let kind = command.kind();
        if matches!(command, Command::Undo | Command::Redo) {
            return Err(Rejection::NotSupportedByPolicy(kind));
        }
        if state.outcome.is_some() {
            return Err(Rejection::GameOver);
        }
        gate_pause(state, command)?;
        if let Some(result) = common(state, command, FIDE_AUTO) {
            return result;
        }
        match command {
            Command::Touch { square } => self.touch(state, *square),
            Command::Adjust { square } => self.adjust(state, *square),
            Command::Release { from, to } => self.release(state, *from, *to),
            Command::ChoosePromotion { piece } => self.choose_promotion(state, *piece),
            Command::RetractMove => retract(state),
            Command::PressClock => Ok(self.press_clock(state)),
            Command::ClaimDraw => claim(state),
            Command::ClaimDrawWithMove { mv } => claim_with_move(state, *mv),
            _ => Err(Rejection::Inconsistent),
        }
    }
}

/// Só se toca, ajusta ou solta enquanto a vez está "aberta": sem Lance feito, peça
/// provisória ou promoção pendente.
fn guard_turn_open(state: &GameState) -> Result<(), Rejection> {
    if state.pending_promotion.is_some() {
        return Err(Rejection::PromotionPending);
    }
    if state.made.is_some() {
        return Err(Rejection::MoveAlreadyMade);
    }
    if state.provisional_release.is_some() {
        return Err(Rejection::ProvisionalReleasePending);
    }
    Ok(())
}

/// Casas de onde o Lance precisa sair, sem repetição.
fn froms(moves: &[Move]) -> Vec<Square> {
    let mut v: Vec<Square> = moves.iter().map(|m| m.from()).collect();
    v.sort_by_key(|s| (s.rank(), s.file()));
    v.dedup();
    v
}

impl Competitive {
    fn stage(&self, state: &GameState) -> InfractionStage {
        if self.configurable_stage {
            state.stage
        } else {
            InfractionStage::Clock
        }
    }

    /// Toque intencional (art. 4.3): cria Obrigação de peça tocada, de peça própria
    /// ou do adversário.
    fn touch(&self, state: &GameState, square: Square) -> Result<Vec<EventKind>, Rejection> {
        guard_turn_open(state)?;
        let position = state.position();
        let (owner, piece) = position
            .piece_at(square)
            .ok_or(Rejection::NoPieceAt(square))?;
        Ok(vec![EventKind::PieceTouched {
            side: position.side_to_move(),
            square,
            owner,
            piece,
            implicit: false,
        }])
    }

    /// Ajuste (art. 4.2): sem obrigação.
    fn adjust(&self, state: &GameState, square: Square) -> Result<Vec<EventKind>, Rejection> {
        guard_turn_open(state)?;
        let position = state.position();
        position
            .piece_at(square)
            .ok_or(Rejection::NoPieceAt(square))?;
        Ok(vec![EventKind::PieceAdjusted {
            side: position.side_to_move(),
            square,
        }])
    }

    fn release(
        &self,
        state: &GameState,
        from: Square,
        to: Square,
    ) -> Result<Vec<EventKind>, Rejection> {
        guard_turn_open(state)?;
        let position = state.position();
        let side = position.side_to_move();
        let (owner, piece) = position.piece_at(from).ok_or(Rejection::NoPieceAt(from))?;

        // Soltar sem Touch prévio é toque implícito da peça solta.
        let implicit = !state.touched.iter().any(|t| t.square == from);
        let mut touched: Vec<Touched> = state.touched.clone();
        if implicit {
            touched.push(Touched {
                square: from,
                owner,
                piece,
            });
        }
        let candidates: Vec<Move> = position
            .legal_moves()
            .into_iter()
            .filter(|m| m.from() == from && m.to() == to)
            .collect();

        // Obrigação de peça tocada: a peça solta tem de ser a que obriga, e um Lance
        // legal incompatível com o toque (ex.: roque com a torre tocada primeiro) é
        // recusado, não punido.
        if let Some(allowed) = obligation(position, &touched, state.claimed_move) {
            let from_ok = allowed.iter().any(|m| m.from() == from);
            let dest_ok = candidates.is_empty()
                || candidates.iter().any(|c| {
                    allowed
                        .iter()
                        .any(|m| m.from() == c.from() && m.to() == c.to())
                });
            if !from_ok || !dest_ok {
                return Err(Rejection::TouchedPieceObligation {
                    must_move_from: froms(&allowed),
                });
            }
        }

        let mut events = Vec::new();
        if implicit {
            events.push(EventKind::PieceTouched {
                side,
                square: from,
                owner,
                piece,
                implicit: true,
            });
        }
        match candidates.as_slice() {
            [] => {
                let reason = illegal_reason(position, from, to);
                if self.stage(state) == InfractionStage::Release {
                    events.extend(
                        self.infraction(state, InfractionCause::IllegalMove { from, to, reason }),
                    );
                } else {
                    events.push(EventKind::IllegalMoveReleased {
                        side,
                        from,
                        to,
                        reason,
                    });
                }
            }
            [mv] if mv.promotion().is_none() => events.extend(made(state, *mv)?),
            _ => events.push(EventKind::PromotionPending { side, from, to }),
        }
        Ok(events)
    }

    fn choose_promotion(
        &self,
        state: &GameState,
        piece: Piece,
    ) -> Result<Vec<EventKind>, Rejection> {
        let pending = state
            .pending_promotion
            .ok_or(Rejection::NoPromotionPending)?;
        promotion_piece(piece)?;
        made(state, Move::new(pending.from, pending.to, Some(piece)))
    }

    /// Acionar o relógio (art. 6.2.1): conclui o Lance feito; nos demais casos é um
    /// Lance ilegal concluído (arts. 7.5.1 a 7.5.3).
    fn press_clock(&self, state: &GameState) -> Vec<EventKind> {
        if let Some(m) = &state.made {
            return vec![EventKind::MoveCompleted {
                side: m.side,
                mv: m.mv,
                san: m.san.clone(),
            }];
        }
        if let Some(r) = state.provisional_release {
            let reason = illegal_reason(state.position(), r.from, r.to);
            return self.infraction(
                state,
                InfractionCause::IllegalMove {
                    from: r.from,
                    to: r.to,
                    reason,
                },
            );
        }
        if let Some(p) = state.pending_promotion {
            // Art. 7.5.2: o peão vira dama e valem as penalidades do art. 7.5.5.
            let mut events = self.infraction(
                state,
                InfractionCause::PromotionNotCompleted {
                    from: p.from,
                    to: p.to,
                },
            );
            if !matches!(events.last(), Some(EventKind::GameEnded { .. })) {
                let queen = Move::new(p.from, p.to, Some(Piece::Queen));
                if let Ok(done) = completed(state, queen, false, FIDE_AUTO) {
                    events.extend(done);
                }
            }
            return events;
        }
        self.infraction(state, InfractionCause::ClockPressedWithoutMove)
    }

    /// Infração (art. 7.5.5): a 1ª dá 2 minutos ao adversário; a 2ª perde a Partida,
    /// salvo se o adversário não puder dar mate por nenhuma série de Lances legais
    /// (empate). A posição regular é a da última vez concluída; não há o que restaurar
    /// além de descartar a peça provisória (aplicado em `apply`).
    fn infraction(&self, state: &GameState, cause: InfractionCause) -> Vec<EventKind> {
        let position = state.position();
        let side = position.side_to_move();
        let count = state.infractions[usize::from(side == Color::Black)] + 1;
        let stage = self.stage(state);
        if count == 1 {
            return vec![EventKind::InfractionRecorded {
                side,
                cause,
                count,
                stage,
                penalty: Some(TimePenalty {
                    to: side.opposite(),
                    ms: TWO_MINUTES_MS,
                }),
            }];
        }
        let winner = side.opposite();
        let (result, reason) = if position.can_checkmate(winner) {
            (
                match winner {
                    Color::White => GameResult::WhiteWins,
                    Color::Black => GameResult::BlackWins,
                },
                EndReason::SecondIllegalMove,
            )
        } else {
            (GameResult::Draw, EndReason::SecondIllegalMoveDraw)
        };
        vec![
            EventKind::InfractionRecorded {
                side,
                cause,
                count,
                stage,
                penalty: None,
            },
            EventKind::GameEnded {
                outcome: Outcome { result, reason },
            },
        ]
    }
}

/// Lance legal feito (peça solta); se encerra a Partida (mate, afogamento, posição
/// morta, 5 repetições, 75 lances), já é concluído sem acionar o relógio (6.2.1.1).
fn made(state: &GameState, mv: Move) -> Result<Vec<EventKind>, Rejection> {
    let position = state.position();
    let side = position.side_to_move();
    let san = position.san(mv).map_err(|_| Rejection::Inconsistent)?;
    let next = position.play(mv).map_err(|_| Rejection::Inconsistent)?;
    let occurrences = state.occurrences(&next.repetition_key()) + 1;
    let mut events = vec![EventKind::MoveMade {
        side,
        mv,
        san: san.clone(),
    }];
    if let Some(outcome) = automatic_outcome(&next, occurrences, FIDE_AUTO) {
        events.push(EventKind::MoveCompleted { side, mv, san });
        events.push(EventKind::GameEnded { outcome });
    }
    Ok(events)
}

/// Retratar: a peça solta em casa ilegal volta (a obrigação fica); a promoção ainda
/// sem peça escolhida pode ser desfeita (o Lance só está feito quando a nova peça é
/// colocada, art. 4.7.3). Lance legal feito não se retrata (art. 4.7).
fn retract(state: &GameState) -> Result<Vec<EventKind>, Rejection> {
    if let Some(r) = state.provisional_release {
        return Ok(vec![EventKind::ReleaseRetracted { side: r.side }]);
    }
    if let Some(p) = state.pending_promotion {
        return Ok(vec![EventKind::PromotionCancelled { side: p.side }]);
    }
    if state.made.is_some() {
        return Err(Rejection::MoveAlreadyMade);
    }
    Err(Rejection::NothingToRetract)
}

/// Reclamação só antes de qualquer toque na vez (art. 9.4) e com a vez "aberta".
fn guard_claim(state: &GameState) -> Result<(), Rejection> {
    guard_turn_open(state)?;
    if !state.touched.is_empty() {
        return Err(Rejection::DrawClaimRightLost);
    }
    Ok(())
}

fn rejected_claim(state: &GameState, intended: Option<Move>) -> Vec<EventKind> {
    let by = state.position().side_to_move();
    vec![EventKind::DrawClaimRejected {
        by,
        intended,
        penalty: TimePenalty {
            to: by.opposite(),
            ms: TWO_MINUTES_MS,
        },
    }]
}

fn draw_end(basis: DrawBasis) -> EventKind {
    EventKind::GameEnded {
        outcome: Outcome {
            result: GameResult::Draw,
            reason: match basis {
                DrawBasis::ThreefoldRepetition => EndReason::ThreefoldRepetition,
                DrawBasis::FiftyMoveRule => EndReason::FiftyMoveRule,
            },
        },
    }
}

/// Reclamação sobre a Posição que acabou de surgir (arts. 9.2.2 e 9.3.2); o
/// reclamante é o lado da vez. Correta: empate imediato (9.5.2). Incorreta: 2 minutos
/// ao adversário e a Partida continua (9.5.3).
fn claim(state: &GameState) -> Result<Vec<EventKind>, Rejection> {
    guard_claim(state)?;
    let position = state.position();
    let basis = if state.occurrences(&position.repetition_key()) >= 3 {
        Some(DrawBasis::ThreefoldRepetition)
    } else if position.can_claim_fifty_move() {
        Some(DrawBasis::FiftyMoveRule)
    } else {
        None
    };
    Ok(match basis {
        Some(basis) => vec![
            EventKind::DrawClaimAccepted {
                by: position.side_to_move(),
                basis,
                intended: None,
            },
            draw_end(basis),
        ],
        None => rejected_claim(state, None),
    })
}

/// Reclamação com Lance anunciado (arts. 9.2.1 e 9.3.1): vale a Posição que o Lance
/// produziria. Correta: o Lance se completa (art. 6.2.1.1) e a Partida empata.
/// Incorreta: 2 minutos ao adversário e o Lance anunciado passa a ser obrigatório.
fn claim_with_move(state: &GameState, mv: Move) -> Result<Vec<EventKind>, Rejection> {
    guard_claim(state)?;
    let position = state.position();
    let side = position.side_to_move();
    if !position.legal_moves().contains(&mv) {
        return Err(Rejection::ClaimedMoveIllegal);
    }
    let san = position.san(mv).map_err(|_| Rejection::Inconsistent)?;
    let next = position.play(mv).map_err(|_| Rejection::Inconsistent)?;
    let occurrences = state.occurrences(&next.repetition_key()) + 1;
    let basis = if matches!(next.status(), GameStatus::Checkmate { .. }) {
        None // o mate prevalece sobre a Reclamação
    } else if occurrences >= 3 {
        Some(DrawBasis::ThreefoldRepetition)
    } else if next.can_claim_fifty_move() {
        Some(DrawBasis::FiftyMoveRule)
    } else {
        None
    };
    Ok(match basis {
        Some(basis) => vec![
            EventKind::DrawClaimAccepted {
                by: side,
                basis,
                intended: Some(mv),
            },
            EventKind::MoveCompleted { side, mv, san },
            draw_end(basis),
        ],
        None => rejected_claim(state, Some(mv)),
    })
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;
    use crate::setup::{GameSetup, PolicyKind};

    fn state_for(stage: InfractionStage) -> GameState {
        GameState::new(
            &GameSetup::standard(PolicyKind::FideSimulation).with_infraction_stage(stage),
        )
    }

    fn sq(s: &str) -> Square {
        s.parse().unwrap()
    }

    #[test]
    fn etapa_soltura_vira_infracao_ao_soltar_so_quando_configuravel() {
        let rook_to_b2 = Command::Release {
            from: sq("a1"),
            to: sq("b2"),
        };
        let eliminate_on_release = Competitive {
            configurable_stage: true,
        };
        let state = state_for(InfractionStage::Release);
        let events = eliminate_on_release.decide(&state, &rook_to_b2).unwrap();
        assert!(matches!(
            events.as_slice(),
            [
                EventKind::PieceTouched { .. },
                EventKind::InfractionRecorded {
                    stage: InfractionStage::Release,
                    count: 1,
                    ..
                }
            ]
        ));
        // A Simulação FIDE (não configurável) ignora o parâmetro.
        let events = FIDE_SIMULATION.decide(&state, &rook_to_b2).unwrap();
        assert!(matches!(
            events.as_slice(),
            [
                EventKind::PieceTouched { .. },
                EventKind::IllegalMoveReleased { .. }
            ]
        ));
    }
}
