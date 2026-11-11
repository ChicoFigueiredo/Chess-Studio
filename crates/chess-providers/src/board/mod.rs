//! Board API do Lichess (#230, ADR-0007, CX-15): jogar como pessoa, sem interface.
//!
//! - [`LichessBoard`]: o cliente, sempre por uma Conexão **de jogador** (`board:play`, pelo
//!   Cofre); uma Conexão de Engine é recusada ([`BoardError::EngineConnection`]).
//! - [`ConnectionEvents`]: o fluxo de eventos da Conexão (`/api/stream/event`): desafios,
//!   começo e fim de Partidas, com reconexão.
//! - Ações sem Partida aberta: buscar adversário ([`LichessBoard::seek`], tempo real e
//!   correspondência, com os limites da Board API), desafiar por nome, aceitar, recusar e
//!   cancelar desafio, desafiar a Engine do Lichess e listar as Partidas em andamento.
//! - [`OnlineGameSession`]: a sessão de uma Partida online, consultável por quadro.
//!
//! ```no_run
//! use chess_providers::{GameInfo, LichessBoard, SessionNotice};
//! # fn demo(board: LichessBoard, info: GameInfo, store: &mut chess_store::Store) {
//! // O fluxo de eventos avisou `gameStart` (ou `playing()` listou a Partida):
//! let mut session = board.open_game(&info);
//! let mut id = None;
//! // A cada quadro (nunca bloqueia):
//! let polled = session.poll();
//! if polled.created {
//!     let game = session.game().expect("criada");
//!     id = store.create_game(game.setup(), &Default::default()).ok();
//! }
//! if let Some(id) = id {
//!     store.append_events(id, &polled.events).ok(); // só o que o Provedor confirmou
//! }
//! for notice in polled.notices {
//!     if let SessionNotice::MoveRefused { mv, .. } = notice {
//!         println!("o Lichess recusou {mv}");
//!     }
//! }
//! // O jogador soltou a peça: intenção enviada, um único POST.
//! # let mv = "e2e4".parse().unwrap();
//! let _ = session.play(mv);
//! # }
//! ```

mod json;
mod link;
mod session;

pub use link::{Backoff, LinkStatus};
pub use session::{BoardAction, Gap, OnlineGameSession, OpponentGone, SessionNotice, SessionPoll};

use crate::ProviderError;
use crate::answer::{self, Query};
use crate::auth::{ConnectionRole, form};
use crate::error::Missing;
use crate::lichess::{API_HOST, PROVIDER};
use chess_core::Color;
use chess_net::{Clock, ConnectionId, Net, Request, Stream, StreamEvent, Timestamp};
use link::{Jitter, Link, LinkItem};
use serde::Deserialize;
use std::collections::hash_map::RandomState;
use std::fmt;
use std::hash::{BuildHasher, Hasher};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, PoisonError};

/// Por que a Board API recusou algo antes (ou fora) da rede. Em português, sem segredo.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum BoardError {
    /// A Conexão é de Engine (`bot:play`): ela nunca joga como pessoa.
    EngineConnection,
    /// Só xadrez padrão: a Partida não foi criada.
    UnsupportedVariant(String),
    /// A Board API só busca adversário em ritmo rápido (rapid) ou mais lento; blitz e mais
    /// rápido, só por desafio direto.
    TooFast,
    /// Já há uma busca de adversário aberta.
    SeekInProgress,
    /// Agora não dá (Partida não começou, já terminou, prazo de reclamar não chegou…).
    NotAllowed(&'static str),
    /// A Partida do Provedor não é a desta sessão, ou a Partida local não é do Lichess.
    WrongGame,
    /// O domínio recusou (Lance ilegal, não é a vez, dessincronizada…).
    Rejected(chess_game::Rejection),
    /// Falha do Provedor ou da rede.
    Provider(ProviderError),
}

impl fmt::Display for BoardError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::EngineConnection => f.write_str(
                "Esta é uma Conexão de Engine: ela não joga como pessoa. Use a Conexão do jogador.",
            ),
            Self::UnsupportedVariant(v) => write!(
                f,
                "A variante «{v}» não é aceita: o app só joga xadrez padrão. Nada foi criado."
            ),
            Self::TooFast => f.write_str(
                "O Lichess só busca adversário pela Board API em ritmo rápido ou mais lento; \
                 para blitz, desafie alguém pelo nome.",
            ),
            Self::SeekInProgress => f.write_str("Já há uma busca de adversário em andamento."),
            Self::NotAllowed(why) => write!(f, "Agora não: {why}."),
            Self::WrongGame => f.write_str("A Partida do Lichess não é a desta sessão."),
            Self::Rejected(r) => r.fmt(f),
            Self::Provider(e) => e.fmt(f),
        }
    }
}

impl std::error::Error for BoardError {}

/// Ritmo de uma Partida online.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum OnlineTime {
    /// Tempo real: base e incremento em segundos.
    RealTime { base_s: u32, increment_s: u32 },
    /// Correspondência: dias por Lance.
    Correspondence { days: u8 },
    /// Sem limite (só por desafio direto).
    Unlimited,
}

/// Dias por Lance que o Lichess aceita na correspondência.
pub const CORRESPONDENCE_DAYS: [u8; 7] = [1, 2, 3, 5, 7, 10, 14];

/// Duração estimada (base + 40 incrementos) a partir da qual o Lichess chama de rápido.
const RAPID_MIN_ESTIMATE_S: u32 = 8 * 60;

/// Desafio por nome (ou à Engine do Lichess). Sempre xadrez padrão.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ChallengeSpec {
    pub time: OnlineTime,
    pub rated: bool,
    /// `None`: cor sorteada.
    pub color: Option<Color>,
}

/// Busca de adversário (`/api/board/seek`). Sempre xadrez padrão.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Seek {
    pub time: OnlineTime,
    pub rated: bool,
    pub color: Option<Color>,
}

impl Seek {
    /// Tempo real, casual, cor sorteada.
    pub fn real_time(base_s: u32, increment_s: u32) -> Self {
        Self {
            time: OnlineTime::RealTime {
                base_s,
                increment_s,
            },
            rated: false,
            color: None,
        }
    }

    /// Correspondência, casual.
    pub fn correspondence(days: u8) -> Self {
        Self {
            time: OnlineTime::Correspondence { days },
            rated: false,
            color: None,
        }
    }
}

/// Como vai a busca.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SeekState {
    /// Tempo real: aberta, à espera de adversário.
    Waiting,
    /// Correspondência: publicada com este id; fica no Lichess até alguém aceitar.
    Posted {
        id: String,
    },
    /// Tempo real: o Lichess fechou a busca (pareou ou expirou). O começo da Partida vem pelo
    /// fluxo de eventos ([`ConnectionEvent::GameStarted`]).
    Ended,
    Failed(ProviderError),
}

/// Adversário de uma Partida em [`GameInfo`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OpponentInfo {
    pub name: String,
    pub rating: Option<u32>,
    /// Nível da Engine do Lichess (1 a 8), se o adversário é ela.
    pub ai_level: Option<u8>,
}

/// Uma Partida da Conexão, como o fluxo de eventos e `/api/account/playing` a descrevem.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(try_from = "json::GameInfoJson")]
pub struct GameInfo {
    pub game_id: String,
    /// Lado do jogador local.
    pub color: Color,
    pub opponent: OpponentInfo,
    pub variant: String,
    /// `rapid`, `classical`, `correspondence`…
    pub speed: String,
    pub rated: bool,
    pub is_my_turn: bool,
    /// Situação no Lichess (`started`, `mate`, `resign`…).
    pub status: Option<String>,
    pub winner: Option<Color>,
    pub fen: Option<String>,
    pub last_move: Option<String>,
    pub seconds_left: Option<u64>,
}

/// Jogador de um desafio.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ChallengePlayer {
    pub id: String,
    pub name: String,
    pub rating: Option<u32>,
}

/// Um desafio recebido ou feito.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(try_from = "json::ChallengeJson")]
pub struct Challenge {
    pub id: String,
    pub challenger: ChallengePlayer,
    pub dest: Option<ChallengePlayer>,
    pub variant: String,
    pub rated: bool,
    pub speed: String,
    pub time: OnlineTime,
    /// `white`, `black` ou `random` (cor de quem desafiou).
    pub color: String,
}

/// Motivo de recusar um desafio (os aceitos pelo Lichess).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DeclineReason {
    Generic,
    Later,
    TooFast,
    TooSlow,
    TimeControl,
    Rated,
    Casual,
    Standard,
    Variant,
}

impl DeclineReason {
    const fn key(self) -> &'static str {
        match self {
            Self::Generic => "generic",
            Self::Later => "later",
            Self::TooFast => "tooFast",
            Self::TooSlow => "tooSlow",
            Self::TimeControl => "timeControl",
            Self::Rated => "rated",
            Self::Casual => "casual",
            Self::Standard => "standard",
            Self::Variant => "variant",
        }
    }
}

/// O que chegou pelo fluxo de eventos da Conexão.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ConnectionEvent {
    GameStarted(GameInfo),
    GameFinished(GameInfo),
    Challenge(Challenge),
    ChallengeCanceled {
        id: String,
    },
    ChallengeDeclined {
        id: String,
        reason: Option<String>,
    },
    /// O fluxo parou por um erro que não passa sozinho (Conexão revogada, Provedor
    /// desligado); nada mais chega.
    Closed(ProviderError),
}

/// Cliente da Board API do Lichess, por uma Conexão de jogador. Barato de clonar.
#[derive(Clone)]
pub struct LichessBoard {
    pub(crate) net: Net,
    pub(crate) clock: Arc<dyn Clock>,
    pub(crate) connection: ConnectionId,
    pub(crate) backoff: Backoff,
    seeds: Arc<Mutex<Jitter>>,
    seeking: Arc<AtomicBool>,
}

impl fmt::Debug for LichessBoard {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("LichessBoard")
            .field("connection", &self.connection)
            .finish_non_exhaustive()
    }
}

fn is_name(text: &str) -> bool {
    (1..=30).contains(&text.len())
        && text
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b == b'_' || b == b'-')
}

fn is_id(text: &str) -> bool {
    (1..=20).contains(&text.len()) && text.bytes().all(|b| b.is_ascii_alphanumeric())
}

/// `600 s` vira `10`; `30 s`, `0.5` (o Lichess aceita minutos fracionários na busca).
fn minutes(base_s: u32) -> String {
    if base_s.is_multiple_of(60) {
        (base_s / 60).to_string()
    } else {
        format!("{}", f64::from(base_s) / 60.0)
    }
}

fn color_key(color: Option<Color>) -> &'static str {
    match color {
        Some(Color::White) => "white",
        Some(Color::Black) => "black",
        None => "random",
    }
}

impl LichessBoard {
    /// O cliente da Board API sobre a camada de rede. `role` é o papel da Conexão na
    /// Central: só a do jogador joga como pessoa.
    pub fn new(
        net: Net,
        clock: Arc<dyn Clock>,
        connection: ConnectionId,
        role: ConnectionRole,
    ) -> Result<Self, BoardError> {
        if role != ConnectionRole::Player {
            return Err(BoardError::EngineConnection);
        }
        let mut hasher = RandomState::new().build_hasher();
        hasher.write_u64(0xb0a2d);
        Ok(Self {
            net,
            clock,
            connection,
            backoff: Backoff::default(),
            seeds: Arc::new(Mutex::new(Jitter::new(hasher.finish()))),
            seeking: Arc::default(),
        })
    }

    /// Espera entre reconexões dos fluxos.
    pub fn backoff(mut self, backoff: Backoff) -> Self {
        self.backoff = backoff;
        self
    }

    /// Semente do jitter (testes reprodutíveis).
    pub fn jitter_seed(self, seed: u64) -> Self {
        *self.seeds.lock().unwrap_or_else(PoisonError::into_inner) = Jitter::new(seed);
        self
    }

    pub(crate) fn seed(&self) -> u64 {
        self.seeds
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .next()
    }

    pub(crate) fn url(path: &str) -> String {
        format!("https://{API_HOST}{path}")
    }

    pub(crate) fn get(&self, path: &str) -> Request {
        Request::get(PROVIDER, Self::url(path)).connection(self.connection)
    }

    pub(crate) fn post(&self, path: &str, body: Option<String>) -> Request {
        let request = Request::post(PROVIDER, Self::url(path)).connection(self.connection);
        match body {
            Some(body) => request.body("application/x-www-form-urlencoded", body.into_bytes()),
            None => request,
        }
    }

    pub(crate) fn link(&self, request: Request) -> Link {
        Link::open(
            self.net.clone(),
            self.clock.clone(),
            request,
            self.backoff,
            self.seed(),
        )
    }

    /// Abre o fluxo de eventos da Conexão. O Lichess mantém um só por token: abrir outro
    /// fecha o anterior.
    pub fn events(&self) -> ConnectionEvents {
        ConnectionEvents {
            link: self.link(self.get("/api/stream/event")),
        }
    }

    /// Busca adversário. Tempo real: a busca vale enquanto o [`Seeking`] existir (o Lichess
    /// a cancela quando a conexão fecha); abra antes o fluxo de eventos, por onde vem o
    /// começo da Partida. Correspondência: publicada na hora. Uma busca por vez; `429` vira
    /// [`ProviderError::RateLimited`] e nada sai antes do prazo.
    pub fn seek(&self, seek: &Seek) -> Result<Seeking, BoardError> {
        let mut pairs: Vec<(&str, String)> = vec![
            ("rated", seek.rated.to_string()),
            ("variant", "standard".to_owned()),
        ];
        let live = match seek.time {
            OnlineTime::RealTime {
                base_s,
                increment_s,
            } => {
                if base_s > 180 * 60 || increment_s > 180 {
                    return Err(BoardError::Provider(ProviderError::invalid(
                        "ritmo acima do máximo do Lichess (180 min + 180 s)",
                    )));
                }
                if base_s.saturating_add(increment_s.saturating_mul(40)) < RAPID_MIN_ESTIMATE_S {
                    return Err(BoardError::TooFast);
                }
                pairs.push(("time", minutes(base_s)));
                pairs.push(("increment", increment_s.to_string()));
                if seek.color.is_some() {
                    pairs.push(("color", color_key(seek.color).to_owned()));
                }
                true
            }
            OnlineTime::Correspondence { days } => {
                if !CORRESPONDENCE_DAYS.contains(&days) {
                    return Err(BoardError::Provider(ProviderError::invalid(format!(
                        "o Lichess aceita {CORRESPONDENCE_DAYS:?} dias por Lance"
                    ))));
                }
                pairs.push(("days", days.to_string()));
                false
            }
            OnlineTime::Unlimited => {
                return Err(BoardError::Provider(ProviderError::invalid(
                    "busca de adversário precisa de ritmo",
                )));
            }
        };
        if self.seeking.swap(true, Ordering::AcqRel) {
            return Err(BoardError::SeekInProgress);
        }
        let slot = SeekSlot(self.seeking.clone());
        let refs: Vec<(&str, &str)> = pairs.iter().map(|(k, v)| (*k, v.as_str())).collect();
        let request = self.post("/api/board/seek", Some(form(&refs)));
        let kind = if live {
            SeekKind::Live(self.net.stream(request))
        } else {
            SeekKind::Posted(answer::fetch(
                &self.net,
                request,
                Missing::NoData,
                |fetched| Ok(answer::json::<json::IdJson>(PROVIDER, fetched)?.id),
            ))
        };
        Ok(Seeking {
            kind,
            state: None,
            _slot: slot,
        })
    }

    fn challenge_pairs(spec: &ChallengeSpec) -> Vec<(&'static str, String)> {
        let mut pairs = vec![
            ("rated", spec.rated.to_string()),
            ("variant", "standard".to_owned()),
            ("color", color_key(spec.color).to_owned()),
        ];
        match spec.time {
            OnlineTime::RealTime {
                base_s,
                increment_s,
            } => {
                pairs.push(("clock.limit", base_s.to_string()));
                pairs.push(("clock.increment", increment_s.to_string()));
            }
            OnlineTime::Correspondence { days } => pairs.push(("days", days.to_string())),
            OnlineTime::Unlimited => {}
        }
        pairs
    }

    fn check_spec(spec: &ChallengeSpec) -> Result<(), ProviderError> {
        match spec.time {
            OnlineTime::RealTime {
                base_s,
                increment_s,
            } if base_s > 10_800 || increment_s > 60 => Err(ProviderError::invalid(
                "ritmo acima do máximo do Lichess (180 min + 60 s)",
            )),
            OnlineTime::Correspondence { days } if !CORRESPONDENCE_DAYS.contains(&days) => {
                Err(ProviderError::invalid(format!(
                    "o Lichess aceita {CORRESPONDENCE_DAYS:?} dias por Lance"
                )))
            }
            _ => Ok(()),
        }
    }

    fn post_query<T: 'static>(
        &self,
        path: &str,
        body: Option<String>,
        decode: impl FnOnce(&chess_net::Fetched) -> Result<T, ProviderError> + Send + 'static,
    ) -> Query<T> {
        answer::fetch(&self.net, self.post(path, body), Missing::NoData, decode)
    }

    /// Desafia `username` (xadrez padrão). O começo da Partida vem pelo fluxo de eventos.
    pub fn challenge(&self, username: &str, spec: &ChallengeSpec) -> Query<Challenge> {
        if !is_name(username) {
            return Query::failed(ProviderError::invalid("nome do Lichess inválido"));
        }
        if let Err(e) = Self::check_spec(spec) {
            return Query::failed(e);
        }
        let pairs = Self::challenge_pairs(spec);
        let refs: Vec<(&str, &str)> = pairs.iter().map(|(k, v)| (*k, v.as_str())).collect();
        let username = username.to_owned();
        answer::fetch(
            &self.net,
            self.post(&format!("/api/challenge/{username}"), Some(form(&refs))),
            Missing::name(&username),
            |fetched| answer::json::<Challenge>(PROVIDER, fetched),
        )
    }

    /// Aceita um desafio recebido. Só xadrez padrão: outra variante é recusada antes da rede.
    pub fn accept_challenge(&self, challenge: &Challenge) -> Query<()> {
        if let chess_game::Variant::Other(variant) = json::variant(&challenge.variant) {
            return Query::failed(ProviderError::VariantRefused {
                provider: PROVIDER,
                variant,
            });
        }
        if !is_id(&challenge.id) {
            return Query::failed(ProviderError::invalid("id de desafio inválido"));
        }
        self.post_query(
            &format!("/api/challenge/{}/accept", challenge.id),
            None,
            |_| Ok(()),
        )
    }

    pub fn decline_challenge(&self, id: &str, reason: DeclineReason) -> Query<()> {
        if !is_id(id) {
            return Query::failed(ProviderError::invalid("id de desafio inválido"));
        }
        self.post_query(
            &format!("/api/challenge/{id}/decline"),
            Some(form(&[("reason", reason.key())])),
            |_| Ok(()),
        )
    }

    /// Cancela um desafio feito por este jogador.
    pub fn cancel_challenge(&self, id: &str) -> Query<()> {
        if !is_id(id) {
            return Query::failed(ProviderError::invalid("id de desafio inválido"));
        }
        self.post_query(&format!("/api/challenge/{id}/cancel"), None, |_| Ok(()))
    }

    /// Desafia a Engine do Lichess (nível 1 a 8); devolve o id da Partida, que começa na
    /// hora. Contra ela não há Trava de fair play.
    pub fn challenge_engine(&self, level: u8, spec: &ChallengeSpec) -> Query<String> {
        if !(1..=8).contains(&level) {
            return Query::failed(ProviderError::invalid(
                "a Engine do Lichess vai do nível 1 ao 8",
            ));
        }
        if let Err(e) = Self::check_spec(spec) {
            return Query::failed(e);
        }
        let mut pairs = Self::challenge_pairs(spec);
        pairs.push(("level", level.to_string()));
        let refs: Vec<(&str, &str)> = pairs.iter().map(|(k, v)| (*k, v.as_str())).collect();
        self.post_query("/api/challenge/ai", Some(form(&refs)), |fetched| {
            Ok(answer::json::<json::IdJson>(PROVIDER, fetched)?.id)
        })
    }

    /// Partidas em andamento da Conexão (para retomar a correspondência).
    pub fn playing(&self) -> Query<Vec<GameInfo>> {
        answer::fetch(
            &self.net,
            self.get("/api/account/playing").query("nb", "50"),
            Missing::NoData,
            |fetched| Ok(answer::json::<json::PlayingJson>(PROVIDER, fetched)?.now_playing),
        )
    }

    /// Sessão de uma Partida nova (avisada por `gameStart` ou listada em `playing`). A
    /// Partida nasce do primeiro `gameFull`; contra pessoa, a Trava de fair play liga já.
    pub fn open_game(&self, info: &GameInfo) -> OnlineGameSession {
        OnlineGameSession::open(self.clone(), info)
    }

    /// Sessão de uma Partida que já está no log (correspondência, reabrir o app): continua
    /// pelo retrato do Provedor.
    pub fn resume_game(&self, game: chess_game::Game) -> Result<OnlineGameSession, BoardError> {
        OnlineGameSession::resume(self.clone(), game)
    }
}

/// Libera a vaga de busca quando o [`Seeking`] cai.
#[derive(Debug)]
struct SeekSlot(Arc<AtomicBool>);

impl Drop for SeekSlot {
    fn drop(&mut self) {
        self.0.store(false, Ordering::Release);
    }
}

#[derive(Debug)]
enum SeekKind {
    Live(Stream),
    Posted(Query<String>),
}

/// Busca de adversário aberta. Descartar fecha a busca em tempo real (o Lichess a cancela).
#[derive(Debug)]
pub struct Seeking {
    kind: SeekKind,
    state: Option<SeekState>,
    _slot: SeekSlot,
}

impl Seeking {
    /// Nunca bloqueia.
    pub fn poll(&mut self) -> SeekState {
        if let Some(state) = &self.state {
            return state.clone();
        }
        let state = match &mut self.kind {
            SeekKind::Live(stream) => {
                let mut state = None;
                while let Some(event) = stream.try_next() {
                    match event {
                        StreamEvent::Line(_) => {}
                        StreamEvent::Ended => {
                            state = Some(SeekState::Ended);
                            break;
                        }
                        StreamEvent::Failed(e) => {
                            state = Some(SeekState::Failed(e.into()));
                            break;
                        }
                    }
                }
                state
            }
            SeekKind::Posted(query) => query.try_take().map(|result| match result {
                Ok(answer) => SeekState::Posted { id: answer.value },
                Err(e) => SeekState::Failed(e),
            }),
        };
        match state {
            Some(state) => {
                self.state = Some(state.clone());
                state
            }
            None => SeekState::Waiting,
        }
    }

    /// Cancela a busca em tempo real (fecha a conexão).
    pub fn cancel(&self) {
        match &self.kind {
            SeekKind::Live(stream) => stream.close(),
            SeekKind::Posted(query) => query.cancel(),
        }
    }
}

/// Fluxo de eventos da Conexão, consultável por quadro, com reconexão por backoff.
#[derive(Debug)]
pub struct ConnectionEvents {
    link: Link,
}

impl ConnectionEvents {
    /// Nunca bloqueia: o que chegou desde a última consulta.
    pub fn poll(&mut self) -> Vec<ConnectionEvent> {
        let mut out = Vec::new();
        for item in self.link.poll() {
            match item {
                LinkItem::Line(line) => out.extend(connection_event(&line)),
                LinkItem::Restored { .. } => {}
                LinkItem::Ended => self.link.retry(None),
                LinkItem::Failed(error) => {
                    if link::is_fatal(&error) {
                        self.link.close();
                        out.push(ConnectionEvent::Closed(error.into()));
                    } else {
                        let hint = link::not_before(&error);
                        self.link.retry(hint);
                    }
                }
            }
        }
        out
    }

    pub fn status(&self) -> LinkStatus {
        self.link.status()
    }

    /// Quando reabre, se o fluxo caiu.
    pub fn retry_at(&self) -> Option<Timestamp> {
        match self.link.status() {
            LinkStatus::Retrying { at, .. } => Some(at),
            _ => None,
        }
    }

    pub fn close(&mut self) {
        self.link.close();
    }
}

fn connection_event(line: &str) -> Option<ConnectionEvent> {
    let value: serde_json::Value = serde_json::from_str(line).ok()?;
    let take = |key: &str| value.get(key).cloned().unwrap_or_default();
    match value.get("type")?.as_str()? {
        "gameStart" => serde_json::from_value(take("game"))
            .ok()
            .map(ConnectionEvent::GameStarted),
        "gameFinish" => serde_json::from_value(take("game"))
            .ok()
            .map(ConnectionEvent::GameFinished),
        "challenge" => serde_json::from_value(take("challenge"))
            .ok()
            .map(ConnectionEvent::Challenge),
        "challengeCanceled" => serde_json::from_value::<json::IdJson>(take("challenge"))
            .ok()
            .map(|c| ConnectionEvent::ChallengeCanceled { id: c.id }),
        "challengeDeclined" => serde_json::from_value::<json::DeclinedJson>(take("challenge"))
            .ok()
            .map(|c| ConnectionEvent::ChallengeDeclined {
                id: c.id,
                reason: c.decline_reason_key,
            }),
        _ => None,
    }
}
