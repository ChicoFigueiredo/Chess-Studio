//! Obrigação de peça tocada (arts. 4.3 e 4.4 das Leis da FIDE, 1/1/2023).
//!
//! Função pura: dada a Posição, os toques desta vez (em ordem) e o Lance anunciado
//! por uma Reclamação incorreta, diz quais Lances legais cumprem a obrigação.
//! `None` significa que não há obrigação (nenhum toque, ou nenhuma peça tocada
//! pode se mover/ser capturada: art. 4.5, vale qualquer Lance legal).

use crate::state::Touched;
use chess_core::{CanonicalPosition, Move, Piece, Square};

/// Lances que cumprem a obrigação, ou `None` se qualquer Lance legal serve.
pub(crate) fn obligation(
    position: &CanonicalPosition,
    touched: &[Touched],
    intended: Option<Move>,
) -> Option<Vec<Move>> {
    let side = position.side_to_move();
    let legal = position.legal_moves();

    // Art. 9.5.3: o Lance anunciado numa Reclamação incorreta deve ser jogado.
    if let Some(m) = intended
        && legal.contains(&m)
    {
        return Some(vec![m]);
    }
    if touched.is_empty() {
        return None;
    }

    let own: Vec<&Touched> = touched.iter().filter(|t| t.owner == side).collect();
    let opp: Vec<&Touched> = touched.iter().filter(|t| t.owner != side).collect();

    // Art. 4.4.1 / 4.4.3: rei e depois torre de roque.
    if let Some(king) = own.first().filter(|t| t.piece == Piece::King)
        && let Some(rook) = own
            .iter()
            .skip(1)
            .find(|t| t.piece == Piece::Rook && castling_target(king.square, t.square).is_some())
        && let Some(dest) = castling_target(king.square, rook.square)
    {
        let castle = legal
            .iter()
            .find(|m| m.from() == king.square && m.to() == dest);
        if let Some(m) = castle {
            return Some(vec![*m]);
        }
        let king_moves = moves_from(&legal, king.square, &[]);
        return (!king_moves.is_empty()).then_some(king_moves);
    }

    // Art. 4.4.2: torre tocada antes do rei (com roque em mente) não roca com ela.
    let mut excluded: Vec<Move> = Vec::new();
    for (i, rook) in own.iter().enumerate() {
        if rook.piece != Piece::Rook {
            continue;
        }
        for king in own.iter().skip(i + 1).filter(|t| t.piece == Piece::King) {
            if let Some(dest) = castling_target(king.square, rook.square) {
                excluded.extend(
                    legal
                        .iter()
                        .filter(|m| m.from() == king.square && m.to() == dest),
                );
            }
        }
    }

    // Art. 4.3.3: peças das duas cores tocadas: capturar a peça do adversário com a
    // (primeira) peça própria tocada, se for legal.
    if let Some(mine) = own.first()
        && !opp.is_empty()
    {
        for theirs in &opp {
            let captures: Vec<Move> = legal
                .iter()
                .filter(|m| {
                    m.from() == mine.square && captures_square(position, **m, theirs.square)
                })
                .copied()
                .collect();
            if !captures.is_empty() {
                return Some(captures);
            }
        }
    }

    // Arts. 4.3.1, 4.3.2 e 4.3.3 (segunda parte): a primeira peça tocada que pode
    // se mover (própria) ou ser capturada (do adversário).
    for t in touched {
        let candidates: Vec<Move> = if t.owner == side {
            moves_from(&legal, t.square, &excluded)
        } else {
            legal
                .iter()
                .filter(|m| captures_square(position, **m, t.square))
                .copied()
                .collect()
        };
        if !candidates.is_empty() {
            return Some(candidates);
        }
    }
    None
}

fn moves_from(legal: &[Move], from: Square, excluded: &[Move]) -> Vec<Move> {
    legal
        .iter()
        .filter(|m| m.from() == from && !excluded.contains(m))
        .copied()
        .collect()
}

/// Casa de destino do rei ao rocar com a torre em `rook`, se as duas casas têm a
/// geometria de roque (rei na coluna e, torre no canto da mesma fileira).
fn castling_target(king: Square, rook: Square) -> Option<Square> {
    if king.file() != 4 || king.rank() != rook.rank() {
        return None;
    }
    match rook.file() {
        7 => Square::new(6, king.rank()),
        0 => Square::new(2, king.rank()),
        _ => None,
    }
}

/// O Lance captura a peça em `target` (inclui en passant)?
fn captures_square(position: &CanonicalPosition, mv: Move, target: Square) -> bool {
    if mv.to() == target {
        return position
            .piece_at(target)
            .is_some_and(|(color, _)| Some(color) != position.piece_at(mv.from()).map(|p| p.0));
    }
    let is_pawn = position
        .piece_at(mv.from())
        .is_some_and(|(_, p)| p == Piece::Pawn);
    is_pawn
        && mv.from().file() != mv.to().file()
        && position.piece_at(mv.to()).is_none()
        && Square::new(mv.to().file(), mv.from().rank()) == Some(target)
}
