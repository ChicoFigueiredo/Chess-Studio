//! Núcleo de regras: Posição canônica, Lance, geração e validação de lances,
//! FEN e perft. Não depende de Bevy, SQLite nem de relógio.
//!
//! A biblioteca `cozy-chess` fica escondida atrás desta API: nenhum tipo dela
//! aparece na interface pública, de modo que ela pode ser trocada sem tocar
//! nos demais crates.
//!
//! Inclui SAN, mate/afogamento, material insuficiente, chave de repetição e
//! contadores de 50/75 lances. Reclamações e Políticas ficam no `chess-game`.
//!
//! ```
//! use chess_core::{CanonicalPosition, Move};
//!
//! let pos = CanonicalPosition::start();
//! let mv: Move = "e2e4".parse().unwrap();
//! let next = pos.play(mv).unwrap();
//! assert_eq!(next.to_fen(), "rnbqkbnr/pppppppp/8/8/4P3/8/PPPP1PPP/RNBQKBNR b KQkq e3 0 1");
//! ```

mod attacks;
mod error;
mod position;
mod repetition;
mod san;
mod status;
mod types;

pub use error::{FenError, IllegalMoveError, MoveParseError, SanError, SquareParseError};
pub use position::CanonicalPosition;
pub use repetition::RepetitionKey;
pub use status::{FIFTY_MOVE_HALFMOVES, GameStatus, SEVENTY_FIVE_MOVE_HALFMOVES};
pub use types::{Color, Move, Piece, Square};
