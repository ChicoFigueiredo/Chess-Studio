//! Disposição visual das peças, compartilhada por todas as Apresentações.
//!
//! Hoje é a posição inicial fixa (a Partida real chega em marcos seguintes). Como as três
//! Apresentações leem esta mesma lista, alternar entre elas nunca altera a posição exibida.

use chess_core::Square;

/// Tipo de peça (apenas visual; as regras vivem em `chess-core`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    Pawn,
    Knight,
    Bishop,
    Rook,
    Queen,
    King,
}

impl Kind {
    /// Nome usado pelos nós do glTF (`piece_<nome>_<cor>[_NN]`).
    pub fn gltf_name(self) -> &'static str {
        match self {
            Kind::Pawn => "pawn",
            Kind::Knight => "knight",
            Kind::Bishop => "bishop",
            Kind::Rook => "rook",
            Kind::Queen => "queen",
            Kind::King => "king",
        }
    }

    /// Glifo Unicode preenchido (U+265A..U+265F) usado no 2D técnico.
    pub fn glyph(self) -> char {
        match self {
            Kind::King => '\u{265A}',
            Kind::Queen => '\u{265B}',
            Kind::Rook => '\u{265C}',
            Kind::Bishop => '\u{265D}',
            Kind::Knight => '\u{265E}',
            Kind::Pawn => '\u{265F}',
        }
    }
}

/// Uma peça exibida em uma casa.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PieceSlot {
    pub square: Square,
    pub white: bool,
    pub kind: Kind,
}

const BACK_RANK: [Kind; 8] = [
    Kind::Rook,
    Kind::Knight,
    Kind::Bishop,
    Kind::Queen,
    Kind::King,
    Kind::Bishop,
    Kind::Knight,
    Kind::Rook,
];

/// As 32 peças da posição inicial, ordenadas por cor, fila e coluna.
pub fn initial_layout() -> Vec<PieceSlot> {
    let mut out = Vec::with_capacity(32);
    for (white, back, pawns) in [(true, 0u8, 1u8), (false, 7, 6)] {
        for file in 0..8u8 {
            for (rank, kind) in [(back, BACK_RANK[usize::from(file)]), (pawns, Kind::Pawn)] {
                if let Some(square) = Square::new(file, rank) {
                    out.push(PieceSlot {
                        square,
                        white,
                        kind,
                    });
                }
            }
        }
    }
    out
}

/// Encontra o slot de um nó do glTF a partir do nome (`piece_rook_white_02`).
/// O ordinal (1-based) conta as peças iguais da esquerda para a direita (coluna a..h).
pub fn slot_for_node(name: &str) -> Option<PieceSlot> {
    let mut parts = name.strip_prefix("piece_")?.split('_');
    let kind_name = parts.next()?;
    let white = match parts.next()? {
        "white" => true,
        "black" => false,
        _ => return None,
    };
    let ordinal: usize = parts.next().map_or(Some(1), |n| n.parse().ok())?;
    let kind = [
        Kind::Pawn,
        Kind::Knight,
        Kind::Bishop,
        Kind::Rook,
        Kind::Queen,
        Kind::King,
    ]
    .into_iter()
    .find(|k| k.gltf_name() == kind_name)?;
    let mut same: Vec<PieceSlot> = initial_layout()
        .into_iter()
        .filter(|s| s.white == white && s.kind == kind)
        .collect();
    same.sort_by_key(|s| s.square.file());
    same.get(ordinal.checked_sub(1)?).copied()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn name(s: PieceSlot) -> String {
        s.square.to_string()
    }

    #[test]
    fn initial_layout_has_32_pieces_on_distinct_squares() {
        let l = initial_layout();
        assert_eq!(l.len(), 32);
        let mut squares: Vec<String> = l.iter().map(|s| name(*s)).collect();
        squares.sort();
        squares.dedup();
        assert_eq!(squares.len(), 32);
        assert_eq!(l.iter().filter(|s| s.white).count(), 16);
    }

    #[test]
    fn kings_on_e_file_and_queens_on_d_file() {
        let l = initial_layout();
        let at = |kind, white| {
            l.iter()
                .find(|s| s.kind == kind && s.white == white)
                .map(|s| name(*s))
        };
        assert_eq!(at(Kind::King, true).as_deref(), Some("e1"));
        assert_eq!(at(Kind::King, false).as_deref(), Some("e8"));
        assert_eq!(at(Kind::Queen, true).as_deref(), Some("d1"));
        assert_eq!(at(Kind::Queen, false).as_deref(), Some("d8"));
    }

    #[test]
    fn gltf_node_names_map_to_squares() {
        let n = |s: &str| slot_for_node(s).map(name);
        assert_eq!(n("piece_rook_white_01").as_deref(), Some("a1"));
        assert_eq!(n("piece_rook_white_02").as_deref(), Some("h1"));
        assert_eq!(n("piece_knight_black_02").as_deref(), Some("g8"));
        assert_eq!(n("piece_pawn_black_08").as_deref(), Some("h7"));
        assert_eq!(n("piece_queen_white").as_deref(), Some("d1"));
        assert_eq!(n("piece_king_black").as_deref(), Some("e8"));
        assert_eq!(n("board"), None);
        assert_eq!(n("piece_pawn_white_09"), None);
    }
}
