//! Tipos de valor públicos: cor, peça, casa e Lance.

use crate::error::{MoveParseError, SquareParseError};
use std::fmt;
use std::str::FromStr;

/// Lado do jogador.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Color {
    White,
    Black,
}

impl Color {
    /// O outro lado.
    pub fn opposite(self) -> Self {
        match self {
            Self::White => Self::Black,
            Self::Black => Self::White,
        }
    }
}

/// Tipo de peça (sem cor).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Piece {
    Pawn,
    Knight,
    Bishop,
    Rook,
    Queen,
    King,
}

/// Casa do tabuleiro. Coordenadas partem de zero: coluna `a` = 0, fileira 1 = 0.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Square(u8);

impl Square {
    /// Cria a casa; `None` se fora de 0..8.
    pub fn new(file: u8, rank: u8) -> Option<Self> {
        (file < 8 && rank < 8).then_some(Self(rank * 8 + file))
    }

    /// Coluna, de 0 (`a`) a 7 (`h`).
    pub fn file(self) -> u8 {
        self.0 % 8
    }

    /// Fileira, de 0 (`1`) a 7 (`8`).
    pub fn rank(self) -> u8 {
        self.0 / 8
    }
}

impl fmt::Display for Square {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{}{}",
            char::from(b'a' + self.file()),
            char::from(b'1' + self.rank())
        )
    }
}

impl FromStr for Square {
    type Err = SquareParseError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.as_bytes() {
            &[f @ b'a'..=b'h', r @ b'1'..=b'8'] => Ok(Self((r - b'1') * 8 + (f - b'a'))),
            _ => Err(SquareParseError(s.to_owned())),
        }
    }
}

/// Lance em coordenadas: origem, destino e peça de promoção opcional.
///
/// O roque é o movimento do rei por duas casas (`e1g1`), como no UCI.
/// Um `Move` é só um valor; quem diz se é legal é a [`crate::CanonicalPosition`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Move {
    from: Square,
    to: Square,
    promotion: Option<Piece>,
}

impl Move {
    pub fn new(from: Square, to: Square, promotion: Option<Piece>) -> Self {
        Self {
            from,
            to,
            promotion,
        }
    }

    pub fn from(&self) -> Square {
        self.from
    }

    pub fn to(&self) -> Square {
        self.to
    }

    pub fn promotion(&self) -> Option<Piece> {
        self.promotion
    }
}

/// Formata em UCI: `e2e4`, `e7e8q`.
impl fmt::Display for Move {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}{}", self.from, self.to)?;
        if let Some(piece) = self.promotion {
            let c = match piece {
                Piece::Queen => 'q',
                Piece::Rook => 'r',
                Piece::Bishop => 'b',
                Piece::Knight => 'n',
                // Não surge de geração nem de parse; mantém o Display total.
                Piece::Pawn => 'p',
                Piece::King => 'k',
            };
            write!(f, "{c}")?;
        }
        Ok(())
    }
}

/// Lê UCI. Só verifica a sintaxe; a legalidade é da Posição.
impl FromStr for Move {
    type Err = MoveParseError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let chars: Vec<char> = s.chars().collect();
        if chars.len() != 4 && chars.len() != 5 {
            return Err(MoveParseError::InvalidLength(chars.len()));
        }
        let square = |part: &[char]| {
            let text: String = part.iter().collect();
            text.parse::<Square>()
                .map_err(|_| MoveParseError::InvalidSquare(text))
        };
        let from = square(&chars[0..2])?;
        let to = square(&chars[2..4])?;
        let promotion = match chars.get(4) {
            None => None,
            Some('q') => Some(Piece::Queen),
            Some('r') => Some(Piece::Rook),
            Some('b') => Some(Piece::Bishop),
            Some('n') => Some(Piece::Knight),
            Some(&c) => return Err(MoveParseError::InvalidPromotion(c)),
        };
        Ok(Self::new(from, to, promotion))
    }
}
