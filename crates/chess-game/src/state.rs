//! Projeção do log: o estado da Partida é uma dobra (fold) dos Eventos.
//! `apply` é a única função que muda o estado; Comandos e `replay` passam por ela.

use crate::event::{Event, EventKind, PAYLOAD_VERSION};
use crate::outcome::Outcome;
use crate::setup::GameSetup;
use chess_core::{CanonicalPosition, Color, Move, RepetitionKey, Square};
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
        }
    }

    pub fn position(&self) -> &CanonicalPosition {
        self.played.last().map_or(&self.start, |p| &p.position)
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

fn move_into(
    state: &mut GameState,
    side: Color,
    mv: Move,
    san: &str,
) -> Result<Played, String> {
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

/// Aplica um Evento ao estado.
pub(crate) fn apply(state: &mut GameState, event: &Event) -> Result<(), String> {
    if event.version != PAYLOAD_VERSION {
        return Err(format!("versão de payload {} não suportada", event.version));
    }
    if event.at.mono_ms < state.last_at_ms {
        return Err(bad("instante anterior ao do evento anterior"));
    }
    match &event.kind {
        EventKind::MoveCompleted { side, mv, san } => {
            let played = move_into(state, *side, *mv, san)?;
            state.played.push(played);
            state.redo.clear();
            state.pending_promotion = None;
            state.draw_offer = None;
        }
        EventKind::MoveRedone { side, mv, san } => {
            if state.redo.last().map(|p| p.mv) != Some(*mv) {
                return Err(bad("lance refeito não é o próximo da pilha de refazer"));
            }
            let played = move_into(state, *side, *mv, san)?;
            state.redo.pop();
            state.played.push(played);
            state.draw_offer = None;
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
        }
        EventKind::IllegalMoveRejected { .. } => {}
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
        }
    }
    state.last_at_ms = event.at.mono_ms;
    Ok(())
}
