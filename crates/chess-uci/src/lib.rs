//! Engine UCI como processo filho: handshake, opções, `position`/`go`/`stop`,
//! timeouts, detecção de crash e log limitado (ADR-0003).
//!
//! Toda espera tem prazo: nenhuma chamada bloqueia quem a invoca indefinidamente.
//! Todo Lance devolvido pela Engine é só uma proposta; [`Engine::search_legal_move`]
//! a valida pelo `chess-core` e rejeita Lance ilegal como erro recuperável.
//!
//! # Rodar com Stockfish real (nunca versionado, ADR-0003)
//!
//! ```text
//! export CHESS_STUDIO_STOCKFISH=~/.cache/chess-studio-dev/stockfish/stockfish
//! cargo test -p chess-uci --test stockfish -- --ignored
//! cargo run -p chess-uci --bin uci-probe -- --engine "$CHESS_STUDIO_STOCKFISH" --movetime 50 --max-plies 40
//! ```

mod config;
mod controller;
mod engine;
mod error;
mod player;
mod protocol;

pub use config::{EngineConfig, Timeouts};
pub use controller::{
    ControllerEvent, EngineController, Failure, FailureKind, SearchRequest, Strength,
    StrengthApplied,
};
pub use engine::{Engine, EngineMove, QuitOutcome};
pub use error::{EngineError, Stage};
pub use player::{DEFAULT_MOVETIME, EnginePlayer, PlayerStatus};
pub use protocol::{
    BestMove, Clock, EngineInfo, EngineOption, GoParams, LogEntry, LogSource, PositionSpec,
};
