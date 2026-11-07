//! Persistência SQLite: migrações versionadas, log de eventos append-only por
//! Partida, preferências, Exercícios, Resoluções e Engines conhecidas (ADR-0002).
//!
//! # Como o app liga o store (ticket seguinte)
//!
//! ```text
//! let mut store = Store::open(path)?;                    // WAL + migrações
//! let resume = match store.latest_unfinished_game()? {   // "continuar última Partida"
//!     Some(id) => Some(store.open_game(id)?),            // OpenedGame: Game pausada
//!     None => None,
//! };
//! // Partida nova:
//! let id = store.create_game(game.setup(), &GameMetadata::default())?;
//! // A cada Command aceito: gravar os Eventos devolvidos por `Game::handle`
//! store.append_events(id, &events)?;                     // 1 transação por chamada
//! // Ao fechar a janela normalmente (nunca há botão Salvar, ADR-0002):
//! store.close_game(id)?;
//! // O jogador clica em "Retomar": store.resume_game(id)?;
//! ```
//!
//! Garantias: cada `append_events` é uma transação (tudo ou nada, mesmo com
//! SIGKILL), `synchronous=FULL` + WAL (o Evento confirmado sobrevive a queda de
//! energia) e o log só é lido de volta por [`Game::try_replay`], nunca por replay
//! tolerante, para que um banco corrompido seja um erro, não uma Partida truncada.
//!
//! # Pausa (nota para o B6)
//!
//! O chess-game ainda não tem o Evento `Paused`. Até lá a Pausa é metadado em
//! `games.paused`: [`Store::open_game`] sempre devolve `paused: true` e o app
//! chama [`Store::resume_game`] quando o jogador retoma. Quando o B6 criar o
//! Evento de Pausa/retomada, o app deve também anotá-lo no log (o relógio
//! congela no último Evento gravado, DEC-42) e este metadado passa a ser só um
//! atalho de listagem.

mod engines;
mod error;
mod imported;
pub mod migrations;
mod store;
mod training;
mod training_profiles;

pub use engines::{EngineId, EngineOrigin, EngineRecord};
pub use error::{Result, StoreError};
pub use imported::{ImportReport, ImportedId, ImportedQuery, ImportedSummary};
pub use migrations::LATEST_VERSION as LATEST_SCHEMA_VERSION;
pub use store::{
    GameId, GameMetadata, GameQuery, GameSummary, OpenedGame, Pragmas, ProfileId, Store,
};
pub use training::{RunId, RunQuery, StoredRun, UpsertReport};

#[cfg(doc)]
use chess_game::Game;
