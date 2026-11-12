//! PubAPI do Chess.com (`api.chess.com/pub`): perfil, lista de arquivos mensais e PGN de um
//! mês. A API só lê e não filtra: período, ritmo e cor são aplicados **localmente** sobre o
//! PGN do mês (`crate::games::GamesFilter`). Os pedidos saem em série (o serviço pede um de
//! cada vez; a fila do Provedor na camada de rede e o plano de [`ChessCom::games`] só
//! mantêm um pedido em andamento) e a camada de rede respeita `ETag`/`If-None-Match`.

use crate::answer::{Answer, Query, fetch, json, stream_provenance, valid_name};
use crate::dates::{YearMonth, year_month_of_ms};
use crate::error::{Missing, ProviderError};
use crate::games::{GamesEvent, GamesFeed, GamesFilter, GamesProgress};
use crate::pgn::{Verdict, judge, split_games};
use crate::ttl;
use chess_net::{Clock, Net, ProviderId, Request};
use serde::Deserialize;
use std::collections::VecDeque;
use std::fmt;
use std::sync::Arc;
use std::time::Duration;

const PROVIDER: ProviderId = ProviderId::ChessCom;
const HOST: &str = "api.chess.com";
/// PGN de um mês grande passa de vários MB.
const MONTH_MAX_BODY: usize = 64 * 1024 * 1024;
const MONTH_TIMEOUT: Duration = Duration::from_secs(90);

/// Perfil público de um usuário do Chess.com.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ChessComProfile {
    pub username: String,
    pub name: Option<String>,
    pub title: Option<String>,
    pub url: Option<String>,
    /// Código do país (`US`), tirado do endereço que o Provedor devolve.
    pub country: Option<String>,
    pub followers: u64,
    /// Instante de entrada, em milissegundos UTC.
    pub joined_ms: Option<i64>,
    pub last_online_ms: Option<i64>,
    /// `basic`, `premium`, `staff`, `closed`…
    pub status: Option<String>,
}

#[derive(Deserialize)]
struct WireProfile {
    username: String,
    #[serde(default)]
    name: Option<String>,
    #[serde(default)]
    title: Option<String>,
    #[serde(default)]
    url: Option<String>,
    #[serde(default)]
    country: Option<String>,
    #[serde(default)]
    followers: u64,
    #[serde(default)]
    joined: Option<i64>,
    #[serde(default)]
    last_online: Option<i64>,
    #[serde(default)]
    status: Option<String>,
}

#[derive(Deserialize)]
struct WireArchives {
    archives: Vec<String>,
}

/// As Partidas de um mês depois do filtro local.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct MonthGames {
    /// Cada Partida em PGN completo, em ordem cronológica.
    pub games: Vec<String>,
    /// Partidas do mês descartadas pelo filtro ou por variante recusada.
    pub skipped: usize,
}

/// Cliente tipado do Chess.com PubAPI sobre `chess_net::Net`.
#[derive(Clone)]
pub struct ChessCom {
    net: Net,
    clock: Arc<dyn Clock>,
}

impl fmt::Debug for ChessCom {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ChessCom").finish_non_exhaustive()
    }
}

/// Teto de meses lidos da lista de arquivos mensais (os mais recentes, sem repetição):
/// 50 anos de Partidas; o Chess.com tem menos de 20 (SEC-07).
pub const MAX_ARCHIVE_MONTHS: usize = 600;

impl ChessCom {
    /// `clock` é o relógio injetado (o mesmo da `Net`).
    pub fn new(net: Net, clock: Arc<dyn Clock>) -> Self {
        Self { net, clock }
    }

    /// Perfil público de `name` (a PubAPI usa minúsculas). Nome inexistente:
    /// [`ProviderError::UnknownName`]. Validade do cache: [`ttl::CHESSCOM_PROFILE`].
    pub fn profile(&self, name: &str) -> Query<ChessComProfile> {
        let name = match valid_name(name) {
            Ok(name) => name.to_ascii_lowercase(),
            Err(e) => return Query::failed(e),
        };
        let request = Request::get(PROVIDER, format!("https://{HOST}/pub/player/{name}"))
            .cache_for(ttl::CHESSCOM_PROFILE);
        fetch(&self.net, request, Missing::name(&name), |fetched| {
            let wire: WireProfile = json(PROVIDER, fetched)?;
            Ok(ChessComProfile {
                username: wire.username,
                name: wire.name,
                title: wire.title,
                url: wire.url,
                country: wire
                    .country
                    .as_deref()
                    .and_then(|url| url.rsplit('/').next())
                    .map(str::to_owned),
                followers: wire.followers,
                joined_ms: wire.joined.map(|s| s.saturating_mul(1000)),
                last_online_ms: wire.last_online.map(|s| s.saturating_mul(1000)),
                status: wire.status,
            })
        })
    }

    /// Meses com Partidas de `name`, do mais antigo ao mais recente. Validade do cache:
    /// [`ttl::CHESSCOM_ARCHIVES`].
    pub fn archives(&self, name: &str) -> Query<Vec<YearMonth>> {
        let name = match valid_name(name) {
            Ok(name) => name.to_ascii_lowercase(),
            Err(e) => return Query::failed(e),
        };
        let request = Request::get(
            PROVIDER,
            format!("https://{HOST}/pub/player/{name}/games/archives"),
        )
        .cache_for(ttl::CHESSCOM_ARCHIVES);
        fetch(&self.net, request, Missing::name(&name), |fetched| {
            let wire: WireArchives = json(PROVIDER, fetched)?;
            let mut months = Vec::with_capacity(wire.archives.len());
            for url in &wire.archives {
                let mut tail = url.trim_end_matches('/').rsplit('/');
                let (month, year) = (tail.next(), tail.next());
                let parsed = month
                    .zip(year)
                    .and_then(|(m, y)| YearMonth::parse(&format!("{y}-{m}")));
                months.push(parsed.ok_or_else(|| {
                    ProviderError::malformed(PROVIDER, "endereço de arquivo mensal sem ano/mês")
                })?);
            }
            months.sort();
            months.dedup();
            // Os mais recentes: um pedido serial por mês seria o custo de uma lista hostil.
            months.drain(..months.len().saturating_sub(MAX_ARCHIVE_MONTHS));
            Ok(months)
        })
    }

    /// Partidas de `name` em `month`, depois do filtro local (`max` não se aplica aqui: vale
    /// no fluxo [`ChessCom::games`]). Mês sem arquivo: [`ProviderError::NoData`]. Validade do
    /// cache: [`ttl::CHESSCOM_CURRENT_MONTH`] no mês corrente, [`ttl::CHESSCOM_PAST_MONTH`]
    /// nos outros.
    pub fn month_games(
        &self,
        name: &str,
        month: YearMonth,
        filter: &GamesFilter,
    ) -> Query<MonthGames> {
        let name = match valid_name(name) {
            Ok(name) => name.to_ascii_lowercase(),
            Err(e) => return Query::failed(e),
        };
        let current = year_month_of_ms(self.clock.now().as_millis());
        let ttl = if month >= current {
            ttl::CHESSCOM_CURRENT_MONTH
        } else {
            ttl::CHESSCOM_PAST_MONTH
        };
        let request = Request::get(
            PROVIDER,
            format!(
                "https://{HOST}/pub/player/{name}/games/{:04}/{:02}/pgn",
                month.year, month.month
            ),
        )
        .cache_for(ttl)
        .max_body(MONTH_MAX_BODY)
        .timeout(MONTH_TIMEOUT);
        let filter = filter.clone();
        let user = name.clone();
        fetch(&self.net, request, Missing::NoData, move |fetched| {
            let text = fetched
                .text()
                .ok_or_else(|| ProviderError::malformed(PROVIDER, "o PGN do mês não é UTF-8"))?;
            let mut month_games = MonthGames::default();
            for game in split_games(text) {
                match judge(game, &user, &filter) {
                    Verdict::Keep => month_games.games.push(game.trim_end().to_owned()),
                    Verdict::Filtered | Verdict::Variant => month_games.skipped += 1,
                }
            }
            Ok(month_games)
        })
    }

    /// Partidas públicas de `name` em fluxo, as mais recentes primeiro, mês a mês e **em
    /// série**: um pedido só depois do outro. Entrega para o importador da Biblioteca como
    /// [`GamesFeed`].
    pub fn games(&self, name: &str, filter: &GamesFilter) -> GamesFeed {
        let provenance = stream_provenance(
            PROVIDER,
            HOST,
            &format!("/pub/player/{}/games", name.trim().to_ascii_lowercase()),
            self.clock.as_ref(),
        );
        let user = match valid_name(name) {
            Ok(name) => name.to_ascii_lowercase(),
            Err(e) => return GamesFeed::failed(e, provenance),
        };
        let archives = self.archives(&user);
        GamesFeed::chesscom(
            ChessComPlan {
                client: self.clone(),
                user,
                filter: filter.clone(),
                state: PlanState::Archives(archives),
                months: VecDeque::new(),
                delivered: 0,
            },
            provenance,
        )
    }
}

enum PlanState {
    Archives(Query<Vec<YearMonth>>),
    Month(Query<MonthGames>),
    Done,
}

/// Máquina do fluxo do Chess.com: lista de meses, depois um mês de cada vez.
pub(crate) struct ChessComPlan {
    client: ChessCom,
    user: String,
    filter: GamesFilter,
    state: PlanState,
    /// Do mais recente ao mais antigo.
    months: VecDeque<YearMonth>,
    delivered: u32,
}

impl ChessComPlan {
    pub(crate) fn poll(&mut self, out: &mut VecDeque<GamesEvent>, progress: &mut GamesProgress) {
        match &mut self.state {
            PlanState::Archives(query) => match query.try_take() {
                None => {}
                Some(Err(error)) => self.fail(error, out),
                Some(Ok(Answer { value: months, .. })) => {
                    let since = self.filter.since.map(|d| d.year_month());
                    let until = self.filter.until.map(|d| d.year_month());
                    self.months = months
                        .into_iter()
                        .rev()
                        .filter(|m| since.is_none_or(|s| *m >= s) && until.is_none_or(|u| *m <= u))
                        .collect();
                    progress.months_total = Some(self.months.len());
                    self.start_next(out);
                }
            },
            PlanState::Month(query) => match query.try_take() {
                None => {}
                // Mês sem arquivo: segue para o próximo.
                Some(Err(ProviderError::NoData)) => {
                    progress.months_done += 1;
                    self.start_next(out);
                }
                Some(Err(error)) => self.fail(error, out),
                Some(Ok(answer)) => {
                    progress.months_done += 1;
                    let MonthGames { mut games, skipped } = answer.value;
                    progress.skipped += skipped;
                    if let Some(max) = self.filter.max {
                        let room = (max - self.delivered) as usize;
                        if games.len() > room {
                            // As mais recentes do mês são as últimas do PGN.
                            games.drain(..games.len() - room);
                        }
                    }
                    if !games.is_empty() {
                        self.delivered += u32::try_from(games.len()).unwrap_or(u32::MAX);
                        progress.games += games.len();
                        let mut text = games.join("\n\n");
                        text.push_str("\n\n");
                        out.push_back(GamesEvent::Chunk(text));
                    }
                    self.start_next(out);
                }
            },
            PlanState::Done => {}
        }
    }

    fn start_next(&mut self, out: &mut VecDeque<GamesEvent>) {
        let reached_max = self.filter.max.is_some_and(|max| self.delivered >= max);
        match self.months.pop_front() {
            Some(month) if !reached_max => {
                let query = self.client.month_games(&self.user, month, &self.filter);
                self.state = PlanState::Month(query);
            }
            _ => {
                self.state = PlanState::Done;
                out.push_back(GamesEvent::Finished);
            }
        }
    }

    fn fail(&mut self, error: ProviderError, out: &mut VecDeque<GamesEvent>) {
        self.state = PlanState::Done;
        out.push_back(GamesEvent::Failed(error));
    }
}
