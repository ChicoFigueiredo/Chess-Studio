//! Erros tipados do núcleo de regras.

use crate::types::Move;
use std::fmt;

/// Motivo pelo qual um texto não é um FEN aceito.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FenError {
    /// Faltam campos (o FEN tem 6: peças, lado, roques, en passant, relógio de meio-lance, número do lance).
    MissingField,
    /// Há campos sobrando depois do número do lance.
    TooManyFields,
    /// O campo de peças é malformado ou a disposição é impossível
    /// (rei ausente ou duplicado, peão na primeira/última fileira, lado que não joga em xeque).
    InvalidBoard,
    /// O lado a jogar não é `w` nem `b`.
    InvalidSideToMove,
    /// Direitos de roque malformados ou sem rei/torre na casa de origem.
    InvalidCastlingRights,
    /// Casa de en passant malformada ou sem peão que tenha acabado de avançar duas casas.
    InvalidEnPassant,
    /// Relógio de meio-lance não numérico ou fora de 0..=65535.
    InvalidHalfmoveClock,
    /// Número do lance não numérico ou zero.
    InvalidFullmoveNumber,
}

impl fmt::Display for FenError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let msg = match self {
            Self::MissingField => "FEN incompleto: esperados 6 campos separados por espaço",
            Self::TooManyFields => "FEN com campos demais: esperados 6 campos",
            Self::InvalidBoard => {
                "campo de peças inválido ou posição impossível (reis, peões ou xeque ilegal)"
            }
            Self::InvalidSideToMove => "lado a jogar inválido: esperado 'w' ou 'b'",
            Self::InvalidCastlingRights => {
                "direitos de roque inválidos: esperado '-' ou combinação de KQkq coerente com o tabuleiro"
            }
            Self::InvalidEnPassant => {
                "casa de en passant inválida: esperado '-' ou casa atrás de um peão que avançou duas casas"
            }
            Self::InvalidHalfmoveClock => {
                "relógio de meio-lance inválido: esperado inteiro de 0 a 65535"
            }
            Self::InvalidFullmoveNumber => {
                "número do lance inválido: esperado inteiro maior que zero"
            }
        };
        f.write_str(msg)
    }
}

impl std::error::Error for FenError {}

/// Texto que não é uma casa (`a1`..`h8`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SquareParseError(pub String);

impl fmt::Display for SquareParseError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "casa inválida: {:?} (esperado a1..h8)", self.0)
    }
}

impl std::error::Error for SquareParseError {}

/// Texto que não é um Lance em notação UCI (`e2e4`, `e7e8q`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MoveParseError {
    /// Comprimento diferente de 4 ou 5 caracteres.
    InvalidLength(usize),
    /// Casa de origem ou destino inválida.
    InvalidSquare(String),
    /// Letra de promoção que não é `q`, `r`, `b` ou `n`.
    InvalidPromotion(char),
}

impl fmt::Display for MoveParseError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidLength(n) => {
                write!(f, "lance UCI deve ter 4 ou 5 caracteres, recebeu {n}")
            }
            Self::InvalidSquare(s) => write!(f, "casa inválida no lance UCI: {s:?}"),
            Self::InvalidPromotion(c) => {
                write!(
                    f,
                    "peça de promoção inválida: {c:?} (esperado q, r, b ou n)"
                )
            }
        }
    }
}

impl std::error::Error for MoveParseError {}

/// Lance que as regras não permitem na Posição canônica.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct IllegalMoveError(pub Move);

impl fmt::Display for IllegalMoveError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "lance ilegal na posição: {}", self.0)
    }
}

impl std::error::Error for IllegalMoveError {}

/// Motivo pelo qual um texto SAN não resolve para um Lance legal.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SanError {
    /// Texto vazio (ou só espaços e anotações).
    Empty,
    /// O texto não tem forma de SAN.
    Syntax(String),
    /// Nenhum Lance legal da Posição corresponde ao texto (inclui lances ilegais).
    NoMatchingMove(String),
    /// Mais de um Lance legal corresponde; desambiguação insuficiente.
    Ambiguous(Vec<Move>),
}

impl fmt::Display for SanError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Empty => f.write_str("SAN vazio"),
            Self::Syntax(s) => write!(f, "SAN malformado: {s:?}"),
            Self::NoMatchingMove(s) => write!(f, "nenhum lance legal corresponde a {s:?}"),
            Self::Ambiguous(moves) => {
                let list: Vec<String> = moves.iter().map(ToString::to_string).collect();
                write!(f, "SAN ambíguo: {}", list.join(", "))
            }
        }
    }
}

impl std::error::Error for SanError {}
