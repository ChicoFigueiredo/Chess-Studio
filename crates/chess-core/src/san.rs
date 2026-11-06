//! SAN (notação algébrica padrão) escrita e leitura sobre a Posição canônica.
//!
//! Implementação própria a partir das Leis da FIDE (Anexo C); nenhum código de
//! terceiros GPL (ADR-0004).

use crate::error::{IllegalMoveError, SanError};
use crate::position::CanonicalPosition;
use crate::types::{Move, Piece, Square};

fn piece_letter(piece: Piece) -> Option<char> {
    match piece {
        Piece::Pawn => None,
        Piece::Knight => Some('N'),
        Piece::Bishop => Some('B'),
        Piece::Rook => Some('R'),
        Piece::Queen => Some('Q'),
        Piece::King => Some('K'),
    }
}

fn file_char(sq: Square) -> char {
    char::from(b'a' + sq.file())
}

fn rank_char(sq: Square) -> char {
    char::from(b'1' + sq.rank())
}

fn is_castling(piece: Piece, mv: Move) -> bool {
    piece == Piece::King && mv.from().file().abs_diff(mv.to().file()) == 2
}

impl CanonicalPosition {
    /// Escreve o Lance em SAN nesta Posição, com sufixo `+` ou `#`.
    ///
    /// Desambigua o mínimo: coluna, senão fileira, senão ambas. Falha se o
    /// Lance não é legal.
    pub fn san(&self, mv: Move) -> Result<String, IllegalMoveError> {
        if !self.is_legal(mv) {
            return Err(IllegalMoveError(mv));
        }
        let (_, piece) = self.piece_at(mv.from()).ok_or(IllegalMoveError(mv))?;
        let mut out = String::new();
        if is_castling(piece, mv) {
            out.push_str(if mv.to().file() > mv.from().file() {
                "O-O"
            } else {
                "O-O-O"
            });
        } else {
            let capture = self.piece_at(mv.to()).is_some()
                || (piece == Piece::Pawn && mv.from().file() != mv.to().file());
            match piece_letter(piece) {
                None => {
                    if capture {
                        out.push(file_char(mv.from()));
                    }
                }
                Some(letter) => {
                    out.push(letter);
                    out.push_str(&self.disambiguation(piece, mv));
                }
            }
            if capture {
                out.push('x');
            }
            out.push_str(&mv.to().to_string());
            if let Some(promo) = mv.promotion() {
                out.push('=');
                out.push(piece_letter(promo).unwrap_or('Q'));
            }
        }
        let next = self.play(mv).map_err(|_| IllegalMoveError(mv))?;
        if next.is_check() {
            out.push(if next.legal_moves().is_empty() {
                '#'
            } else {
                '+'
            });
        }
        Ok(out)
    }

    fn disambiguation(&self, piece: Piece, mv: Move) -> String {
        let rivals: Vec<Move> = self
            .legal_moves()
            .into_iter()
            .filter(|m| {
                m.to() == mv.to()
                    && m.from() != mv.from()
                    && self.piece_at(m.from()).map(|(_, p)| p) == Some(piece)
            })
            .collect();
        if rivals.is_empty() {
            return String::new();
        }
        let shares_file = rivals.iter().any(|m| m.from().file() == mv.from().file());
        let shares_rank = rivals.iter().any(|m| m.from().rank() == mv.from().rank());
        match (shares_file, shares_rank) {
            (false, _) => file_char(mv.from()).to_string(),
            (true, false) => rank_char(mv.from()).to_string(),
            (true, true) => mv.from().to_string(),
        }
    }

    /// Lê um Lance em SAN nesta Posição.
    ///
    /// Tolerante: aceita `0-0`/`0-0-0`, sufixos `+`, `#`, anotações `!`/`?`,
    /// `x` omitido, promoção com ou sem `=` e desambiguação além do necessário.
    /// Rejeita texto malformado, lance ilegal e lance ambíguo com erro tipado.
    pub fn parse_san(&self, text: &str) -> Result<Move, SanError> {
        let trimmed = text
            .trim()
            .trim_end_matches(|c: char| matches!(c, '+' | '#' | '!' | '?') || c.is_whitespace());
        let trimmed = trimmed.strip_suffix("e.p.").unwrap_or(trimmed).trim_end();
        if trimmed.is_empty() {
            return Err(SanError::Empty);
        }
        let legal = self.legal_moves();
        let matches = if let Some(short) = castling_side(trimmed) {
            legal
                .into_iter()
                .filter(|m| {
                    matches!(self.piece_at(m.from()), Some((_, Piece::King)))
                        && is_castling(Piece::King, *m)
                        && (m.to().file() > m.from().file()) == short
                })
                .collect::<Vec<_>>()
        } else {
            let query = Query::parse(trimmed)?;
            legal
                .into_iter()
                .filter(|m| query.matches(self, *m))
                .collect()
        };
        match matches.as_slice() {
            [] => Err(SanError::NoMatchingMove(trimmed.to_owned())),
            [single] => Ok(*single),
            _ => Err(SanError::Ambiguous(matches)),
        }
    }
}

/// `Some(true)` roque curto, `Some(false)` longo.
fn castling_side(s: &str) -> Option<bool> {
    match s.replace('0', "O").as_str() {
        "O-O" => Some(true),
        "O-O-O" => Some(false),
        _ => None,
    }
}

struct Query {
    piece: Piece,
    to: Square,
    promotion: Option<Piece>,
    from_file: Option<u8>,
    from_rank: Option<u8>,
}

fn promotion_piece(c: char) -> Option<Piece> {
    match c.to_ascii_uppercase() {
        'Q' => Some(Piece::Queen),
        'R' => Some(Piece::Rook),
        'B' => Some(Piece::Bishop),
        'N' => Some(Piece::Knight),
        _ => None,
    }
}

impl Query {
    fn parse(s: &str) -> Result<Self, SanError> {
        let syntax = || SanError::Syntax(s.to_owned());
        let mut chars: Vec<char> = s.chars().collect();
        let piece = match chars.first() {
            Some('K') => Piece::King,
            Some('Q') => Piece::Queen,
            Some('R') => Piece::Rook,
            Some('B') => Piece::Bishop,
            Some('N') => Piece::Knight,
            _ => Piece::Pawn,
        };
        if piece != Piece::Pawn {
            chars.remove(0);
        }

        // Promoção: "=Q" (qualquer caixa) ou "Q" maiúsculo colado ao destino.
        let mut promotion = None;
        if let Some(&last) = chars.last() {
            let has_eq = chars.len() >= 2 && chars[chars.len() - 2] == '=';
            if has_eq || last.is_ascii_uppercase() {
                promotion = Some(promotion_piece(last).ok_or_else(syntax)?);
                chars.pop();
                if has_eq {
                    chars.pop();
                }
            }
        }
        if promotion.is_some() && piece != Piece::Pawn {
            return Err(syntax());
        }

        if chars.len() < 2 {
            return Err(syntax());
        }
        let dest: String = chars[chars.len() - 2..].iter().collect();
        let to: Square = dest.parse().map_err(|_| syntax())?;
        chars.truncate(chars.len() - 2);

        let had_capture = chars.last() == Some(&'x');
        if had_capture {
            chars.pop();
        }
        let (mut from_file, mut from_rank) = (None, None);
        for &c in &chars {
            match c {
                'a'..='h' if from_file.is_none() && from_rank.is_none() => {
                    from_file = Some(c as u8 - b'a');
                }
                '1'..='8' if from_rank.is_none() => from_rank = Some(c as u8 - b'1'),
                _ => return Err(syntax()),
            }
        }
        if piece == Piece::Pawn && (from_rank.is_some() || (had_capture && from_file.is_none())) {
            return Err(syntax());
        }
        Ok(Self {
            piece,
            to,
            promotion,
            from_file,
            from_rank,
        })
    }

    fn matches(&self, pos: &CanonicalPosition, mv: Move) -> bool {
        let Some((_, piece)) = pos.piece_at(mv.from()) else {
            return false;
        };
        piece == self.piece
            && mv.to() == self.to
            && mv.promotion() == self.promotion
            && !is_castling(piece, mv)
            && self.from_file.is_none_or(|f| f == mv.from().file())
            && self.from_rank.is_none_or(|r| r == mv.from().rank())
            // Peão sem coluna de origem anda reto (não captura).
            && (piece != Piece::Pawn
                || self.from_file.is_some()
                || mv.from().file() == mv.to().file())
    }
}
