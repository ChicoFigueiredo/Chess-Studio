//! Chave normalizada de Posição, compartilhada pelo índice local (`chess-store`) e pelo
//! cache de consultas por Posição aos Provedores (`chess-net`): o Explorador local e o de
//! Provedor precisam concordar sobre "a mesma Posição".

use crate::position::CanonicalPosition;
use crate::types::Piece;

/// Chave normalizada da Posição: campos de peças, vez, roque e en passant (este só quando
/// há captura en passant legal). Ignora os contadores de Lances.
pub fn position_key(position: &CanonicalPosition) -> String {
    let fen = position.to_fen();
    let mut fields = fen.split(' ');
    let board = fields.next().unwrap_or_default();
    let side = fields.next().unwrap_or("w");
    let castling = fields.next().unwrap_or("-");
    let ep = fields.next().unwrap_or("-");
    let ep = if ep != "-" && en_passant_capturable(position, ep) {
        ep
    } else {
        "-"
    };
    format!("{board} {side} {castling} {ep}")
}

fn en_passant_capturable(position: &CanonicalPosition, target: &str) -> bool {
    position.legal_moves().into_iter().any(|mv| {
        mv.to().to_string() == target
            && mv.from().file() != mv.to().file()
            && matches!(position.piece_at(mv.from()), Some((_, Piece::Pawn)))
            && position.piece_at(mv.to()).is_none()
    })
}
