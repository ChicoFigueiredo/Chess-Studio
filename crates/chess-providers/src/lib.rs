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
//! | Perfil do Lichess | [`Lichess::profile`] | 10 min |
//! | FIDE via Lichess | [`Lichess::fide_player`], [`Lichess::fide_search`] | 1 dia |
//! | Perfil do Chess.com | [`ChessCom::profile`] | 10 min |
//! | Meses com Partidas | [`ChessCom::archives`] | 1 hora |
//! | PGN de um mês | [`ChessCom::month_games`] | 7 dias; mês corrente 10 min |
//! | Partidas públicas (fluxo, em série) | [`ChessCom::games`] | por mês, como acima |

mod answer;
mod chesscom;
mod dates;
mod error;
mod eval;
mod explorer;
mod games;
mod lichess;
mod pgn;
mod profile;
mod tablebase;
pub mod ttl;

pub use answer::{Answer, Query};
pub use chesscom::{ChessCom, ChessComProfile, MonthGames};
pub use dates::{Day, YearMonth};
pub use error::ProviderError;
pub use eval::{CloudEval, EvalLine, POV, Score};
pub use explorer::{
    ExplorerEvent, ExplorerMove, ExplorerPosition, ExplorerStream, LichessFilter, MastersFilter,
    OpeningName, Outcomes, PlayerFilter, ReferenceGame, ReferencePlayer, explorer_license,
};
pub use games::{
    DEFAULT_MAX_GAMES, GamesEvent, GamesFeed, GamesFilter, GamesProgress, GamesReader,
    import_options,
};
pub use lichess::Lichess;
pub use profile::{FideRating, LichessProfile, PerfRating};
pub use tablebase::{Category, MAX_PIECES, Tablebase, TablebaseMove};

/// Ritmo de jogo e cor, tipos dos filtros (vêm do `chess-store` e do `chess-core`).
pub use chess_core::Color;
pub use chess_store::Pace;
