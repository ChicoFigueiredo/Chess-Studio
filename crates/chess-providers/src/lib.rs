//! Adaptadores tipados de dados públicos (Lichess e Chess.com PubAPI) sobre a camada de rede
//! por Provedor (`chess-net`, ADR-0006). Sem Bevy e sem egui; **nenhuma conexão é aberta
//! aqui**: todo pedido passa por `chess_net::Net`, que confere consentimento, catálogo,
//! Trava de fair play e limites antes de tocar a rede.
//!
//! # Forma de uso (módulo profundo)
//!
//! Há dois clientes, [`Lichess`] e [`ChessCom`], e um só tipo de pedido pendente por
//! consulta:
//!
//! - consulta única: [`Query<T>`], com `try_take()` por quadro (nunca bloqueia), `cancel()` e
//!   cancelamento ao descartar. O resultado é um [`Answer<T>`]: o valor tipado **mais a
//!   Origem** (Provedor, instante, licença, modo) e o frescor do cache;
//! - fluxos: [`ExplorerStream`] (resultados parciais do Explorador de jogador) e
//!   [`GamesFeed`] (Partidas para a importação da Biblioteca).
//!
//! Erros são [`ProviderError`], tipados e em português: Provedor desligado, limitado até,
//! sem dados, nome inexistente, variante recusada, Trava de fair play.
//!
//! ```no_run
//! use chess_providers::{Lichess, MastersFilter};
//! # fn demo(lichess: Lichess) {
//! let mut pending = lichess.masters(
//!     "rnbqkbnr/pppppppp/8/8/4P3/8/PPPP1PPP/RNBQKBNR b KQkq - 0 1",
//!     &MastersFilter::default(),
//! );
//! // A cada quadro:
//! if let Some(result) = pending.try_take() {
//!     match result {
//!         Ok(answer) => println!(
//!             "{} Partidas ({})",
//!             answer.value.outcomes.total(),
//!             answer.provenance.license
//!         ),
//!         Err(error) => println!("{error}"),
//!     }
//! }
//! # }
//! ```
//!
//! | Consulta | Função | Cache ([`ttl`]) |
//! |---|---|---|
//! | Explorador de mestres | [`Lichess::masters`] | 7 dias |
//! | Explorador do Lichess | [`Lichess::explorer`] | 6 horas |
//! | Explorador de jogador (fluxo) | [`Lichess::player_explorer`] | sem cache |
//! | Avaliação em cache | [`Lichess::cloud_eval`] | 7 dias; «sem avaliação» 10 min |
//! | Finais exatos (até 7 peças) | [`Lichess::tablebase`] | 30 dias |
//! | Partidas públicas (fluxo) | [`Lichess::games`] | sem cache |
//! | Pacote de problemas (download em fluxo) | [`Lichess::download_puzzles`] | sem cache; corpo nunca guardado |
//! | Perfil do Lichess | [`Lichess::profile`] | 10 min |
//! | FIDE via Lichess | [`Lichess::fide_player`], [`Lichess::fide_search`] | 1 dia |
//! | Perfil do Chess.com | [`ChessCom::profile`] | 10 min |
//! | Meses com Partidas | [`ChessCom::archives`] | 1 hora |
//! | PGN de um mês | [`ChessCom::month_games`] | 7 dias; mês corrente 10 min |
//! | Partidas públicas (fluxo, em série) | [`ChessCom::games`] | por mês, como acima |
//!
//! # Conexão autorizada do Lichess
//!
//! O Explorador do Lichess exige uma Conexão autorizada (desde 2026-11-11). [`LichessAuth`]
//! autoriza pelo navegador (OAuth2 PKCE S256, retorno no loopback `127.0.0.1`) ou por token
//! pessoal, confere em `/api/account` e guarda o token no Cofre (`chess-vault`); o
//! [`Lichess`] passa a usar a Conexão com [`Lichess::use_connection`].
//!
//! ```no_run
//! use chess_net::ConnectionId;
//! use chess_providers::{AuthState, ConnectionRole, Lichess, LichessAuth};
//! # fn demo(auth: LichessAuth, lichess: Lichess) -> Result<(), chess_providers::AuthError> {
//! let connection = ConnectionId(7); // a linha da Conexão na Central
//! let flow = auth.authorize(connection, ConnectionRole::Player)?;
//! // A interface abre flow.url() no navegador do sistema e, a cada quadro:
//! match flow.state() {
//!     AuthState::Authorized(account) => {
//!         println!("autorizado como {} ({:?})", account.username, account.scopes);
//!         lichess.use_connection(Some(connection)); // o Explorador volta a funcionar
//!     }
//!     AuthState::Waiting => {}
//!     other => println!("{other:?}"),
//! }
//! // Desconectar: revoga no Lichess e apaga o segredo local de qualquer jeito.
//! let mut revocation = auth.revoke(connection);
//! # let _ = revocation.try_take();
//! # Ok(())
//! # }
//! ```

mod answer;
mod auth;
mod board;
mod bot;
mod chesscom;
mod dates;
mod error;
mod eval;
mod explorer;
mod family;
mod fics;
mod game_export;
mod games;
mod lichess;
mod pgn;
mod profile;
mod puzzles;
mod remote;
mod tablebase;
pub mod ttl;

pub use answer::{Answer, Query};
pub use auth::{
    AuthError, AuthFlow, AuthState, AuthTask, CLIENT_ID, ConnectionRole, DEFAULT_DEADLINE,
    LichessAccount, LichessAuth, Revocation, pkce_challenge,
};
pub use board::{
    Backoff, BoardAction, BoardError, CORRESPONDENCE_DAYS, Challenge, ChallengePlayer,
    ChallengeSpec, ConnectionEvent, ConnectionEvents, DeclineReason, DrawResponse, EnginePolicy,
    GameInfo, Gap, LichessBoard, LinkStatus, MoveEngine, OnlineGameSession, OnlineTime,
    OpponentGone, OpponentInfo, Seek, SeekState, Seeking, SessionNotice, SessionPoll,
};
pub use bot::{
    ChallengeAnswer, ChallengeDecision, ChallengePolicy, ChallengeSpeed, IrreversibleConfirmation,
    LichessBot, OpponentPolicy, RatedPolicy,
};
pub use chesscom::{ChessCom, ChessComProfile, MonthGames};
pub use dates::{Day, YearMonth};
pub use error::ProviderError;
pub use eval::{CloudEval, EvalLine, POV, Score};
pub use explorer::{
    ExplorerEvent, ExplorerMove, ExplorerPosition, ExplorerStream, LichessFilter, MastersFilter,
    OpeningName, Outcomes, PlayerFilter, ReferenceGame, ReferencePlayer, explorer_license,
};
pub use family::ProviderProfile;
pub use fics::{
    DEFAULT_LOGIN_DEADLINE, Fics, FicsChallenge, FicsError, FicsLogin, FicsNotice, FicsOutgoing,
    FicsPoll, FicsSession, FicsStatus, MAX_FICS_EVENTS_PER_POLL, ObservedGame,
};
pub use game_export::ExportedGame;
pub use games::{
    DEFAULT_MAX_GAMES, GamesEvent, GamesFeed, GamesFilter, GamesProgress, GamesReader,
    import_options,
};
pub use lichess::Lichess;
pub use profile::{FideRating, LichessProfile, PerfRating};
pub use puzzles::{
    MAX_PUZZLES, PUZZLE_HOST, PUZZLE_PATH, PuzzleDownload, PuzzleEvent, PuzzleOutcome,
    PuzzleProgress, PuzzleRequest,
};
pub use remote::{
    ChessApi, DEFAULT_DEPTH as REMOTE_DEFAULT_DEPTH, ENGINE_NAME as REMOTE_ENGINE_NAME,
    MAX_DEPTH as REMOTE_MAX_DEPTH, RemoteEngine, user_message as remote_user_message,
};
pub use tablebase::{Category, MAX_PIECES, Tablebase, TablebaseMove};

/// Ritmo de jogo e cor, tipos dos filtros (vêm do `chess-store` e do `chess-core`).
pub use chess_core::Color;
pub use chess_store::Pace;
