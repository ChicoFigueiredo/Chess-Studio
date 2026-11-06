//! Leitura da Partida para a UI e para os testes. Dados puros, sem lógica de escrita.

use crate::clock::{ClockReading, ClockView};
use crate::event::DrawBasis;
use crate::outcome::Outcome;
use crate::setup::{GameSetup, PolicyKind};
use crate::state::{GameState, PendingPromotion};
use crate::time::Timestamp;
use crate::touch::obligation;
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

/// Peça tocada nesta vez (Obrigação de peça tocada), na ordem do toque.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TouchedPiece {
    pub square: Square,
    pub owner: Color,
    pub piece: Piece,
}

/// Lance legal feito (peça solta, art. 4.7) à espera do relógio. A Posição canônica
/// ainda é a anterior; `position_after` é o que a UI desenha.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MadeMove {
    pub side: Color,
    pub mv: Move,
    pub san: String,
    pub position_after: CanonicalPosition,
}

/// Peça solta numa casa ilegal (Simulação FIDE, DEC-32): fica lá, visualmente,
/// até o relógio (Infração) ou a retratação. Nunca é a Posição canônica.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProvisionalRelease {
    pub side: Color,
    pub from: Square,
    pub to: Square,
    /// Todas as peças desenhadas, em ordem de casa (a1, b1, ... h8).
    pub pieces: Vec<(Square, Color, Piece)>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Snapshot {
    pub policy: PolicyKind,
    /// Versão das Leis simuladas pela Política (RULE-01), se houver.
    pub rules_version: Option<&'static str>,
    /// Controle de tempo não clássico: os apêndices de rápido/blitz não são
    /// simulados (DEC-37, RULE-05).
    pub non_classical_notice: bool,
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
    /// Relógio no último Evento; consulte um instante com [`ClockView::at`].
    pub clock: Option<ClockView>,
    pub paused: bool,
    /// Toques intencionais desta vez, em ordem (Políticas competitivas).
    pub touched: Vec<TouchedPiece>,
    /// Casas de onde o Lance precisa sair para cumprir a Obrigação de peça tocada
    /// (vazio se não há obrigação).
    pub must_move_from: Vec<Square>,
    pub made_move: Option<MadeMove>,
    pub provisional_release: Option<ProvisionalRelease>,
    /// Infrações concluídas pelas brancas e pelas pretas.
    pub infractions_white: u8,
    pub infractions_black: u8,
    /// Empate reclamável agora pelo lado da vez (para a UI e para a Engine, B9).
    pub claimable_draw: Option<DrawBasis>,
}

impl Snapshot {
    pub(crate) fn from_state(setup: &GameSetup, state: &GameState) -> Self {
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
        let provisional_release = state.provisional_release.map(|r| {
            let (color, piece) = position.piece_at(r.from).unwrap_or((r.side, Piece::Pawn));
            ProvisionalRelease {
                side: r.side,
                from: r.from,
                to: r.to,
                pieces: pieces_with_moved(&position, r.from, r.to, color, piece),
            }
        });
        let open = state.outcome.is_none_or(|o| o.reason.is_positional());
        let touched: Vec<TouchedPiece> = state
            .touched
            .iter()
            .map(|t| TouchedPiece {
                square: t.square,
                owner: t.owner,
                piece: t.piece,
            })
            .collect();
        let mut must_move_from: Vec<Square> = if state.outcome.is_none() {
            obligation(&position, &state.touched, state.claimed_move)
                .map(|moves| moves.iter().map(|m| m.from()).collect())
                .unwrap_or_default()
        } else {
            Vec::new()
        };
        must_move_from.sort_by_key(|s| (s.rank(), s.file()));
        must_move_from.dedup();
        let quiet = state.touched.is_empty()
            && state.made.is_none()
            && state.provisional_release.is_none()
            && state.pending_promotion.is_none();
        let claimable_draw = (state.outcome.is_none() && quiet)
            .then(|| {
                if state.occurrences(&position.repetition_key()) >= 3 {
                    Some(DrawBasis::ThreefoldRepetition)
                } else if position.can_claim_fifty_move() {
                    Some(DrawBasis::FiftyMoveRule)
                } else {
                    None
                }
            })
            .flatten();
        let clock = state.clock.as_ref().map(|c| {
            ClockView::from_state(
                c,
                state.paused,
                state.outcome.is_some(),
                state.effective_position(),
            )
        });
        Self {
            policy: setup.policy(),
            rules_version: setup.rules_version(),
            non_classical_notice: setup.non_classical_notice(),
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
            clock,
            paused: state.paused,
            touched,
            must_move_from,
            made_move: state.made.as_ref().map(|m| MadeMove {
                side: m.side,
                mv: m.mv,
                san: m.san.clone(),
                position_after: m.position.clone(),
            }),
            provisional_release,
            infractions_white: state.infractions[0],
            infractions_black: state.infractions[1],
            claimable_draw,
        }
    }

    /// Relógio em `ts` (atalho para `clock.at(ts)`); `None` sem controle de tempo.
    pub fn clock_at(&self, ts: Timestamp) -> Option<ClockReading> {
        self.clock.map(|c| c.at(ts))
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

/// Peças desenhadas com a peça de `from` em `to` (sem validar nada).
fn pieces_with_moved(
    position: &CanonicalPosition,
    from: Square,
    to: Square,
    color: Color,
    piece: Piece,
) -> Vec<(Square, Color, Piece)> {
    let mut pieces = Vec::new();
    for rank in 0..8 {
        for file in 0..8 {
            let Some(sq) = Square::new(file, rank) else {
                continue;
            };
            if sq == from {
                continue;
            }
            if sq == to {
                pieces.push((sq, color, piece));
            } else if let Some((c, p)) = position.piece_at(sq) {
                pieces.push((sq, c, p));
            }
        }
    }
    pieces
}

fn provisional_view(position: &CanonicalPosition, pending: PendingPromotion) -> ProvisionalView {
    ProvisionalView {
        pending_promotion: pending,
        pieces: pieces_with_moved(
            position,
            pending.from,
            pending.to,
            pending.side,
            Piece::Pawn,
        ),
    }
}
