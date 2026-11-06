//! Nomes visuais das peças, compartilhados pelas Apresentações: nomes dos nós do glTF
//! (`piece_<peça>_<cor>[_NN]`) e glifos Unicode do 2D técnico. A posição das peças vem
//! sempre da Partida (`Session::snapshot`), nunca daqui.

use chess_core::{Color, Piece};

/// Nome da peça usado pelos nós do glTF.
pub fn gltf_name(piece: Piece) -> &'static str {
    match piece {
        Piece::Pawn => "pawn",
        Piece::Knight => "knight",
        Piece::Bishop => "bishop",
        Piece::Rook => "rook",
        Piece::Queen => "queen",
        Piece::King => "king",
    }
}

/// Glifo Unicode preenchido (U+265A..U+265F) usado no 2D técnico.
pub fn glyph(piece: Piece) -> char {
    match piece {
        Piece::King => '\u{265A}',
        Piece::Queen => '\u{265B}',
        Piece::Rook => '\u{265C}',
        Piece::Bishop => '\u{265D}',
        Piece::Knight => '\u{265E}',
        Piece::Pawn => '\u{265F}',
    }
}

/// Peça e cor de um nó do glTF a partir do nome (`piece_rook_white_02` -> torre branca).
/// O ordinal é ignorado: as cópias de uma mesma peça são idênticas.
pub fn parse_node_name(name: &str) -> Option<(Color, Piece)> {
    let mut parts = name.strip_prefix("piece_")?.split('_');
    let kind_name = parts.next()?;
    let color = match parts.next()? {
        "white" => Color::White,
        "black" => Color::Black,
        _ => return None,
    };
    let piece = [
        Piece::Pawn,
        Piece::Knight,
        Piece::Bishop,
        Piece::Rook,
        Piece::Queen,
        Piece::King,
    ]
    .into_iter()
    .find(|p| gltf_name(*p) == kind_name)?;
    Some((color, piece))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn gltf_node_names_map_to_color_and_piece() {
        assert_eq!(
            parse_node_name("piece_rook_white_01"),
            Some((Color::White, Piece::Rook))
        );
        assert_eq!(
            parse_node_name("piece_knight_black_02"),
            Some((Color::Black, Piece::Knight))
        );
        assert_eq!(
            parse_node_name("piece_queen_white"),
            Some((Color::White, Piece::Queen))
        );
        assert_eq!(parse_node_name("board"), None);
        assert_eq!(parse_node_name("piece_dragon_white"), None);
        assert_eq!(parse_node_name("piece_pawn_green"), None);
    }

    #[test]
    fn every_piece_has_a_distinct_glyph() {
        let mut glyphs: Vec<char> = [
            Piece::Pawn,
            Piece::Knight,
            Piece::Bishop,
            Piece::Rook,
            Piece::Queen,
            Piece::King,
        ]
        .into_iter()
        .map(glyph)
        .collect();
        glyphs.sort_unstable();
        glyphs.dedup();
        assert_eq!(glyphs.len(), 6);
    }
}
