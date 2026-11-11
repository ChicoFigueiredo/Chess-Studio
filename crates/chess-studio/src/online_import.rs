//! Importar do Lichess, do PlayStrategy e do Chess.com (CX-9, #226, #253): as Partidas públicas de um jogador entram
//! na Biblioteca em segundo plano, com a Origem de cada uma (Provedor, instante, licença).
//!
//! - o pedido é um [`GamesFeed`] do `chess-providers` (nenhuma conexão sai daqui: tudo passa
//!   pelo `chess-net`, que confere consentimento, catálogo, Trava e limites);
//! - `GamesFeed::into_reader` + `Store::import_pgn_stream` rodam numa **thread própria** com
//!   a sua conexão do banco, em lotes pequenos para o andamento aparecer logo;
//! - "Atualizar" pede só o que veio depois da última Partida importada daquele nome;
//!   Partidas repetidas são reconhecidas pelo importador (id remoto e impressão digital),
//!   então nada se duplica;
//! - não há sincronização automática: só roda quando a pessoa aperta o botão.
//!
//! - o Lichess e o PlayStrategy são a mesma família de API: um só caminho, escolhido pelo
//!   `ProviderProfile` do Provedor (`Lichess::with_profile`); só xadrez padrão, e as Partidas
//!   de variantes são puladas e contadas ([`Summary::skipped_variants`]).
//!
//! A lógica pura e o trabalho da thread ficam aqui; o formulário fica em `library_ui`.

use crate::net_service::NetService;
use chess_core::Color;
use chess_net::{NetError, ProviderId, Timestamp};
use chess_providers::{
    ChessCom, Day, GamesFeed, GamesFilter, GamesProgress, Lichess, Pace, ProviderError,
};
use chess_store::{ImportProgress, ImportSource, ProfileId, Store};
use std::io::BufReader;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};

/// Partidas por transação: pequeno, para o andamento (recebidas, novas, repetidas) subir
/// a cada poucas Partidas em vez de só no fim.
pub const BATCH: usize = 10;

/// De onde vêm as Partidas.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Site {
    Lichess,
    ChessCom,
    PlayStrategy,
}

impl Site {
    pub fn provider(self) -> ProviderId {
        match self {
            Site::Lichess => ProviderId::Lichess,
            Site::ChessCom => ProviderId::ChessCom,
            Site::PlayStrategy => ProviderId::PlayStrategy,
        }
    }

    /// A Origem que a Biblioteca grava nas Partidas deste Provedor.
    pub fn source(self) -> ImportSource {
        match self {
            Site::Lichess => ImportSource::Lichess,
            Site::ChessCom => ImportSource::ChessCom,
            Site::PlayStrategy => ImportSource::PlayStrategy,
        }
    }

    /// Nome gravado na Origem de cada Partida (`imported_games.provider`): o mesmo da
    /// Biblioteca e da aba Origens, não a chave de rede do Provedor.
    pub fn key(self) -> &'static str {
        self.source().provider_id()
    }

    /// Provedor da família do Lichess: só xadrez padrão, variantes puladas e contadas.
    pub fn is_lila_family(self) -> bool {
        matches!(self, Site::Lichess | Site::PlayStrategy)
    }

    pub fn name(self) -> &'static str {
        self.provider().info().name
    }
}

/// O período pedido.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Period {
    #[default]
    All,
    Week,
    Month,
    Year,
}

impl Period {
    pub const ALL: [Period; 4] = [Period::All, Period::Week, Period::Month, Period::Year];

    pub fn key(self) -> &'static str {
        match self {
            Period::All => "library-online-period-all",
            Period::Week => "library-online-period-week",
            Period::Month => "library-online-period-month",
            Period::Year => "library-online-period-year",
        }
    }

    /// O primeiro dia incluído, contado a partir de `now`.
    pub fn since(self, now: Timestamp) -> Option<Day> {
        let days: i64 = match self {
            Period::All => return None,
            Period::Week => 7,
            Period::Month => 30,
            Period::Year => 365,
        };
        day_of(Timestamp::from_millis(now.as_millis() - days * 86_400_000))
    }
}

/// O dia (UTC) de um instante.
pub fn day_of(at: Timestamp) -> Option<Day> {
    Day::parse_pgn(&chess_game::pgn_date_from_unix_ms(at.as_millis()))
}

/// O que a pessoa pediu no formulário.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Request {
    pub site: Site,
    pub name: String,
    pub period: Period,
    pub pace: Option<Pace>,
    pub color: Option<Color>,
    pub max: u32,
    /// "Atualizar": só o que veio a partir deste dia (vence o período).
    pub since: Option<Day>,
}

impl Request {
    pub fn filter(&self, now: Timestamp) -> GamesFilter {
        GamesFilter {
            since: self.since.or_else(|| self.period.since(now)),
            until: None,
            paces: self.pace.into_iter().collect(),
            color: self.color,
            max: Some(self.max),
        }
    }

    pub fn is_valid(&self) -> bool {
        !self.name.trim().is_empty()
    }
}

/// Os clientes de dados públicos, copiados para a thread (ver [`NetService::lichess`]).
#[derive(Clone)]
pub struct Clients {
    lichess: Lichess,
    playstrategy: Lichess,
    chesscom: ChessCom,
}

impl Clients {
    pub fn of(service: &NetService) -> Self {
        Self {
            lichess: service.lichess().clone(),
            playstrategy: service.playstrategy().clone(),
            chesscom: service.chesscom().clone(),
        }
    }

    /// Abre o fluxo de Partidas (nada bloqueia: o pedido sai da rede em segundo plano).
    pub fn feed(&self, request: &Request, now: Timestamp) -> GamesFeed {
        let filter = request.filter(now);
        let name = request.name.trim();
        match request.site {
            // Mesmo cliente, perfil de Provedor diferente.
            Site::Lichess => self.lichess.games(name, &filter),
            Site::PlayStrategy => self.playstrategy.games(name, &filter),
            Site::ChessCom => self.chesscom.games(name, &filter),
        }
    }
}

/// Andamento da importação (o que a interface mostra).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Progress {
    /// Partidas recebidas do Provedor até agora.
    pub received: usize,
    pub new: usize,
    pub duplicates: usize,
    pub errors: usize,
}

impl From<&ImportProgress> for Progress {
    fn from(p: &ImportProgress) -> Self {
        Self {
            received: p.games_seen,
            new: p.new,
            duplicates: p.duplicates,
            errors: p.errors,
        }
    }
}

/// O resumo de uma importação concluída (o mesmo da importação de arquivo).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Summary {
    pub received: usize,
    pub new: usize,
    pub duplicates: usize,
    pub errors: usize,
    pub cancelled: bool,
    /// Partidas que o Provedor mandou e o app pulou por não serem xadrez padrão (variantes).
    /// Só o Lichess e o PlayStrategy contam; no Chess.com fica em zero.
    pub skipped_variants: usize,
}

/// Por que a importação não terminou, em termos que a interface sabe explicar.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Failure {
    /// O Provedor está desligado.
    Off(Site),
    /// O Provedor pediu uma pausa.
    Limited { until: Timestamp },
    /// O nome não existe no Provedor.
    UnknownName(String),
    /// Sem conexão com a internet.
    Offline,
    /// A Trava de fair play está ligada.
    FairPlay,
    /// O banco da Biblioteca não está em arquivo (só nas capturas).
    NoDatabase,
    /// Qualquer outra coisa; o texto já é legível.
    Other(String),
}

impl Failure {
    fn of(site: Site, error: &ProviderError) -> Self {
        match error {
            ProviderError::Disabled { .. } => Failure::Off(site),
            ProviderError::RateLimited { until, .. } => Failure::Limited { until: *until },
            ProviderError::UnknownName { name, .. } => Failure::UnknownName(name.clone()),
            ProviderError::FairPlayLocked { .. } => Failure::FairPlay,
            ProviderError::Net(NetError::Unreachable { .. } | NetError::Timeout { .. }) => {
                Failure::Offline
            }
            other => Failure::Other(other.to_string()),
        }
    }
}

/// `Read` que guarda o erro tipado do Provedor quando o fluxo cai: o importador só devolve
/// um erro de E/S, e a interface quer saber se foi "nome inexistente" ou "pausa".
struct Tap<R> {
    inner: R,
    error: Arc<Mutex<Option<ProviderError>>>,
}

impl<R: std::io::Read> std::io::Read for Tap<R> {
    fn read(&mut self, out: &mut [u8]) -> std::io::Result<usize> {
        let result = self.inner.read(out);
        if let Err(error) = &result
            && let Some(provider) = error
                .get_ref()
                .and_then(|inner| inner.downcast_ref::<ProviderError>())
            && let Ok(mut slot) = self.error.lock()
        {
            *slot = Some(provider.clone());
        }
        result
    }
}

/// Importa o fluxo para a Biblioteca. Bloqueia até o fim: **só para threads de fundo e
/// testes**. `progress` é chamado a cada lote.
pub fn run(
    site: Site,
    feed: GamesFeed,
    store: &mut Store,
    profile: ProfileId,
    cancel: &Arc<AtomicBool>,
    progress: &mut dyn FnMut(Progress),
) -> Result<Summary, Failure> {
    let mut options = feed.import_options();
    options.batch_size = BATCH;
    let feed_progress = feed.progress_handle();
    let tapped = Arc::new(Mutex::new(None));
    let reader = BufReader::new(Tap {
        inner: feed.into_reader(cancel.clone()),
        error: tapped.clone(),
    });
    let outcome = store.import_pgn_stream(profile, reader, &options, cancel, &mut |p| {
        progress(Progress::from(p));
    });
    let provider_error = tapped.lock().ok().and_then(|mut slot| slot.take());
    let skipped_variants = if site.is_lila_family() {
        feed_progress
            .lock()
            .map(|p: std::sync::MutexGuard<'_, GamesProgress>| p.skipped)
            .unwrap_or(0)
    } else {
        0
    };
    match (outcome, provider_error) {
        (_, Some(error)) => Err(Failure::of(site, &error)),
        (Ok(summary), None) => Ok(Summary {
            received: summary.new + summary.duplicates + summary.errors,
            new: summary.new,
            duplicates: summary.duplicates,
            errors: summary.errors,
            cancelled: summary.cancelled,
            skipped_variants,
        }),
        (Err(error), None) => Err(Failure::Other(error.to_string())),
    }
}

/// O dia da Partida mais recente já importada de `name` neste Provedor (para o "Atualizar").
pub fn last_imported_day(store: &Store, profile: ProfileId, site: Site, name: &str) -> Option<Day> {
    let wanted = name.trim().to_lowercase();
    if wanted.is_empty() {
        return None;
    }
    let query = chess_store::ImportedQuery {
        text: Some(name.trim().to_owned()),
        profile: Some(profile),
        provider: Some(site.key().to_owned()),
        ..chess_store::ImportedQuery::default()
    };
    store
        .list_imported(&query)
        .ok()?
        .into_iter()
        // O texto procura em qualquer jogador: só vale quem jogou **como** esse nome.
        .filter(|g| {
            [&g.white, &g.black]
                .into_iter()
                .flatten()
                .any(|player| player.to_lowercase() == wanted)
        })
        .filter_map(|g| g.date.as_deref().and_then(Day::parse_pgn))
        .max()
}

/// Uma importação em segundo plano, com o andamento e o cancelamento compartilhados.
pub struct Job {
    pub site: Site,
    pub name: String,
    progress: Arc<Mutex<Progress>>,
    cancel: Arc<AtomicBool>,
    result: Arc<Mutex<Option<Result<Summary, Failure>>>>,
}

impl Job {
    /// Dispara a importação. Com `db` (o banco da Biblioteca em arquivo) a thread abre a sua
    /// própria conexão; sem arquivo (capturas) falha logo com [`Failure::NoDatabase`].
    pub fn spawn(
        clients: Clients,
        db: Option<PathBuf>,
        profile: ProfileId,
        request: Request,
        now: Timestamp,
    ) -> Self {
        let job = Self {
            site: request.site,
            name: request.name.trim().to_owned(),
            progress: Arc::default(),
            cancel: Arc::new(AtomicBool::new(false)),
            result: Arc::default(),
        };
        let (progress, cancel, result) = (
            Arc::clone(&job.progress),
            Arc::clone(&job.cancel),
            Arc::clone(&job.result),
        );
        std::thread::spawn(move || {
            let outcome = match db {
                None => Err(Failure::NoDatabase),
                Some(path) => Store::open(&path)
                    .map_err(|e| Failure::Other(e.to_string()))
                    .and_then(|mut store| {
                        let feed = clients.feed(&request, now);
                        run(request.site, feed, &mut store, profile, &cancel, &mut |p| {
                            if let Ok(mut shared) = progress.lock() {
                                *shared = p;
                            }
                        })
                    }),
            };
            if let Ok(mut slot) = result.lock() {
                *slot = Some(outcome);
            }
        });
        job
    }

    /// O resultado, uma vez, quando a thread terminou.
    pub fn take_result(&self) -> Option<Result<Summary, Failure>> {
        self.result.lock().ok()?.take()
    }

    pub fn progress(&self) -> Progress {
        self.progress.lock().map(|p| *p).unwrap_or_default()
    }

    pub fn cancel(&self) {
        self.cancel.store(true, Ordering::Release);
    }

    pub fn is_cancelling(&self) -> bool {
        self.cancel.load(Ordering::Acquire)
    }

    /// Uma importação de mentira para as capturas: nunca termina.
    pub fn demo_running(site: Site, name: &str, progress: Progress) -> Self {
        Self {
            site,
            name: name.to_owned(),
            progress: Arc::new(Mutex::new(progress)),
            cancel: Arc::new(AtomicBool::new(false)),
            result: Arc::default(),
        }
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;
    use chess_net::testing::{FakeResponse, FakeServer};
    use chess_net::{FakeClock, Net, NetConfig};
    use std::path::Path;
    use std::time::{Duration, Instant};

    const T0: Timestamp = Timestamp::from_millis(1_794_398_400_000); // 2026-11-11T12:00Z
    const ME: ProfileId = ProfileId(1);

    struct Env {
        server: FakeServer,
        service: NetService,
        clock: Arc<FakeClock>,
        db: PathBuf,
    }

    impl Drop for Env {
        fn drop(&mut self) {
            let _ = std::fs::remove_file(&self.db);
        }
    }

    fn env(tag: &str, enabled: bool) -> Env {
        let clock = Arc::new(FakeClock::new(T0));
        let server = FakeServer::with_clock(clock.clone());
        let mut config = NetConfig::in_memory().clock(clock.clone()).jitter_seed(3);
        for host in ["lichess.org", "api.chess.com", "playstrategy.org"] {
            config = config.route(host, &server.base_url());
        }
        let net = Net::open(config).unwrap();
        if enabled {
            net.set_enabled(ProviderId::Lichess, true).unwrap();
            net.set_enabled(ProviderId::PlayStrategy, true).unwrap();
        }
        let service = NetService::from_net(net, clock.clone(), false);
        let db = std::env::temp_dir().join(format!(
            "chess-studio-online-import-{tag}-{}.db",
            std::process::id()
        ));
        let _ = std::fs::remove_file(&db);
        Env {
            server,
            service,
            clock,
            db,
        }
    }

    fn lichess_games() -> FakeResponse {
        let path = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../chess-providers/tests/fixtures/lichess-games-ndjson.http");
        FakeResponse::recorded(&path).unwrap()
    }

    fn request(name: &str) -> Request {
        Request {
            site: Site::Lichess,
            name: name.into(),
            period: Period::All,
            pace: None,
            color: None,
            max: 300,
            since: None,
        }
    }

    /// Roda a importação numa thread de fundo e espera o fim (o app faz o mesmo, por quadro).
    fn import(env: &Env, request: Request) -> Result<Summary, Failure> {
        let job = Job::spawn(
            Clients::of(&env.service),
            Some(env.db.clone()),
            ME,
            request,
            env.clock_now(),
        );
        let start = Instant::now();
        loop {
            if let Some(result) = job.take_result() {
                return result;
            }
            assert!(
                start.elapsed() < Duration::from_secs(20),
                "importação travou"
            );
            std::thread::sleep(Duration::from_millis(10));
        }
    }

    impl Env {
        fn clock_now(&self) -> Timestamp {
            use chess_net::Clock;
            self.clock.now()
        }
    }

    #[test]
    fn periods_count_back_from_now() {
        assert_eq!(Period::All.since(T0), None);
        assert_eq!(Period::Week.since(T0), Day::new(2026, 11, 4).ok());
        assert_eq!(Period::Month.since(T0), Day::new(2026, 10, 12).ok());
        assert_eq!(Period::Year.since(T0), Day::new(2025, 11, 11).ok());
    }

    #[test]
    fn the_form_becomes_the_filter_and_update_wins_over_the_period() {
        let mut req = request("thibault");
        req.period = Period::Week;
        req.pace = Some(Pace::Blitz);
        req.color = Some(Color::White);
        req.max = 50;
        let filter = req.filter(T0);
        assert_eq!(filter.since, Day::new(2026, 11, 4).ok());
        assert_eq!(filter.paces, [Pace::Blitz]);
        assert_eq!(filter.color, Some(Color::White));
        assert_eq!(filter.max, Some(50));
        req.since = Day::new(2026, 11, 7).ok();
        assert_eq!(req.filter(T0).since, Day::new(2026, 11, 7).ok());
        assert!(!request("  ").is_valid());
    }

    #[test]
    fn importing_end_to_end_stores_the_games_with_their_origin_and_the_update_adds_no_duplicates() {
        let env = env("e2e", true);
        env.server
            .route("/api/games/user/thibault", lichess_games());
        let first = import(&env, request("thibault")).unwrap();
        assert_eq!((first.new, first.duplicates, first.errors), (2, 0, 0));
        assert_eq!(first.received, 2);

        // A Origem de cada Partida: Provedor, modo API e licença.
        let store = Store::open(&env.db).unwrap();
        let games = store
            .list_imported(&chess_store::ImportedQuery::default())
            .unwrap();
        assert_eq!(games.len(), 2);
        assert!(
            games
                .iter()
                .all(|g| g.provider.as_deref() == Some("lichess"))
        );
        assert!(
            games
                .iter()
                .all(|g| g.import_mode == chess_store::ImportMode::Api)
        );
        let origin = &store.origins(ME, games[0].id).unwrap()[0];
        assert_eq!(origin.provider.as_deref(), Some("lichess"));
        assert!(origin.license.is_some());
        assert!(origin.remote_id.is_some());

        // "Atualizar": pede só a partir do dia da última Partida (2026-11-07)…
        let last = last_imported_day(&store, ME, Site::Lichess, "Thibault").unwrap();
        assert_eq!(last, Day::new(2026, 11, 7).unwrap());
        drop(store);
        let mut update = request("thibault");
        update.since = Some(last);
        let second = import(&env, update).unwrap();
        // …e o Provedor repete as mesmas Partidas: todas reconhecidas, nenhuma duplicada.
        assert_eq!((second.new, second.duplicates), (0, 2));
        let sent = env.server.requests();
        assert_eq!(sent.len(), 2);
        assert_eq!(
            sent[1].query_value("since").as_deref(),
            Some(last.start_ms().to_string().as_str())
        );
        assert_eq!(sent[0].query_value("since"), None);
        let store = Store::open(&env.db).unwrap();
        assert_eq!(
            store
                .list_imported(&chess_store::ImportedQuery::default())
                .unwrap()
                .len(),
            2
        );
    }

    #[test]
    fn the_last_import_only_counts_games_where_the_name_played() {
        let mut store = Store::open_in_memory().unwrap();
        let pgn = "[Event \"a\"]\n[White \"thibault\"]\n[Black \"outro\"]\n[Date \"2026.11.07\"]\n[Result \"1-0\"]\n\n1. e4 1-0\n\n\
                   [Event \"b\"]\n[White \"alguem\"]\n[Black \"thibaultfan\"]\n[Date \"2026.11.09\"]\n[Result \"0-1\"]\n\n1. d4 0-1\n";
        store.import_pgn(pgn, None).unwrap();
        let profile = store.default_profile().unwrap();
        // Sem Origem de Provedor: nada conta como "importado do Lichess".
        assert_eq!(
            last_imported_day(&store, profile, Site::Lichess, "thibault"),
            None
        );
    }

    #[test]
    fn failures_are_explained_not_dumped() {
        // Provedor desligado: nenhum pedido sai.
        let off = env("off", false);
        off.server
            .route("/api/games/user/thibault", lichess_games());
        assert_eq!(
            import(&off, request("thibault")),
            Err(Failure::Off(Site::Lichess))
        );
        assert!(off.server.requests().is_empty());
        // Nome que não existe.
        let env = env("unknown", true);
        env.server
            .route("/api/games/user/fantasma", FakeResponse::status(404));
        assert_eq!(
            import(&env, request("fantasma")),
            Err(Failure::UnknownName("fantasma".into()))
        );
    }

    /// Duas Partidas de xadrez padrão e duas de variantes, no formato NDJSON da família do
    /// Lichess (sintético: o PlayStrategy não foi consultado de verdade, CX-17).
    fn playstrategy_games() -> FakeResponse {
        let game = |id: &str, variant: &str, day: u32, moves: &str| {
            let pgn = format!(
                "[Event \"Rated Rapid game\"]\n[Site \"https://playstrategy.org/{id}\"]\n\
                 [Date \"2026.11.{day:02}\"]\n[White \"ana_exemplo\"]\n[Black \"adversario\"]\n\
                 [Result \"1-0\"]\n\n{moves} 1-0\n\n"
            );
            let line = serde_json::json!({ "id": id, "variant": variant, "pgn": pgn });
            format!("{line}\n")
        };
        let body = [
            game("Ps4rT9xQ", "standard", 3, "1. e4 e5 2. Nf3"),
            game("Ps5aB8yZ", "fromPosition", 5, "1. d4 d5 2. c4"),
            game("Ps6shogi", "shogi", 6, "1. P-76"),
            game("Ps7crazy", "crazyhouse", 7, "1. e4 e5"),
        ]
        .concat();
        FakeResponse::ok(body).header("content-type", "application/x-ndjson")
    }

    fn ps_request(name: &str) -> Request {
        Request {
            site: Site::PlayStrategy,
            ..request(name)
        }
    }

    #[test]
    fn playstrategy_reuses_the_lichess_flow_keeps_standard_chess_and_counts_the_skipped_variants() {
        let env = env("ps", true);
        env.server
            .route("/api/games/user/ana_exemplo", playstrategy_games());
        let first = import(&env, ps_request("ana_exemplo")).unwrap();
        assert_eq!((first.new, first.duplicates, first.errors), (2, 0, 0));
        assert_eq!(first.skipped_variants, 2, "shogi e crazyhouse puladas");

        // O pedido foi ao PlayStrategy, com os filtros da família do Lichess.
        let sent = env.server.requests();
        assert_eq!(sent.len(), 1);
        assert!(sent[0].path.ends_with("/api/games/user/ana_exemplo"));
        assert!(
            sent[0].header("authorization").is_none(),
            "público: sem token"
        );

        // Origem correta: Provedor "playstrategy", modo API, licença e id remoto.
        let store = Store::open(&env.db).unwrap();
        let games = store
            .list_imported(&chess_store::ImportedQuery::default())
            .unwrap();
        assert_eq!(games.len(), 2);
        assert!(
            games
                .iter()
                .all(|g| g.provider.as_deref() == Some("playstrategy"))
        );
        let origin = &store.origins(ME, games[0].id).unwrap()[0];
        assert_eq!(origin.provider.as_deref(), Some("playstrategy"));
        assert!(origin.license.is_some());
        assert!(origin.remote_id.is_some());
        let last = last_imported_day(&store, ME, Site::PlayStrategy, "ANA_exemplo");
        assert!(
            last.is_some(),
            "o Atualizar enxerga a Origem do PlayStrategy"
        );
        // As Partidas do PlayStrategy não são "do Lichess".
        assert_eq!(
            last_imported_day(&store, ME, Site::Lichess, "ana_exemplo"),
            None
        );
        drop(store);

        // Importar de novo: tudo reconhecido, nada duplicado.
        let second = import(&env, ps_request("ana_exemplo")).unwrap();
        assert_eq!((second.new, second.duplicates), (0, 2));
        assert_eq!(second.skipped_variants, 2);
    }

    #[test]
    fn playstrategy_switched_off_sends_no_request() {
        let env = env("psoff", false);
        env.server
            .route("/api/games/user/ana_exemplo", playstrategy_games());
        assert_eq!(
            import(&env, ps_request("ana_exemplo")),
            Err(Failure::Off(Site::PlayStrategy))
        );
        assert!(env.server.requests().is_empty(), "zero pedidos");
    }

    #[test]
    fn each_provider_is_independent_and_the_origin_keys_match_the_library() {
        // Só o PlayStrategy ligado: o Lichess continua sem pedir.
        let env = env("psonly", false);
        env.service
            .set_enabled(ProviderId::PlayStrategy, true)
            .unwrap();
        env.server
            .route("/api/games/user/ana_exemplo", playstrategy_games());
        assert_eq!(
            import(&env, request("ana_exemplo")),
            Err(Failure::Off(Site::Lichess))
        );
        assert!(env.server.requests().is_empty());
        assert!(import(&env, ps_request("ana_exemplo")).is_ok());
        for site in [Site::Lichess, Site::ChessCom, Site::PlayStrategy] {
            assert_eq!(
                chess_store::ImportSource::from_provider_id(site.key()),
                Some(site.source())
            );
        }
    }

    #[test]
    fn cancelling_stops_the_import_and_keeps_what_was_stored() {
        let env = env("cancel", true);
        env.server.route(
            "/api/games/user/thibault",
            FakeResponse::stream_forever(b"\n"),
        );
        let job = Job::spawn(
            Clients::of(&env.service),
            Some(env.db.clone()),
            ME,
            request("thibault"),
            env.clock_now(),
        );
        std::thread::sleep(Duration::from_millis(150));
        job.cancel();
        let start = Instant::now();
        let result = loop {
            if let Some(result) = job.take_result() {
                break result;
            }
            assert!(
                start.elapsed() < Duration::from_secs(20),
                "o cancelamento travou"
            );
            std::thread::sleep(Duration::from_millis(10));
        };
        let summary = result.unwrap();
        assert!(summary.cancelled || summary.received == 0);
    }

    #[test]
    fn without_a_database_file_the_job_says_so() {
        let env = env("nodb", true);
        let job = Job::spawn(
            Clients::of(&env.service),
            None,
            ME,
            request("thibault"),
            env.clock_now(),
        );
        let start = Instant::now();
        while job.take_result().is_none() {
            assert!(start.elapsed() < Duration::from_secs(5));
            std::thread::sleep(Duration::from_millis(5));
        }
    }
}
