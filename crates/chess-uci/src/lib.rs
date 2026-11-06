//! Engine UCI como processo filho: handshake, opções, `position`/`go`/`stop`,
//! timeouts, detecção de crash e log limitado (ADR-0003).
//!
//! Toda espera tem prazo: nenhuma chamada bloqueia quem a invoca indefinidamente.
//! Todo Lance devolvido pela Engine é só uma proposta; [`Engine::search_legal_move`]
//! a valida pelo `chess-core` e rejeita Lance ilegal como erro recuperável.

mod config;
mod engine;
mod error;
mod protocol;

pub use config::{EngineConfig, Timeouts};
pub use engine::{Engine, EngineMove, QuitOutcome};
pub use error::{EngineError, Stage};
pub use protocol::{
    BestMove, Clock, EngineInfo, EngineOption, GoParams, LogEntry, LogSource, PositionSpec,
};
