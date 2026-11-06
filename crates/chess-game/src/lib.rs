//! A Partida: recebe um [`Command`] mais um instante injetado ([`Timestamp`]) e
//! devolve [`Event`]s; [`Game::replay`] reconstrói o estado do log (ADR-0001/0002).
//!
//! É a interface principal de testes do domínio: nenhum relógio do sistema é lido
//! aqui, nada depende de Bevy ou SQLite. As Políticas de arbitragem são internas;
//! hoje existe o Assistido. Leitura pela UI: [`Game::snapshot`].
//!
//! ```
//! use chess_game::{Command, Game, GameSetup, PolicyKind, Timestamp};
//! use chess_core::Square;
//!
//! let mut game = Game::new(GameSetup::standard(PolicyKind::Assisted));
//! let sq = |s: &str| s.parse::<Square>().unwrap();
//! game.handle(Command::Release { from: sq("e2"), to: sq("e4") }, Timestamp::mono(0)).unwrap();
//! assert_eq!(game.snapshot().history[0].san, "e4");
//! ```

mod command;
mod event;
mod game;
mod outcome;
mod policy;
mod rejection;
mod setup;
mod snapshot;
mod state;
mod time;
mod wire;

pub use command::{Command, CommandKind};
pub use event::{Event, EventKind, IllegalReason, PAYLOAD_VERSION};
pub use game::Game;
pub use outcome::{EndReason, GameResult, Outcome};
pub use rejection::Rejection;
pub use setup::{GameSetup, PolicyKind};
pub use snapshot::{HistoryEntry, ProvisionalView, Snapshot};
pub use state::{PendingPromotion, ReplayError};
pub use time::{TimeControl, Timestamp};
