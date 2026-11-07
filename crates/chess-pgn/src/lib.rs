//! Importação e exportação PGN com variações, comentários e NAGs.
//!
//! Parser próprio (tokenizador + descida recursiva), sem dependências de PGN/GPL
//! (ADR-0004). O PGN é interoperabilidade; o log de Eventos continua sendo a
//! verdade (ADR-0002). Cada Lance é validado pelo `chess-core`.
//!
//! ```
//! let games = chess_pgn::parse_pgn("1. e4 e5 (1... c5 {Siciliana}) 2. Nf3 *").unwrap();
//! assert_eq!(games[0].mainline_sans(), ["e4", "e5", "Nf3"]);
//! assert_eq!(games[0].variation_count(), 1);
//! ```

mod convert;
mod error;
mod export;
mod lexer;
mod model;
mod parser;

pub use convert::ConvertError;
pub use error::{PgnError, PgnErrorKind};
pub use model::{MoveNode, PgnGame, PgnResult};
pub use parser::{ParseReport, parse_pgn, parse_pgn_lenient};
