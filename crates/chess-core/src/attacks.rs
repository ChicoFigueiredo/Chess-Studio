//! Ataques, ameaças e dica de Lance: consultas puras sobre a Posição, usadas pelo HUD.
//!
//! "Ataque" é geométrico (cravamentos não contam): uma peça ataca as casas que alcançaria
//! se a casa fosse capturável, e uma casa ocupada por peça do próprio lado fica atacada
//! (a peça está defendida). Nada aqui altera a Posição.

use crate::position::CanonicalPosition;
use crate::status::GameStatus;
use crate::types::{Color, Move, Piece, Square};

const KNIGHT: [(i8, i8); 8] = [
    (1, 2),
    (2, 1),
    (2, -1),
    (1, -2),
    (-1, -2),
    (-2, -1),
    (-2, 1),
    (-1, 2),
];
const KING: [(i8, i8); 8] = [
    (1, 0),
    (1, 1),
    (0, 1),
    (-1, 1),
    (-1, 0),
    (-1, -1),
    (0, -1),
    (1, -1),
];
const ORTHOGONAL: [(i8, i8); 4] = [(1, 0), (0, 1), (-1, 0), (0, -1)];
const DIAGONAL: [(i8, i8); 4] = [(1, 1), (-1, 1), (-1, -1), (1, -1)];

fn offset(from: Square, df: i8, dr: i8) -> Option<Square> {
    let file = u8::try_from(i8::try_from(from.file()).ok()? + df).ok()?;
    let rank = u8::try_from(i8::try_from(from.rank()).ok()? + dr).ok()?;
    Square::new(file, rank)
}

fn all_squares() -> impl Iterator<Item = Square> {
    (0..8u8).flat_map(|rank| (0..8u8).filter_map(move |file| Square::new(file, rank)))
}

fn index(square: Square) -> usize {
    usize::from(square.rank()) * 8 + usize::from(square.file())
}

/// Valor de material em centipeões, só para ordenar dicas.
fn value(piece: Piece) -> i32 {
    match piece {
        Piece::Pawn => 100,
        Piece::Knight | Piece::Bishop => 300,
        Piece::Rook => 500,
        Piece::Queen => 900,
        Piece::King => 20_000,
    }
}

impl CanonicalPosition {
    /// Casas que a peça de `from` ataca (vazio se não há peça).
    fn attacks_from(&self, from: Square) -> Vec<Square> {
        let Some((color, piece)) = self.piece_at(from) else {
            return Vec::new();
        };
        let step = |deltas: &[(i8, i8)]| -> Vec<Square> {
            deltas
                .iter()
                .filter_map(|&(df, dr)| offset(from, df, dr))
                .collect()
        };
        match piece {
            Piece::Knight => step(&KNIGHT),
            Piece::King => step(&KING),
            Piece::Pawn => {
                let dr = if color == Color::White { 1 } else { -1 };
                step(&[(-1, dr), (1, dr)])
            }
            Piece::Bishop | Piece::Rook | Piece::Queen => {
                let mut rays: Vec<(i8, i8)> = Vec::new();
                if piece != Piece::Bishop {
                    rays.extend(ORTHOGONAL);
                }
                if piece != Piece::Rook {
                    rays.extend(DIAGONAL);
                }
                let mut out = Vec::new();
                for (df, dr) in rays {
                    let mut at = from;
                    while let Some(next) = offset(at, df, dr) {
                        out.push(next);
                        if self.piece_at(next).is_some() {
                            break;
                        }
                        at = next;
                    }
                }
                out
            }
        }
    }

    fn squares_of(&self, side: Color) -> impl Iterator<Item = Square> + '_ {
        all_squares().filter(move |s| self.piece_at(*s).is_some_and(|(c, _)| c == side))
    }

    /// Casas atacadas por alguma peça de `by`, em ordem de casa (a1, b1, ... h8).
    pub fn attacked_squares(&self, by: Color) -> Vec<Square> {
        let mut hit = [false; 64];
        for from in self.squares_of(by) {
            for target in self.attacks_from(from) {
                hit[index(target)] = true;
            }
        }
        all_squares().filter(|s| hit[index(*s)]).collect()
    }

    /// Casas das peças de `by` que atacam `square`.
    pub fn attackers_of(&self, square: Square, by: Color) -> Vec<Square> {
        self.squares_of(by)
            .filter(|s| self.attacks_from(*s).contains(&square))
            .collect()
    }

    /// Peças de `side` (rei incluído) atacadas por alguma peça do adversário.
    pub fn threatened_pieces(&self, side: Color) -> Vec<Square> {
        let attacked = self.attacked_squares(side.opposite());
        self.squares_of(side)
            .filter(|s| attacked.contains(s))
            .collect()
    }

    /// Peças de `side` (menos o rei) que nenhuma outra peça do mesmo lado defende.
    pub fn undefended_pieces(&self, side: Color) -> Vec<Square> {
        self.squares_of(side)
            .filter(|s| self.piece_at(*s).is_some_and(|(_, p)| p != Piece::King))
            .filter(|s| self.attackers_of(*s, side).is_empty())
            .collect()
    }

    /// Dica pedagógica para o lado a jogar: mate em um, senão a captura de maior ganho
    /// líquido. `None` quando nada é claramente bom (a dica não inventa um Lance).
    pub fn suggest_hint(&self) -> Option<Move> {
        let side = self.side_to_move();
        let mut moves = self.legal_moves();
        moves.sort_by_key(|m| (index(m.from()), index(m.to())));
        let mut best: Option<(i32, Move)> = None;
        for mv in moves {
            let Ok(after) = self.play(mv) else { continue };
            let score = if matches!(after.status(), GameStatus::Checkmate { .. }) {
                1_000_000
            } else {
                let gained = self.piece_at(mv.to()).map_or(0, |(_, p)| value(p))
                    + mv.promotion().map_or(0, |p| value(p) - value(Piece::Pawn));
                let mover = self.piece_at(mv.from()).map_or(0, |(_, p)| value(p));
                let at_risk = !after.attackers_of(mv.to(), side.opposite()).is_empty();
                gained - if at_risk { mover } else { 0 }
            };
            if score > 0 && best.is_none_or(|(b, _)| score > b) {
                best = Some((score, mv));
            }
        }
        best.map(|(_, mv)| mv)
    }
}
