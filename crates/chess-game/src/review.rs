//! Revisão Lance a Lance: leitura de uma Posição do histórico, sem tocar a Partida.
//! Revisar não é desfazer (DEC-39): nenhum Evento é gerado e o resultado não muda.

use crate::snapshot::HistoryEntry;
use crate::state::GameState;
use chess_core::{CanonicalPosition, Color};

/// A Partida como estava após `ply` meios-lances (0 = Posição inicial).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReviewView {
    pub ply: usize,
    /// Total de meios-lances do histórico legal atual.
    pub total_plies: usize,
    pub position: CanonicalPosition,
    pub side_to_move: Color,
    pub in_check: bool,
    /// Lance que levou a esta Posição; `None` na Posição inicial.
    pub last_move: Option<HistoryEntry>,
    pub is_start: bool,
    pub is_end: bool,
}

impl ReviewView {
    pub(crate) fn at(state: &GameState, ply: usize) -> Option<Self> {
        let total = state.played.len();
        if ply > total {
            return None;
        }
        let (position, last_move) = match ply.checked_sub(1) {
            None => (state.start.clone(), None),
            Some(i) => {
                let p = &state.played[i];
                (
                    p.position.clone(),
                    Some(HistoryEntry {
                        ply,
                        side: p.side,
                        mv: p.mv,
                        san: p.san.clone(),
                    }),
                )
            }
        };
        Some(Self {
            ply,
            total_plies: total,
            side_to_move: position.side_to_move(),
            in_check: position.is_check(),
            position,
            last_move,
            is_start: ply == 0,
            is_end: ply == total,
        })
    }
}
