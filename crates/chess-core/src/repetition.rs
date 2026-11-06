//! Chave de repetição de Posição (arts. 9.2 e 9.6.1 da FIDE).

use crate::position::CanonicalPosition;
use crate::types::{Piece, Square};

/// Identifica a Posição para fins de repetição: mesma colocação de peças, mesmo
/// lado a jogar, mesmos direitos de roque e mesma possibilidade de captura en
/// passant. Os relógios não entram. Valor opaco: só serve para `==` e hash.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct RepetitionKey(String);

impl CanonicalPosition {
    /// Chave de repetição FIDE. O en passant só conta se existir captura en
    /// passant **legal** (um peão cravado, por exemplo, não a torna possível).
    pub fn repetition_key(&self) -> RepetitionKey {
        let fen = self.to_fen();
        let mut fields = fen.split(' ');
        let placement = fields.next().unwrap_or("");
        let side = fields.next().unwrap_or("");
        let castling = fields.next().unwrap_or("");
        let ep = fields.next().unwrap_or("-");
        let ep = if self.has_legal_en_passant(ep) {
            ep
        } else {
            "-"
        };
        RepetitionKey(format!("{placement} {side} {castling} {ep}"))
    }

    fn has_legal_en_passant(&self, ep: &str) -> bool {
        let Ok(target) = ep.parse::<Square>() else {
            return false;
        };
        self.legal_moves().iter().any(|m| {
            m.to() == target
                && m.from().file() != target.file()
                && matches!(self.piece_at(m.from()), Some((_, Piece::Pawn)))
        })
    }
}
