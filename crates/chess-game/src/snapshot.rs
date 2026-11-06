//! Leitura da Partida para a UI e para os testes. Dados puros, sem lógica de escrita.

use crate::outcome::Outcome;
use crate::setup::PolicyKind;
use crate::state::{GameState, PendingPromotion};
use chess_core::{CanonicalPosition, Color, Move, Piece, Square};

/// Lance do histórico legal. Lances ilegais nunca aparecem aqui.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HistoryEntry {
    /// Meio-lance, a partir de 1.
    pub ply: usize,
    pub side: Color,
    pub mv: Move,
    pub san: String,
}

/// Posição visual provisória: o que a UI desenha enquanto a Posição canônica
/// ainda não mudou (hoje, o peão na casa de promoção aguardando a peça).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProvisionalView {
    pub pending_promotion: PendingPromotion,
    /// Todas as peças desenhadas, em ordem de casa (a1, b1, ... h8).
    pub pieces: Vec<(Square, Color, Piece)>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Snapshot {
    pub policy: PolicyKind,
    /// Posição canônica (a única fonte de verdade das regras).
    pub position: CanonicalPosition,
    pub side_to_move: Color,
    pub in_check: bool,
    pub history: Vec<HistoryEntry>,
    pub outcome: Option<Outcome>,
    pub provisional: Option<ProvisionalView>,
    pub pending_draw_offer: Option<Color>,
    /// Partida marcada "com desfazer" (DEC-39).
    pub with_undo: bool,
    pub can_undo: bool,
    pub can_redo: bool,
}

impl Snapshot {
    pub(crate) fn from_state(policy: PolicyKind, state: &GameState) -> Self {
        let position = state.position().clone();
        let history = state
            .played
            .iter()
            .enumerate()
            .map(|(i, p)| HistoryEntry {
                ply: i + 1,
                side: p.side,
                mv: p.mv,
                san: p.san.clone(),
            })
            .collect();
        let provisional = state
            .pending_promotion
            .map(|pending| provisional_view(&position, pending));
        let open = state.outcome.is_none_or(|o| o.reason.is_positional());
        Self {
            policy,
            side_to_move: position.side_to_move(),
            in_check: position.is_check(),
            position,
            history,
            outcome: state.outcome,
            provisional,
            pending_draw_offer: state.draw_offer,
            with_undo: state.with_undo,
            can_undo: !state.played.is_empty() && open && state.pending_promotion.is_none(),
            can_redo: !state.redo.is_empty()
                && state.outcome.is_none()
                && state.pending_promotion.is_none(),
        }
    }

    /// Destinos legais da peça em `from` (para Auxílios visuais), sem repetições e
    /// em ordem de casa. Vazio se não há peça do lado da vez.
    pub fn legal_destinations(&self, from: Square) -> Vec<Square> {
        if self.outcome.is_some() {
            return Vec::new();
        }
        let mut dests: Vec<Square> = self
            .position
            .legal_moves()
            .into_iter()
            .filter(|m| m.from() == from)
            .map(|m| m.to())
            .collect();
        dests.sort_by_key(|s| (s.rank(), s.file()));
        dests.dedup();
        dests
    }
}

fn provisional_view(position: &CanonicalPosition, pending: PendingPromotion) -> ProvisionalView {
    let mut pieces = Vec::new();
    for rank in 0..8 {
        for file in 0..8 {
            let Some(sq) = Square::new(file, rank) else {
                continue;
            };
            if sq == pending.from {
                continue;
            }
            if sq == pending.to {
                pieces.push((sq, pending.side, Piece::Pawn));
            } else if let Some((color, piece)) = position.piece_at(sq) {
                pieces.push((sq, color, piece));
            }
        }
    }
    ProvisionalView {
        pending_promotion: pending,
        pieces,
    }
}
