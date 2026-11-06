//! A Partida: recebe um [`Command`] mais um instante injetado ([`Timestamp`]) e
//! devolve [`Event`]s; [`Game::replay`] reconstrói o estado do log (ADR-0001/0002).
//!
//! É a interface principal de testes do domínio: nenhum relógio do sistema é lido
//! aqui, nada depende de Bevy ou SQLite. As Políticas de arbitragem são internas:
//! Assistido, Simulação FIDE (Leis de 1/1/2023) e Eliminação Imediata (regra personalizada). Leitura pela UI: [`Game::snapshot`];
//! o relógio por instante, em [`Game::clock_at`].
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

mod clock;
mod command;
mod event;
mod fide;
mod game;
mod outcome;
mod pgn;
mod policy;
mod rejection;
mod review;
mod setup;
mod snapshot;
mod state;
mod time;
mod touch;
mod wire;

pub use clock::{ClockReading, ClockView, FlagFall, TWO_MINUTES_MS};
pub use command::{Command, CommandKind};
pub use event::{
    DrawBasis, Event, EventKind, IllegalReason, InfractionCause, PAYLOAD_VERSION, TimePenalty,
};
pub use game::Game;
pub use outcome::{EndReason, GameResult, Outcome};
pub use pgn::{PgnOptions, pgn_date_from_unix_ms};
pub use rejection::Rejection;
pub use review::ReviewView;
pub use setup::{
    CLASSICAL_MIN_BASE_MS, CUSTOM_RULES_LABEL, FIDE_LAWS_VERSION, GameSetup, InfractionStage,
    PolicyKind,
};
pub use snapshot::{
    HistoryEntry, MadeMove, ProvisionalRelease, ProvisionalView, Snapshot, TouchedPiece,
};
pub use state::{PendingPromotion, ReplayError};
pub use time::{TimeControl, Timestamp};
