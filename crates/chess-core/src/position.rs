//! Posição canônica sobre `cozy-chess`, com conversão de/para os tipos próprios.
//!
//! O cozy-chess codifica o roque como "rei captura a própria torre"; aqui o
//! roque é sempre o rei andando duas casas (`e1g1`), então a tradução acontece
//! nas fronteiras `cozy_move` / `public_move`.

use crate::error::{FenError, IllegalMoveError};
use crate::types::{Color, Move, Piece, Square};
use cozy_chess as cc;
use std::fmt;
use std::str::FromStr;

/// Posição canônica: a única fonte de verdade das regras (ADR-0001).
/// Imutável: aplicar um Lance devolve uma nova Posição.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct CanonicalPosition {
    board: cc::Board,
}

impl CanonicalPosition {
    /// Posição inicial do xadrez.
    pub fn start() -> Self {
        Self {
            board: cc::Board::startpos(),
        }
    }

    /// Lê um FEN (seis campos). Espaços extras nas pontas e entre campos são tolerados.
    ///
    /// Roque só em formato padrão (`KQkq`); o relógio de meio-lance aceita 0..=100.
    pub fn from_fen(fen: &str) -> Result<Self, FenError> {
        let normalized = fen.split_whitespace().collect::<Vec<_>>().join(" ");
        if normalized.is_empty() {
            return Err(FenError::MissingField);
        }
        cc::Board::from_fen(&normalized, false)
            .map(|board| Self { board })
            .map_err(|e| match e {
                cc::FenParseError::InvalidBoard => FenError::InvalidBoard,
                cc::FenParseError::InvalidSideToMove => FenError::InvalidSideToMove,
                cc::FenParseError::InvalidCastlingRights => FenError::InvalidCastlingRights,
                cc::FenParseError::InvalidEnPassant => FenError::InvalidEnPassant,
                cc::FenParseError::InvalidHalfMoveClock => FenError::InvalidHalfmoveClock,
                cc::FenParseError::InvalidFullmoveNumber => FenError::InvalidFullmoveNumber,
                cc::FenParseError::MissingField => FenError::MissingField,
                cc::FenParseError::TooManyFields => FenError::TooManyFields,
            })
    }

    /// Serializa em FEN padrão (casa de en passant sempre após avanço duplo).
    pub fn to_fen(&self) -> String {
        self.board.to_string()
    }

    /// Lado a jogar.
    pub fn side_to_move(&self) -> Color {
        color_from(self.board.side_to_move())
    }

    /// Peça (com cor) na casa, se houver.
    pub fn piece_at(&self, square: Square) -> Option<(Color, Piece)> {
        let sq = square_to(square);
        let piece = self.board.piece_on(sq)?;
        let color = self.board.color_on(sq)?;
        Some((color_from(color), piece_from(piece)))
    }

    /// Meio-lances desde a última captura ou lance de peão (0..=100).
    pub fn halfmove_clock(&self) -> u8 {
        self.board.halfmove_clock()
    }

    /// Número do lance completo (começa em 1, incrementa após o lance preto).
    pub fn fullmove_number(&self) -> u16 {
        self.board.fullmove_number()
    }

    /// O lado a jogar está em xeque.
    pub fn is_check(&self) -> bool {
        !self.board.checkers().is_empty()
    }

    /// Todos os Lances legais, incluindo roque (rei por duas casas), en passant
    /// e as quatro promoções. A ordem não é especificada.
    pub fn legal_moves(&self) -> Vec<Move> {
        let mut moves = Vec::with_capacity(48);
        self.board.generate_moves(|piece_moves| {
            moves.extend(piece_moves.into_iter().map(|m| self.public_move(m)));
            false
        });
        moves
    }

    /// O Lance é legal nesta Posição?
    pub fn is_legal(&self, mv: Move) -> bool {
        self.cozy_move(mv)
            .is_some_and(|cozy_mv| self.board.is_legal(cozy_mv))
    }

    /// Aplica o Lance e devolve a nova Posição; `self` não muda.
    pub fn play(&self, mv: Move) -> Result<Self, IllegalMoveError> {
        let mut board = self.board.clone();
        let cozy_mv = self.cozy_move(mv).ok_or(IllegalMoveError(mv))?;
        board.try_play(cozy_mv).map_err(|_| IllegalMoveError(mv))?;
        Ok(Self { board })
    }

    /// Conta as folhas da árvore de Lances legais à profundidade dada
    /// (`perft(0) == 1`). Usado para provar a geração contra números de referência.
    pub fn perft(&self, depth: u32) -> u64 {
        perft(&self.board, depth)
    }

    /// `None` quando o destino tem peça do próprio lado e não é roque: o cozy-chess
    /// leria isso como roque ("rei captura torre"), que aqui não é notação válida.
    fn cozy_move(&self, mv: Move) -> Option<cc::Move> {
        let from = square_to(mv.from());
        let mut to = square_to(mv.to());
        let side = self.board.side_to_move();
        let is_king = self.board.piece_on(from) == Some(cc::Piece::King)
            && self.board.color_on(from) == Some(side);
        if is_king && from.rank() == to.rank() && (from.file() as u8).abs_diff(to.file() as u8) == 2
        {
            let rights = self.board.castle_rights(side);
            let rook_file = if to.file() > from.file() {
                rights.short
            } else {
                rights.long
            };
            if let Some(file) = rook_file {
                to = cc::Square::new(file, from.rank());
            }
        } else if self.board.color_on(to) == Some(side) {
            return None;
        }
        Some(cc::Move {
            from,
            to,
            promotion: mv.promotion().map(piece_to),
        })
    }

    fn public_move(&self, mv: cc::Move) -> Move {
        let side = self.board.side_to_move();
        let mut to = mv.to;
        // Roque: rei "captura" a própria torre.
        if self.board.color_on(mv.to) == Some(side) {
            let file = if mv.to.file() > mv.from.file() {
                cc::File::G
            } else {
                cc::File::C
            };
            to = cc::Square::new(file, mv.from.rank());
        }
        Move::new(
            square_from(mv.from),
            square_from(to),
            mv.promotion.map(piece_from),
        )
    }
}

fn perft(board: &cc::Board, depth: u32) -> u64 {
    match depth {
        0 => 1,
        1 => {
            let mut count = 0;
            board.generate_moves(|moves| {
                count += moves.len() as u64;
                false
            });
            count
        }
        _ => {
            let mut count = 0;
            board.generate_moves(|moves| {
                for mv in moves {
                    let mut next = board.clone();
                    next.play_unchecked(mv);
                    count += perft(&next, depth - 1);
                }
                false
            });
            count
        }
    }
}

impl FromStr for CanonicalPosition {
    type Err = FenError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Self::from_fen(s)
    }
}

impl fmt::Display for CanonicalPosition {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.to_fen())
    }
}

fn color_from(c: cc::Color) -> Color {
    match c {
        cc::Color::White => Color::White,
        cc::Color::Black => Color::Black,
    }
}

fn piece_from(p: cc::Piece) -> Piece {
    match p {
        cc::Piece::Pawn => Piece::Pawn,
        cc::Piece::Knight => Piece::Knight,
        cc::Piece::Bishop => Piece::Bishop,
        cc::Piece::Rook => Piece::Rook,
        cc::Piece::Queen => Piece::Queen,
        cc::Piece::King => Piece::King,
    }
}

fn piece_to(p: Piece) -> cc::Piece {
    match p {
        Piece::Pawn => cc::Piece::Pawn,
        Piece::Knight => cc::Piece::Knight,
        Piece::Bishop => cc::Piece::Bishop,
        Piece::Rook => cc::Piece::Rook,
        Piece::Queen => cc::Piece::Queen,
        Piece::King => cc::Piece::King,
    }
}

fn square_to(s: Square) -> cc::Square {
    cc::Square::new(
        cc::File::index(usize::from(s.file())),
        cc::Rank::index(usize::from(s.rank())),
    )
}

fn square_from(s: cc::Square) -> Square {
    // file()/rank() do cozy-chess são enums com valores 0..8.
    Square::new(s.file() as u8, s.rank() as u8).expect("casa do cozy-chess está em 0..8")
}
