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

mod catalog;
mod config;
mod controller;
mod detect;
mod engine;
mod error;
mod install;
mod player;
mod protocol;

pub use catalog::{
    ASSETS, ArchiveKind, Asset, RELEASE_BASE_URL, STOCKFISH_LICENSE_NAME, STOCKFISH_LICENSE_URL,
    STOCKFISH_SOURCE_URL, STOCKFISH_VERSION, asset_for, asset_for_host, install_dir_name,
};
pub use config::{EngineConfig, Timeouts};
pub use controller::{
    ControllerEvent, EngineController, Failure, FailureKind, SearchRequest, Strength,
    StrengthApplied,
};
pub use detect::{
    DetectedEngine, PROBE_TIMEOUT, SearchEnv, app_data_dir, detect_host, detect_in,
    engine_files_in, engines_dir, load_preferred, probe_config, probe_engine, save_preferred,
    search_dirs,
};
pub use engine::{Engine, EngineMove, QuitOutcome};
pub use error::{EngineError, Stage};
pub use install::{InstallError, InstallSpec, Installed, Progress, install};
pub use player::{DEFAULT_MOVETIME, EnginePlayer, PlayerStatus};
pub use protocol::{
    BestMove, Clock, EngineInfo, EngineOption, GoParams, LogEntry, LogSource, PositionSpec,
};
