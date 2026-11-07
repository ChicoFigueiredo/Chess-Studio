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

mod analysis;
mod backend;
mod catalog;
mod config;
mod controller;
mod detect;
mod engine;
mod error;
mod install;
mod load;
mod manager;
mod options;
mod player;
mod protocol;

pub use analysis::{
    AnalysisConfig, AnalysisController, AnalysisEvent, AnalysisLine, AnalysisUpdate,
    DEFAULT_INTERVAL, MAX_MULTIPV, Score,
};
pub use backend::{
    BACKENDS, BackendClass, BackendInfo, BackendProblem, GpuVendor, Hardware, advertised_backends,
    auto_order, backend_info, classify_failure, describe_failure, detect_hardware,
    fallback_backend, is_gpu_backend, lc0_install_help, recommended_backend, supported_on,
};
pub use catalog::{
    ASSETS, ArchiveKind, Asset, Backend, CATALOG, CatalogEntry, LC0_ASSETS, LC0_NETWORKS,
    LC0_VERSION, Network, OFFICIAL_HOSTS, RELEASE_BASE_URL, STOCKFISH_LICENSE_NAME,
    STOCKFISH_LICENSE_URL, STOCKFISH_SOURCE_URL, STOCKFISH_VERSION, asset_for, asset_for_host,
    entry as catalog_entry, install_dir_name, is_official_url,
};
pub use config::{EngineConfig, Timeouts};
pub use controller::{
    ControllerEvent, EngineController, Failure, FailureKind, SearchRequest, Strength,
    StrengthApplied,
};
pub use detect::{
    DetectedEngine, PROBE_TIMEOUT, SearchEnv, app_data_dir, detect_host, detect_in,
    engine_files_in, engines_dir, load_preferred, probe_config, probe_engine, probe_full,
    save_preferred, search_dirs,
};
pub use engine::{Engine, EngineMove, QuitOutcome};
pub use error::{EngineError, Stage};
pub use install::{
    INSTALL_MARKER, InstallError, InstallSpec, Installed, InstalledNetwork, NetworkSpec, Progress,
    install, install_network,
};
pub use load::{
    Duty, FpsTarget, FrameStats, FrameWindow, Governor, Level, LoadMode, LoadPlan,
};
pub use manager::{
    InstallPlan, NETWORKS_DIR, RemoveError, Updated, install_from_catalog,
    install_network_from_catalog, installed_networks, launch_options, plans_for, remove_installed,
    update, update_available, update_from_catalog, weights_option,
};
pub use options::{
    Control, Limits, OptionGroup, OptionSpec, SettingError, effective_bounds, group_of,
    is_path_like, parse_option, parse_options, resolve_settings, validate_option,
};
pub use player::{DEFAULT_MOVETIME, EnginePlayer, PlayerStatus};
pub use protocol::{
    BestMove, Bound, Clock, EngineInfo, EngineOption, GoParams, InfoLine, LogEntry, LogSource,
    PositionSpec, UciScore,
};
