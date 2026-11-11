//! Partidas públicas de um jogador, em fluxo, para a importação da Biblioteca.
//!
//! [`Lichess::games`] e [`ChessCom::games`] devolvem um [`GamesFeed`]: a interface o lê por
//! quadro (`try_next`, com progresso) ou o entrega a um importador em segundo plano como um
//! `Read` de PGN ([`GamesFeed::into_reader`]) para `chess_store::Store::import_pgn_stream`,
//! que já cuida de lotes, cancelamento, duplicatas e Origem por Partida.
//!
//! ```no_run
//! # use chess_providers::{GamesFilter, Lichess};
//! # fn demo(lichess: Lichess, store: &mut chess_store::Store) -> Result<(), Box<dyn std::error::Error>> {
//! use std::sync::{Arc, atomic::AtomicBool};
//! let feed = lichess.games("thibault", &GamesFilter::default());
//! let options = feed.import_options();
//! let cancel = Arc::new(AtomicBool::new(false));
//! let reader = std::io::BufReader::new(feed.into_reader(cancel.clone()));
//! let summary = store.import_pgn_stream(
//!     chess_store::ProfileId(1), reader, &options, &cancel, &mut |_progress| {},
//! )?;
//! println!("{} novas, {} repetidas", summary.new, summary.duplicates);
//! # Ok(()) }
//! ```

use crate::answer::{stream_provenance, valid_name};
use crate::chesscom::ChessComPlan;
use crate::dates::Day;
use crate::error::{Missing, ProviderError};
use crate::lichess::Lichess;
use chess_core::Color;
use chess_net::{ProviderId, Request, Stream, StreamEvent};
use chess_store::{ImportMode, ImportOptions, ImportSource, Pace};
use serde::Deserialize;
use std::collections::VecDeque;
use std::fmt;
use std::io::{self, Read};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

/// Máximo padrão de Partidas por consulta: ninguém precisa de dez anos de blitz por engano.
pub const DEFAULT_MAX_GAMES: u32 = 300;

/// Filtros das Partidas públicas. No Lichess viram parâmetros do pedido; no Chess.com são
/// aplicados localmente sobre o PGN do mês.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GamesFilter {
    /// Primeiro dia incluído (UTC).
    pub since: Option<Day>,
    /// Último dia incluído (UTC).
    pub until: Option<Day>,
    /// Vazio = todos os ritmos.
    pub paces: Vec<Pace>,
    /// Só as Partidas em que o jogador tinha esta cor.
    pub color: Option<Color>,
    /// Máximo de Partidas, as mais recentes primeiro. `None` = sem limite.
    pub max: Option<u32>,
}

impl Default for GamesFilter {
    fn default() -> Self {
        Self {
            since: None,
            until: None,
            paces: Vec::new(),
            color: None,
            max: Some(DEFAULT_MAX_GAMES),
        }
    }
}

/// O que o fluxo de Partidas entrega.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum GamesEvent {
    /// PGN de uma ou mais Partidas, já filtradas (texto completo, com tags e Lances).
    Chunk(String),
    /// Acabou: tudo o que o filtro permitia foi entregue.
    Finished,
    /// O fluxo não abriu ou caiu; o que já foi entregue vale.
    Failed(ProviderError),
}

/// Andamento do fluxo.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct GamesProgress {
    /// Partidas entregues.
    pub games: usize,
    /// Partidas descartadas pelo filtro local ou por variante recusada.
    pub skipped: usize,
    /// Meses já lidos (só Chess.com).
    pub months_done: usize,
    /// Meses a ler (só Chess.com, depois de conhecer a lista).
    pub months_total: Option<usize>,
}

pub(crate) type SharedProgress = Arc<Mutex<GamesProgress>>;

enum Source {
    Lichess {
        provider: ProviderId,
        stream: Stream,
        name: String,
    },
    ChessCom(Box<ChessComPlan>),
    Early(Option<ProviderError>),
    Done,
}

/// Fluxo de Partidas de um jogador. Descartar o fluxo cancela o que estiver em andamento.
pub struct GamesFeed {
    source: Source,
    provenance: chess_net::Provenance,
    progress: SharedProgress,
    queued: VecDeque<GamesEvent>,
}

impl fmt::Debug for GamesFeed {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("GamesFeed")
            .field("progress", &self.progress())
            .finish_non_exhaustive()
    }
}

impl GamesFeed {
    pub(crate) fn lichess(
        provider: ProviderId,
        stream: Stream,
        name: String,
        provenance: chess_net::Provenance,
    ) -> Self {
        Self::with(
            Source::Lichess {
                provider,
                stream,
                name,
            },
            provenance,
        )
    }

    pub(crate) fn chesscom(plan: ChessComPlan, provenance: chess_net::Provenance) -> Self {
        Self::with(Source::ChessCom(Box::new(plan)), provenance)
    }

    pub(crate) fn failed(error: ProviderError, provenance: chess_net::Provenance) -> Self {
        Self::with(Source::Early(Some(error)), provenance)
    }

    fn with(source: Source, provenance: chess_net::Provenance) -> Self {
        Self {
            source,
            provenance,
            progress: Arc::default(),
            queued: VecDeque::new(),
        }
    }

    /// Origem das Partidas deste fluxo (Provedor, instante da consulta, licença).
    pub fn provenance(&self) -> &chess_net::Provenance {
        &self.provenance
    }

    pub fn progress(&self) -> GamesProgress {
        self.progress.lock().map(|p| p.clone()).unwrap_or_default()
    }

    /// Próximo evento, se já chegou (nunca bloqueia). Chame a cada quadro.
    pub fn try_next(&mut self) -> Option<GamesEvent> {
        if let Some(event) = self.queued.pop_front() {
            return Some(event);
        }
        let mut out = VecDeque::new();
        let mut progress = self.progress();
        match &mut self.source {
            Source::Lichess {
                provider,
                stream,
                name,
            } => poll_lichess(*provider, stream, name, &mut out, &mut progress),
            Source::ChessCom(plan) => plan.poll(&mut out, &mut progress),
            Source::Early(error) => {
                if let Some(error) = error.take() {
                    out.push_back(GamesEvent::Failed(error));
                }
            }
            Source::Done => {}
        }
        if let Ok(mut shared) = self.progress.lock() {
            *shared = progress;
        }
        if out
            .iter()
            .any(|e| matches!(e, GamesEvent::Finished | GamesEvent::Failed(_)))
        {
            self.close();
        }
        self.queued = out;
        self.queued.pop_front()
    }

    /// Encerra o fluxo; a conexão cai na próxima leitura.
    pub fn close(&mut self) {
        if let Source::Lichess { stream, .. } = &self.source {
            stream.close();
        }
        // Descartar o plano do Chess.com cancela o mês em andamento.
        self.source = Source::Done;
    }

    /// Opções de importação com a Origem deste fluxo: Provedor, modo API, instante da
    /// consulta, licença e o endereço consultado. O identificador remoto de cada Partida o
    /// importador lê das tags.
    pub fn import_options(&self) -> ImportOptions {
        import_options(&self.provenance)
    }

    /// Entrega o fluxo como um `Read` de PGN para o importador, que roda em segundo plano:
    /// **bloqueia** esperando a rede, então nunca o use na thread da interface. Ao ver
    /// `cancel` ligado a leitura termina (o importador também o vê e para).
    pub fn into_reader(self, cancel: Arc<AtomicBool>) -> GamesReader {
        GamesReader {
            feed: self,
            cancel,
            buffer: Vec::new(),
            position: 0,
            done: false,
        }
    }
}

/// Opções de importação da Biblioteca para uma Origem de API (ver
/// [`GamesFeed::import_options`]).
pub fn import_options(provenance: &chess_net::Provenance) -> ImportOptions {
    let source = match provenance.provider {
        ProviderId::Lichess => ImportSource::Lichess,
        ProviderId::ChessCom => ImportSource::ChessCom,
        _ => ImportSource::Other,
    };
    let mut origin = chess_store::Provenance::from_source(
        source,
        ImportMode::Api,
        provenance.queried_at.as_millis(),
    );
    origin.reference_url = Some(provenance.source.clone());
    ImportOptions::new(None, origin)
}

/// `Read` de PGN sobre um [`GamesFeed`] (ver [`GamesFeed::into_reader`]).
pub struct GamesReader {
    feed: GamesFeed,
    cancel: Arc<AtomicBool>,
    buffer: Vec<u8>,
    position: usize,
    done: bool,
}

impl fmt::Debug for GamesReader {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("GamesReader")
            .field("done", &self.done)
            .finish_non_exhaustive()
    }
}

impl GamesReader {
    pub fn progress(&self) -> GamesProgress {
        self.feed.progress()
    }
}

impl Read for GamesReader {
    fn read(&mut self, out: &mut [u8]) -> io::Result<usize> {
        loop {
            if self.position < self.buffer.len() {
                let n = out.len().min(self.buffer.len() - self.position);
                out[..n].copy_from_slice(&self.buffer[self.position..self.position + n]);
                self.position += n;
                return Ok(n);
            }
            if self.done || self.cancel.load(Ordering::Acquire) {
                self.feed.close();
                return Ok(0);
            }
            match self.feed.try_next() {
                Some(GamesEvent::Chunk(pgn)) => {
                    self.buffer = pgn.into_bytes();
                    if !self.buffer.ends_with(b"\n\n") {
                        self.buffer.extend_from_slice(b"\n\n");
                    }
                    self.position = 0;
                }
                Some(GamesEvent::Finished) => self.done = true,
                Some(GamesEvent::Failed(error)) => {
                    self.done = true;
                    return Err(io::Error::other(error));
                }
                None => std::thread::sleep(Duration::from_millis(10)),
            }
        }
    }
}

// ---- Lichess ------------------------------------------------------------------------

#[derive(Deserialize)]
struct WireGame {
    #[serde(default)]
    variant: String,
    #[serde(default)]
    pgn: Option<String>,
}

fn poll_lichess(
    provider: ProviderId,
    stream: &Stream,
    name: &str,
    out: &mut VecDeque<GamesEvent>,
    progress: &mut GamesProgress,
) {
    let Some(event) = stream.try_next() else {
        return;
    };
    match event {
        StreamEvent::Line(line) => match serde_json::from_str::<WireGame>(&line) {
            Ok(game) => {
                let accepted = chess_net::provider(provider).accepts_variant(&game.variant);
                match game.pgn {
                    Some(pgn) if accepted => {
                        progress.games += 1;
                        out.push_back(GamesEvent::Chunk(pgn));
                    }
                    _ => progress.skipped += 1,
                }
            }
            Err(e) => {
                stream.close();
                out.push_back(GamesEvent::Failed(ProviderError::malformed(provider, e)));
            }
        },
        StreamEvent::Ended => out.push_back(GamesEvent::Finished),
        StreamEvent::Failed(error) => {
            out.push_back(GamesEvent::Failed(Missing::name(name).translate(error)));
        }
    }
}

impl Lichess {
    /// Partidas públicas de `name` (`/api/games/user/{nome}`), em fluxo NDJSON com o PGN
    /// dentro de cada linha. Período, ritmo, cor e máximo vão no pedido. Só xadrez padrão:
    /// o pedido já pede os ritmos padrão e qualquer variante que escape é descartada
    /// (`progress.skipped`).
    pub fn games(&self, name: &str, filter: &GamesFilter) -> GamesFeed {
        let provider = self.profile.id;
        let provenance = stream_provenance(
            provider,
            self.profile.api_host,
            &format!("/api/games/user/{}", name.trim()),
            self.clock.as_ref(),
        );
        let name = match valid_name(name) {
            Ok(name) => name,
            Err(e) => return GamesFeed::failed(e, provenance),
        };
        let paces: Vec<&str> = if filter.paces.is_empty() {
            Pace::ALL.iter().map(|p| p.as_str()).collect()
        } else {
            filter.paces.iter().map(|p| p.as_str()).collect()
        };
        // «ultraBullet» é um ritmo do Lichess que o app junta ao bullet.
        let mut perf_type = paces.join(",");
        if paces.contains(&"bullet") {
            perf_type.push_str(",ultraBullet");
        }
        let mut request = Request::get(
            provider,
            self.profile.url(&format!("/api/games/user/{name}")),
        )
        .accept("application/x-ndjson")
        .query("pgnInJson", "true")
        .query("clocks", "false")
        .query("evals", "false")
        .query("opening", "false")
        .query("perfType", perf_type);
        if let Some(day) = filter.since {
            request = request.query("since", day.start_ms().to_string());
        }
        if let Some(day) = filter.until {
            request = request.query("until", day.end_ms().to_string());
        }
        if let Some(color) = filter.color {
            request = request.query(
                "color",
                match color {
                    Color::White => "white",
                    Color::Black => "black",
                },
            );
        }
        if let Some(max) = filter.max {
            request = request.query("max", max.to_string());
        }
        GamesFeed::lichess(provider, self.net.stream(request), name.to_owned(), provenance)
    }
}
