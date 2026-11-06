//! Núcleo de regras: Posição canônica, Lance, geração e validação de lances,
//! FEN e perft. Não depende de Bevy, SQLite nem de relógio.
//!
//! A biblioteca `cozy-chess` fica escondida atrás desta API: nenhum tipo dela
//! aparece na interface pública, de modo que ela pode ser trocada sem tocar
//! nos demais crates.
//!
//! Fora do escopo desta etapa (vem no B1): SAN, mate/afogamento como resultado,
//! material insuficiente e repetição.
//!
//! ```
//! use chess_core::{CanonicalPosition, Move};
//!
//! let pos = CanonicalPosition::start();
//! let mv: Move = "e2e4".parse().unwrap();
//! let next = pos.play(mv).unwrap();
//! assert_eq!(next.to_fen(), "rnbqkbnr/pppppppp/8/8/4P3/8/PPPP1PPP/RNBQKBNR b KQkq e3 0 1");
//! ```

mod error;
mod position;
mod types;

pub use error::{FenError, IllegalMoveError, MoveParseError, SquareParseError};
pub use position::CanonicalPosition;
pub use types::{Color, Move, Piece, Square};
