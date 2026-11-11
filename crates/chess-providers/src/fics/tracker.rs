//! Do `style 12` ao retrato (#237): o FICS manda o tabuleiro depois de cada Lance, não a
//! lista de Lances. O rastreador guarda a lista desde o começo e só a estende quando o
//! núcleo de regras prova o caminho: do tabuleiro conhecido ao novo há **exatamente uma**
//! sequência de Lances legais que chega à mesma colocação, vez, roque e en passant.
//!
//! - Tabuleiro atrasado (meio-lance menor que o conhecido): ignorado, nada regride.
//! - Repetido (mesmo meio-lance e mesmo tabuleiro): só o relógio pode mudar.
//! - Um `style 12` perdido (linha cortada, por exemplo): o seguinte fica a dois meio-lances;
//!   a busca de profundidade 2 o recupera se o caminho for único.
//! - Qualquer outro caso (salto maior, caminho ambíguo, mesmo meio-lance com outro
//!   tabuleiro): inalcançável. A lista não muda; a sessão marca a dessincronização.

use super::parse::Style12;
use chess_core::{CanonicalPosition, Move};

/// Maior salto de meio-lances que a busca tenta fechar.
const MAX_GAP: u64 = 2;

/// O que um `style 12` fez com a lista de Lances.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Step {
    /// Atrasado: ignorado.
    Stale,
    /// Mesmo meio-lance e mesmo tabuleiro (relógio novo, talvez).
    Same,
    /// A lista ganhou Lances.
    Advanced,
    /// Não há caminho único até este tabuleiro: nada mudou.
    Unreachable { ply: u64 },
}

#[derive(Debug, Clone)]
pub(crate) struct Tracker {
    /// Meio-lance (desde a Posição inicial padrão) da Posição de partida.
    start_ply: u64,
    position: CanonicalPosition,
    moves: Vec<Move>,
}

/// Colocação, vez, roque e en passant (o FEN do núcleo sempre põe o en passant depois do
/// avanço duplo, como o `style 12`).
fn key(position: &CanonicalPosition) -> String {
    position
        .to_fen()
        .split(' ')
        .take(4)
        .collect::<Vec<_>>()
        .join(" ")
}

impl Tracker {
    pub fn new(position: CanonicalPosition, start_ply: u64) -> Self {
        Self {
            start_ply,
            position,
            moves: Vec::new(),
        }
    }

    pub fn known_ply(&self) -> u64 {
        self.start_ply + self.moves.len() as u64
    }

    pub fn position(&self) -> &CanonicalPosition {
        &self.position
    }

    /// Os Lances desde a Posição de partida, em UCI (o roque como o rei andando duas casas).
    pub fn uci(&self) -> Vec<String> {
        self.moves.iter().map(Move::to_string).collect()
    }

    pub fn apply(&mut self, board: &Style12) -> Step {
        let ply = board.ply();
        let known = self.known_ply();
        if ply < known {
            return Step::Stale;
        }
        let target = board.key();
        if ply == known {
            return if key(&self.position) == target {
                Step::Same
            } else {
                Step::Unreachable { ply }
            };
        }
        let gap = ply - known;
        if gap > MAX_GAP {
            return Step::Unreachable { ply };
        }
        match paths(&self.position, gap, &target).as_slice() {
            [(path, position)] => {
                self.moves.extend(path);
                self.position = position.clone();
                Step::Advanced
            }
            _ => Step::Unreachable { ply },
        }
    }
}

/// Os caminhos de `depth` Lances legais de `from` até `target` (para em dois: basta saber se
/// é único).
fn paths(
    from: &CanonicalPosition,
    depth: u64,
    target: &str,
) -> Vec<(Vec<Move>, CanonicalPosition)> {
    let mut found = Vec::new();
    walk(from, depth, target, &mut Vec::new(), &mut found);
    found
}

fn walk(
    position: &CanonicalPosition,
    depth: u64,
    target: &str,
    path: &mut Vec<Move>,
    found: &mut Vec<(Vec<Move>, CanonicalPosition)>,
) {
    for mv in position.legal_moves() {
        if found.len() > 1 {
            return;
        }
        let Ok(next) = position.play(mv) else {
            continue;
        };
        path.push(mv);
        if depth == 1 {
            if key(&next) == target {
                found.push((path.clone(), next));
            }
        } else {
            walk(&next, depth - 1, target, path, found);
        }
        path.pop();
    }
}
