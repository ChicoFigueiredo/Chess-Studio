//! Erros tipados da importação, sempre com linha e coluna (1-based, em caracteres).

use chess_core::{FenError, SanError};
use std::fmt;

#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum PgnErrorKind {
    /// `{` sem `}` até o fim do texto.
    UnterminatedComment,
    /// `"` sem fechamento na mesma linha.
    UnterminatedTagValue,
    /// Tag que não segue `[Nome "valor"]`.
    MalformedTag(String),
    /// `FEN` da tag não é uma Posição aceita.
    InvalidFen {
        fen: String,
        source: FenError,
    },
    /// O Lance não é legal (ou não é SAN) na Posição em que aparece.
    IllegalMove {
        /// Texto do Lance como escrito no arquivo.
        san: String,
        /// Meio-lance (1-based) dentro da linha onde ocorre.
        ply_in_line: usize,
        /// Posição (FEN) onde o Lance foi tentado.
        fen: String,
        source: SanError,
    },
    /// Lance nulo (`--`, `Z0`) não é suportado.
    NullMove(String),
    UnexpectedToken(String),
    /// `)` sem `(` correspondente.
    UnbalancedCloseParen,
    /// `(` sem `)` até o fim da partida.
    UnclosedVariation,
    EmptyVariation,
    /// `(` antes de qualquer Lance da linha.
    VariationWithoutMove,
    /// NAG ou `!`/`?` sem Lance anterior.
    AnnotationWithoutMove,
    /// `$` seguido de algo que não é 0..=255.
    InvalidNag(String),
    /// Variações aninhadas além do limite.
    NestingTooDeep,
}

/// Erro de importação: onde (partida, linha, coluna) e o quê.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PgnError {
    /// Número da partida no arquivo (1-based).
    pub game: usize,
    pub line: u32,
    pub column: u32,
    pub kind: PgnErrorKind,
}

impl fmt::Display for PgnError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "partida {}, linha {}, coluna {}: ",
            self.game, self.line, self.column
        )?;
        match &self.kind {
            PgnErrorKind::UnterminatedComment => f.write_str("comentário `{` sem `}`"),
            PgnErrorKind::UnterminatedTagValue => {
                f.write_str("valor de tag sem aspas de fechamento")
            }
            PgnErrorKind::MalformedTag(why) => write!(f, "tag malformada ({why})"),
            PgnErrorKind::InvalidFen { fen, source } => {
                write!(f, "FEN inválido {fen:?}: {source}")
            }
            PgnErrorKind::IllegalMove {
                san,
                ply_in_line,
                fen,
                source,
            } => write!(
                f,
                "lance {san:?} (meio-lance {ply_in_line} da linha) inválido na posição {fen}: {source}"
            ),
            PgnErrorKind::NullMove(t) => write!(f, "lance nulo {t:?} não suportado"),
            PgnErrorKind::UnexpectedToken(t) => write!(f, "texto inesperado {t:?}"),
            PgnErrorKind::UnbalancedCloseParen => f.write_str("`)` sem `(` correspondente"),
            PgnErrorKind::UnclosedVariation => f.write_str("variação `(` sem `)`"),
            PgnErrorKind::EmptyVariation => f.write_str("variação vazia"),
            PgnErrorKind::VariationWithoutMove => {
                f.write_str("variação antes de qualquer lance da linha")
            }
            PgnErrorKind::AnnotationWithoutMove => f.write_str("anotação sem lance anterior"),
            PgnErrorKind::InvalidNag(t) => write!(f, "NAG inválido {t:?}"),
            PgnErrorKind::NestingTooDeep => f.write_str("variações aninhadas demais"),
        }
    }
}

impl std::error::Error for PgnError {}
