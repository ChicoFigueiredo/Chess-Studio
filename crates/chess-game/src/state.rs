//! Projeção do log: o estado da Partida é uma dobra (fold) dos Eventos.
//! `apply` é a única função que muda o estado; Comandos e `replay` passam por ela.

use crate::clock::ClockState;
use crate::event::{Event, EventKind, PAYLOAD_VERSION};
use crate::outcome::Outcome;
use crate::setup::{GameSetup, InfractionStage};
use chess_core::{CanonicalPosition, Color, Move, Piece, RepetitionKey, Square};
use std::fmt;

/// Lance já jogado, com a Posição resultante.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Played {
    pub side: Color,
    pub mv: Move,
    pub san: String,
    pub position: CanonicalPosition,
    pub key: RepetitionKey,
}

/// Peça tocada nesta vez (Obrigação de peça tocada, art. 4.3), na ordem do toque.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct Touched {
    pub square: Square,
    pub owner: Color,
    pub piece: Piece,
}

/// Peça solta numa casa ilegal, ainda provisória (DEC-32).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct ProvisionalRelease {
    pub side: Color,
    pub from: Square,
    pub to: Square,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PendingPromotion {
    pub side: Color,
    pub from: Square,
    pub to: Square,
}

#[derive(Debug, Clone)]
pub(crate) struct GameState {
    pub start: CanonicalPosition,
    pub start_key: RepetitionKey,
    pub played: Vec<Played>,
    /// Lances desfeitos, o próximo a refazer por último.
    pub redo: Vec<Played>,
    pub pending_promotion: Option<PendingPromotion>,
    pub draw_offer: Option<Color>,
    pub outcome: Option<Outcome>,
    pub with_undo: bool,
    pub last_at_ms: u64,
    /// Toques intencionais desta vez, em ordem (vazio fora das Políticas competitivas).
    pub touched: Vec<Touched>,
    /// Lance legal feito (art. 4.7) que ainda não foi concluído pelo relógio.
    pub made: Option<Played>,
    pub provisional_release: Option<ProvisionalRelease>,
    /// Infrações concluídas por lado (brancas, pretas).
    pub infractions: [u8; 2],
    /// Lance anunciado numa Reclamação incorreta: passa a ser obrigatório (art. 9.5.3).
    pub claimed_move: Option<Move>,
    pub paused: bool,
    pub clock: Option<ClockState>,
    pub stage: InfractionStage,
}

impl GameState {
    pub fn new(setup: &GameSetup) -> Self {
        Self {
            start: setup.start().clone(),
            start_key: setup.start().repetition_key(),
            played: Vec::new(),
            redo: Vec::new(),
            pending_promotion: None,
            draw_offer: None,
            outcome: None,
            with_undo: false,
            last_at_ms: 0,
            touched: Vec::new(),
            made: None,
            provisional_release: None,
            infractions: [0; 2],
            claimed_move: None,
            paused: false,
            clock: setup
                .time_control()
                .map(|tc| ClockState::new(tc, setup.start().side_to_move())),
            stage: setup.infraction_stage(),
        }
    }

    /// Posição canônica: a do último Lance **concluído**.
    pub fn position(&self) -> &CanonicalPosition {
        self.played.last().map_or(&self.start, |p| &p.position)
    }

    /// Posição para decisões de fim de Partida: a do Lance feito, se houver.
    pub fn effective_position(&self) -> &CanonicalPosition {
        self.made
            .as_ref()
            .map_or_else(|| self.position(), |m| &m.position)
    }

    /// Limpa o que vale só durante uma vez (toques, Lance feito, provisório).
    fn end_turn(&mut self) {
        self.touched.clear();
        self.made = None;
        self.provisional_release = None;
        self.claimed_move = None;
        self.pending_promotion = None;
        self.draw_offer = None;
    }

    /// Quantas vezes a Posição (por chave de repetição) já ocorreu, contando a inicial.
    pub fn occurrences(&self, key: &RepetitionKey) -> usize {
        usize::from(&self.start_key == key) + self.played.iter().filter(|p| &p.key == key).count()
    }
}

/// Evento que não cabe no estado (log corrompido ou fora de ordem).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReplayError {
    /// Índice do Evento problemático no log.
    pub index: usize,
    pub reason: String,
}

impl fmt::Display for ReplayError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "evento {} não se aplica: {}", self.index, self.reason)
    }
}

impl std::error::Error for ReplayError {}

fn bad(reason: &str) -> String {
    reason.to_owned()
}

fn move_into(state: &GameState, side: Color, mv: Move, san: &str) -> Result<Played, String> {
    let before = state.position();
    if before.side_to_move() != side {
        return Err(bad("lado do evento não é o lado da vez"));
    }
    let position = before.play(mv).map_err(|e| e.to_string())?;
    let key = position.repetition_key();
    Ok(Played {
        side,
        mv,
        san: san.to_owned(),
        position,
        key,
    })
}

fn color_idx(color: Color) -> usize {
    usize::from(color == Color::Black)
}

/// Aplica um Evento ao estado.
pub(crate) fn apply(state: &mut GameState, event: &Event) -> Result<(), String> {
    if event.version != PAYLOAD_VERSION {
        return Err(format!("versão de payload {} não suportada", event.version));
    }
    if event.at.mono_ms < state.last_at_ms {
        return Err(bad("instante anterior ao do evento anterior"));
    }
    let paused = state.paused;
    if let Some(clock) = &mut state.clock {
        clock.settle(event.at.mono_ms, paused);
    }
    match &event.kind {
        EventKind::MoveCompleted { side, mv, san } => {
            if state.made.as_ref().is_some_and(|m| m.mv != *mv) {
                return Err(bad("lance concluído difere do lance feito"));
            }
            let played = move_into(state, *side, *mv, san)?;
            state.played.push(played);
            state.redo.clear();
            state.end_turn();
            if let Some(clock) = &mut state.clock {
                clock.move_completed(*side, true);
            }
        }
        EventKind::MoveRedone { side, mv, san } => {
            if state.redo.last().map(|p| p.mv) != Some(*mv) {
                return Err(bad("lance refeito não é o próximo da pilha de refazer"));
            }
            let played = move_into(state, *side, *mv, san)?;
            state.redo.pop();
            state.played.push(played);
            state.draw_offer = None;
            if let Some(clock) = &mut state.clock {
                clock.move_completed(*side, false);
            }
        }
        EventKind::MoveUndone { side, mv } => {
            match state.played.last() {
                Some(p) if p.side == *side && p.mv == *mv => {}
                _ => return Err(bad("lance desfeito não é o último jogado")),
            }
            if let Some(p) = state.played.pop() {
                state.redo.push(p);
            }
            state.with_undo = true;
            state.pending_promotion = None;
            state.draw_offer = None;
            if state.outcome.is_some_and(|o| o.reason.is_positional()) {
                state.outcome = None;
            }
            if let Some(clock) = &mut state.clock {
                clock.running = Some(*side);
            }
        }
        EventKind::IllegalMoveRejected { .. } | EventKind::EngineMoveRejected { .. } => {}
        EventKind::PromotionPending { side, from, to } => {
            state.pending_promotion = Some(PendingPromotion {
                side: *side,
                from: *from,
                to: *to,
            });
        }
        EventKind::PromotionCancelled { .. } => state.pending_promotion = None,
        EventKind::DrawOffered { by } => state.draw_offer = Some(*by),
        EventKind::DrawDeclined { .. } => state.draw_offer = None,
        EventKind::Resigned { .. } => {}
        EventKind::GameEnded { outcome } => {
            state.outcome = Some(*outcome);
            state.pending_promotion = None;
            state.draw_offer = None;
            if let Some(clock) = &mut state.clock {
                clock.running = None;
            }
        }
        EventKind::PieceTouched {
            square,
            owner,
            piece,
            ..
        } => {
            if !state.touched.iter().any(|t| t.square == *square) {
                state.touched.push(Touched {
                    square: *square,
                    owner: *owner,
                    piece: *piece,
                });
            }
        }
        EventKind::PieceAdjusted { .. } => {}
        EventKind::MoveMade { side, mv, san } => {
            if state.made.is_some() {
                return Err(bad("já há um lance feito aguardando o relógio"));
            }
            let played = move_into(state, *side, *mv, san)?;
            state.made = Some(played);
            state.pending_promotion = None;
            state.provisional_release = None;
        }
        EventKind::IllegalMoveReleased { side, from, to, .. } => {
            state.provisional_release = Some(ProvisionalRelease {
                side: *side,
                from: *from,
                to: *to,
            });
        }
        EventKind::ReleaseRetracted { .. } => {
            if state.provisional_release.take().is_none() {
                return Err(bad("não há peça solta ilegalmente para retratar"));
            }
        }
        EventKind::InfractionRecorded {
            side,
            count,
            penalty,
            ..
        } => {
            let slot = &mut state.infractions[color_idx(*side)];
            if *count != *slot + 1 {
                return Err(bad("contagem de Infrações fora de ordem"));
            }
            *slot = *count;
            state.provisional_release = None;
            if let (Some(p), Some(clock)) = (penalty, &mut state.clock) {
                clock.add(p.to, p.ms);
            }
        }
        EventKind::Paused => {
            if state.paused {
                return Err(bad("a Partida já está em Pausa"));
            }
            state.paused = true;
        }
        EventKind::Resumed => {
            if !state.paused {
                return Err(bad("a Partida não está em Pausa"));
            }
            state.paused = false;
        }
        EventKind::FlagFell { .. } | EventKind::DrawClaimAccepted { .. } => {}
        EventKind::DrawClaimRejected {
            intended, penalty, ..
        } => {
            state.claimed_move = *intended;
            if let Some(clock) = &mut state.clock {
                clock.add(penalty.to, penalty.ms);
            }
        }
    }
    state.last_at_ms = event.at.mono_ms;
    Ok(())
}
