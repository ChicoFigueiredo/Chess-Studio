//! `Net`: o ponto único por onde toda chamada a Provedor passa (gate), as threads por
//! Provedor, o cache com Origem e o estado da rede.
//!
//! Modelo de threads (como o gerenciador de Engines do app): cada Provedor tem uma thread
//! própria com fila, criada no primeiro pedido; os pedidos de um Provedor saem um de cada vez
//! (Lichess, PlayStrategy e Chess.com pedem acesso serial). O resultado volta por canal e a
//! interface consulta [`Pending::try_take`] a cada quadro, sem bloquear. Fluxos NDJSON têm
//! thread própria, para não ocupar a fila.

use crate::LocalProfileId;
use crate::catalog::{Capability, Endpoint, ProviderId, RateLimit, catalog};
use crate::clock::{Clock, SystemClock, Timestamp};
use crate::error::NetError;
use crate::guard::FairPlayGuard;
use crate::limits::{DEFAULT_RETRY_AFTER, RetryPolicy, SplitMix64, TokenBucket, retry_after};
use crate::ndjson::{NdjsonError, NdjsonItem, NdjsonReader};
use crate::request::{Method, Request, Target, sanitized};
use crate::response::{Fetched, Freshness, ImportMode, Provenance};
use crate::secret::{Authorization, AuthorizationSource, NoVault};
use crate::status::{NetworkStatus, Operation, ProviderState};
use crate::store::{CacheEntry, NetStore};
use crate::transport::{Failure, Http, Outgoing, ResponseHead};
use std::collections::hash_map::RandomState;
use std::collections::{HashMap, HashSet, VecDeque};
use std::fmt;
use std::hash::{BuildHasher, Hasher};
use std::io::ErrorKind;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::mpsc::{self, Receiver, RecvTimeoutError, Sender, TryRecvError};
use std::sync::{Arc, Mutex, MutexGuard, PoisonError, RwLock};
use std::thread;
use std::time::Duration;

const DEFAULT_TIMEOUT: Duration = Duration::from_secs(20);
const DEFAULT_CONNECT_TIMEOUT: Duration = Duration::from_secs(10);
const DEFAULT_STREAM_READ_TIMEOUT: Duration = Duration::from_secs(30);
const DEFAULT_MAX_BODY: usize = 16 * 1024 * 1024;
const DEFAULT_MAX_LINE: usize = 1024 * 1024;
/// Quantas pausas (`429`) seguidas um pedido paciente aceita antes de desistir.
const MAX_RATE_LIMIT_HITS: u32 = 3;
const OPERATION_LOG: usize = 100;

#[derive(Debug, Clone)]
enum DbLocation {
    Memory,
    Path(PathBuf),
}

/// Configuração da camada de rede. Os padrões servem ao app; os testes trocam o relógio, as
/// rotas (servidor falso) e os limites.
#[derive(Debug, Clone)]
pub struct NetConfig {
    db: DbLocation,
    clock: Arc<dyn Clock>,
    routes: Vec<(String, String)>,
    rate_limits: HashMap<ProviderId, RateLimit>,
    retry: RetryPolicy,
    vault: Arc<dyn AuthorizationSource>,
    guard: FairPlayGuard,
    profile: LocalProfileId,
    timeout: Duration,
    connect_timeout: Duration,
    stream_read_timeout: Duration,
    max_body: usize,
    max_line: usize,
    jitter_seed: Option<u64>,
}

impl NetConfig {
    /// Banco da rede no arquivo `path` (ao lado do banco das Partidas, no diretório de dados).
    pub fn at_path(path: impl AsRef<Path>) -> Self {
        Self::with_db(DbLocation::Path(path.as_ref().to_path_buf()))
    }

    /// Banco em memória (testes; nada persiste).
    pub fn in_memory() -> Self {
        Self::with_db(DbLocation::Memory)
    }

    fn with_db(db: DbLocation) -> Self {
        Self {
            db,
            clock: Arc::new(SystemClock),
            routes: Vec::new(),
            rate_limits: HashMap::new(),
            retry: RetryPolicy::default(),
            vault: Arc::new(NoVault),
            guard: FairPlayGuard::new(),
            profile: LocalProfileId(1),
            timeout: DEFAULT_TIMEOUT,
            connect_timeout: DEFAULT_CONNECT_TIMEOUT,
            stream_read_timeout: DEFAULT_STREAM_READ_TIMEOUT,
            max_body: DEFAULT_MAX_BODY,
            max_line: DEFAULT_MAX_LINE,
            jitter_seed: None,
        }
    }

    pub fn clock(mut self, clock: Arc<dyn Clock>) -> Self {
        self.clock = clock;
        self
    }

    /// Manda o host oficial `host` para `base_url` (servidor falso, espelho local). Só HTTPS
    /// ou loopback (`http://127.0.0.1`, `http://localhost`); a Origem e o cache continuam
    /// usando o endereço oficial.
    pub fn route(mut self, host: &str, base_url: &str) -> Self {
        self.routes.push((
            host.to_ascii_lowercase(),
            base_url.trim_end_matches('/').to_owned(),
        ));
        self
    }

    /// Troca o token bucket do Provedor (o padrão vem do catálogo).
    pub fn rate_limit(mut self, provider: ProviderId, limit: RateLimit) -> Self {
        self.rate_limits.insert(provider, limit);
        self
    }

    pub fn retry(mut self, policy: RetryPolicy) -> Self {
        self.retry = policy;
        self
    }

    /// De onde vem o `Authorization` das Conexões (o Cofre, etapa 3).
    pub fn vault(mut self, vault: Arc<dyn AuthorizationSource>) -> Self {
        self.vault = vault;
        self
    }

    /// Trava compartilhada com quem conduz a Partida online (etapa 4).
    pub fn fair_play_guard(mut self, guard: FairPlayGuard) -> Self {
        self.guard = guard;
        self
    }

    /// Perfil local ativo ao abrir (padrão: 1, o primeiro do `chess-store`).
    pub fn profile(mut self, profile: LocalProfileId) -> Self {
        self.profile = profile;
        self
    }

    /// Prazo padrão de cada pedido.
    pub fn timeout(mut self, timeout: Duration) -> Self {
        self.timeout = timeout;
        self
    }

    /// Limite padrão de bytes de uma resposta.
    pub fn max_body(mut self, bytes: usize) -> Self {
        self.max_body = bytes;
        self
    }

    /// Linha máxima dos fluxos NDJSON.
    pub fn max_line(mut self, bytes: usize) -> Self {
        self.max_line = bytes;
        self
    }

    /// Semente do jitter do backoff (testes reprodutíveis; o padrão é aleatório).
    pub fn jitter_seed(mut self, seed: u64) -> Self {
        self.jitter_seed = Some(seed);
        self
    }
}

fn valid_route(base: &str) -> bool {
    let loopback = ["http://127.0.0.1", "http://localhost", "http://[::1]"]
        .iter()
        .any(|prefix| {
            base.strip_prefix(prefix)
                .is_some_and(|rest| rest.is_empty() || rest.starts_with(':'))
        });
    loopback || base.starts_with("https://")
}

fn lock<T>(mutex: &Mutex<T>) -> MutexGuard<'_, T> {
    mutex.lock().unwrap_or_else(PoisonError::into_inner)
}

/// Estado de execução de um Provedor (vale para todos os Perfis locais: o limite é do
/// servidor, não do perfil).
#[derive(Debug)]
struct Runtime {
    bucket: TokenBucket,
    limited_until: Option<Timestamp>,
    failing: Option<(Timestamp, NetError)>,
}

/// Pedido admitido pelo gate.
#[derive(Debug)]
struct Admitted {
    target: Target,
    endpoint: &'static Endpoint,
    authorization: Option<Authorization>,
}

struct Job {
    request: Request,
    profile: LocalProfileId,
    cancel: Arc<AtomicBool>,
    reply: Sender<Result<Fetched, NetError>>,
}

struct Shared {
    clock: Arc<dyn Clock>,
    http: Http,
    store: Mutex<NetStore>,
    consents: RwLock<HashSet<(LocalProfileId, ProviderId)>>,
    active: RwLock<LocalProfileId>,
    runtime: Mutex<HashMap<ProviderId, Runtime>>,
    routes: HashMap<String, String>,
    rate_limits: HashMap<ProviderId, RateLimit>,
    retry: RetryPolicy,
    vault: Arc<dyn AuthorizationSource>,
    guard: FairPlayGuard,
    timeout: Duration,
    stream_read_timeout: Duration,
    max_body: usize,
    max_line: usize,
    rng: Mutex<SplitMix64>,
    operations: Mutex<VecDeque<Operation>>,
    active_requests: AtomicUsize,
}

struct Handle {
    shared: Arc<Shared>,
    lanes: Mutex<HashMap<ProviderId, Sender<Job>>>,
}

/// A camada de rede. Barata de clonar: os clones compartilham filas, limites e cache.
#[derive(Clone)]
pub struct Net {
    handle: Arc<Handle>,
}

impl fmt::Debug for Net {
    /// Só contagens: nada do Cofre, das rotas ou do cache.
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let shared = &self.handle.shared;
        f.debug_struct("Net")
            .field("active_profile", &self.active_profile())
            .field("enabled", &self.status().enabled_count())
            .field(
                "active_requests",
                &shared.active_requests.load(Ordering::Relaxed),
            )
            .finish_non_exhaustive()
    }
}

impl Net {
    pub fn open(config: NetConfig) -> Result<Self, NetError> {
        let store = match &config.db {
            DbLocation::Memory => NetStore::open_in_memory()?,
            DbLocation::Path(path) => NetStore::open(path)?,
        };
        let mut routes = HashMap::new();
        for (host, base) in &config.routes {
            if !valid_route(base) {
                return Err(NetError::invalid(format!(
                    "rota para {host} precisa ser HTTPS ou loopback"
                )));
            }
            routes.insert(host.clone(), base.clone());
        }
        let consents = store.enabled_consents()?.into_iter().collect();
        let seed = config.jitter_seed.unwrap_or_else(|| {
            let mut hasher = RandomState::new().build_hasher();
            hasher.write_u64(0x5eed);
            hasher.finish()
        });
        let shared = Shared {
            clock: config.clock,
            http: Http::new(config.connect_timeout),
            store: Mutex::new(store),
            consents: RwLock::new(consents),
            active: RwLock::new(config.profile),
            runtime: Mutex::new(HashMap::new()),
            routes,
            rate_limits: config.rate_limits,
            retry: config.retry,
            vault: config.vault,
            guard: config.guard,
            timeout: config.timeout,
            stream_read_timeout: config.stream_read_timeout,
            max_body: config.max_body,
            max_line: config.max_line,
            rng: Mutex::new(SplitMix64::new(seed)),
            operations: Mutex::new(VecDeque::new()),
            active_requests: AtomicUsize::new(0),
        };
        Ok(Self {
            handle: Arc::new(Handle {
                shared: Arc::new(shared),
                lanes: Mutex::new(HashMap::new()),
            }),
        })
    }

    // ---------------- Perfil local e consentimento ----------------

    pub fn active_profile(&self) -> LocalProfileId {
        *self
            .handle
            .shared
            .active
            .read()
            .unwrap_or_else(PoisonError::into_inner)
    }

    /// Troca o Perfil local: consentimento, cache e pedidos seguintes passam a ser dele.
    pub fn set_active_profile(&self, profile: LocalProfileId) {
        *self
            .handle
            .shared
            .active
            .write()
            .unwrap_or_else(PoisonError::into_inner) = profile;
    }

    pub fn is_enabled(&self, provider: ProviderId) -> bool {
        self.handle
            .shared
            .is_enabled(self.active_profile(), provider)
    }

    /// Liga ou desliga o Provedor para o Perfil local ativo (gravado). Ligar só depois de
    /// mostrar o aviso [`crate::Provider::sends`]. Provedor sem serviço de rede não liga.
    pub fn set_enabled(&self, provider: ProviderId, enabled: bool) -> Result<(), NetError> {
        if enabled && !provider.info().uses_network() {
            return Err(NetError::NoNetworkService { provider });
        }
        let shared = &self.handle.shared;
        let profile = self.active_profile();
        lock(&shared.store).set_consent(profile, provider, enabled, shared.clock.now())?;
        let mut consents = shared
            .consents
            .write()
            .unwrap_or_else(PoisonError::into_inner);
        if enabled {
            consents.insert((profile, provider));
        } else {
            consents.remove(&(profile, provider));
        }
        Ok(())
    }

    /// O Perfil local foi excluído: apaga o consentimento e o cache dele.
    pub fn forget_profile(&self, profile: LocalProfileId) -> Result<(), NetError> {
        let shared = &self.handle.shared;
        lock(&shared.store).forget_profile(profile)?;
        shared
            .consents
            .write()
            .unwrap_or_else(PoisonError::into_inner)
            .retain(|(p, _)| *p != profile);
        Ok(())
    }

    pub fn fair_play_guard(&self) -> &FairPlayGuard {
        &self.handle.shared.guard
    }

    // ---------------- Pedidos ----------------

    /// Envia o pedido pela fila do Provedor. Nunca bloqueia: um pedido recusado pelo gate
    /// (Provedor desligado, endereço fora do catálogo, Trava de fair play, sem autorização)
    /// já volta resolvido, sem thread e sem rede.
    pub fn fetch(&self, request: Request) -> Pending {
        let shared = &self.handle.shared;
        let profile = self.active_profile();
        let started = shared.clock.now();
        if let Err(err) = shared.admit(&request, profile) {
            shared.record(&request, None, started, &Err(err.clone()));
            return Pending::ready(Err(err));
        }
        let (reply, rx) = mpsc::channel();
        let cancel = Arc::new(AtomicBool::new(false));
        let pending = Pending {
            rx,
            wake: reply.clone(),
            cancel: cancel.clone(),
            done: false,
        };
        shared.active_requests.fetch_add(1, Ordering::AcqRel);
        let provider = request.provider;
        let job = Job {
            request,
            profile,
            cancel,
            reply,
        };
        if self.lane(provider).send(job).is_err() {
            shared.active_requests.fetch_sub(1, Ordering::AcqRel);
            return Pending::ready(Err(NetError::Unreachable {
                provider,
                detail: "fila do Provedor indisponível".to_owned(),
            }));
        }
        pending
    }

    /// Abre um fluxo NDJSON (eventos, exportação de Partidas) numa thread própria, pelo mesmo
    /// gate. Sem cache e sem repetição: reconectar é decisão do adaptador.
    pub fn stream(&self, request: Request) -> Stream {
        let shared = &self.handle.shared;
        let profile = self.active_profile();
        let (tx, rx) = mpsc::channel();
        let cancel = Arc::new(AtomicBool::new(false));
        let stream = Stream {
            rx,
            cancel: cancel.clone(),
        };
        let started = shared.clock.now();
        if let Err(err) = shared.admit(&request, profile) {
            shared.record(&request, None, started, &Err(err.clone()));
            let _ = tx.send(StreamEvent::Failed(err));
            return stream;
        }
        shared.active_requests.fetch_add(1, Ordering::AcqRel);
        let worker = shared.clone();
        let name = format!("chess-net-fluxo-{}", request.provider.key());
        let spawned = thread::Builder::new().name(name).spawn(move || {
            worker.run_stream(&request, profile, &cancel, &tx);
            worker.active_requests.fetch_sub(1, Ordering::AcqRel);
        });
        if spawned.is_err() {
            shared.active_requests.fetch_sub(1, Ordering::AcqRel);
        }
        stream
    }

    /// Lê só o cache local do pedido, sem rede e sem consultar o consentimento (leitura
    /// offline do que já está no disco). A idade diz quão antigo é o conteúdo.
    pub fn read_cache(&self, request: &Request) -> Result<Option<Fetched>, NetError> {
        let shared = &self.handle.shared;
        let target = request.target()?;
        let key = request.cache_key(&target);
        let entry = lock(&shared.store).get(self.active_profile(), &key)?;
        Ok(entry.map(|e| from_entry(&e, shared.clock.now(), Freshness::Cache)))
    }

    /// Apaga o cache do Provedor no Perfil local ativo; devolve quantas entradas saíram.
    pub fn clear_cache(&self, provider: ProviderId) -> Result<usize, NetError> {
        lock(&self.handle.shared.store).clear_provider(self.active_profile(), provider)
    }

    // ---------------- Estado ----------------

    /// Estado da rede para a barra de status (barato; pode ser lido a cada quadro).
    pub fn status(&self) -> NetworkStatus {
        let shared = &self.handle.shared;
        let profile = self.active_profile();
        let now = shared.clock.now();
        let runtime = lock(&shared.runtime);
        let providers = catalog()
            .iter()
            .map(|p| {
                let state = if !shared.is_enabled(profile, p.id) {
                    ProviderState::Disabled
                } else {
                    match runtime.get(&p.id) {
                        Some(Runtime {
                            limited_until: Some(until),
                            ..
                        }) if *until > now => ProviderState::Limited { until: *until },
                        Some(Runtime {
                            failing: Some((since, error)),
                            ..
                        }) => ProviderState::Failing {
                            since: *since,
                            error: error.clone(),
                        },
                        _ => ProviderState::Available,
                    }
                };
                (p.id, state)
            })
            .collect();
        NetworkStatus {
            providers,
            active_requests: shared.active_requests.load(Ordering::Acquire),
        }
    }

    /// Últimas operações (sanitizadas: sem cabeçalho, consulta, corpo ou segredo), da mais
    /// antiga à mais nova.
    pub fn recent_operations(&self) -> Vec<Operation> {
        lock(&self.handle.shared.operations)
            .iter()
            .cloned()
            .collect()
    }

    fn lane(&self, provider: ProviderId) -> Sender<Job> {
        let mut lanes = lock(&self.handle.lanes);
        lanes
            .entry(provider)
            .or_insert_with(|| {
                let (tx, rx) = mpsc::channel::<Job>();
                let shared = self.handle.shared.clone();
                let name = format!("chess-net-{}", provider.key());
                // Sem a thread, o `rx` cai e o `send` falha: o pedido volta com erro.
                let _ = thread::Builder::new()
                    .name(name)
                    .spawn(move || worker(&shared, &rx));
                tx
            })
            .clone()
    }
}

fn worker(shared: &Shared, jobs: &Receiver<Job>) {
    while let Ok(job) = jobs.recv() {
        if !job.cancel.load(Ordering::Acquire) {
            let started = shared.clock.now();
            let (result, endpoint) = shared.run(&job);
            shared.record(&job.request, endpoint, started, &result);
            if !job.cancel.load(Ordering::Acquire) {
                let _ = job.reply.send(result);
            }
        }
        shared.active_requests.fetch_sub(1, Ordering::AcqRel);
    }
}

fn mode_for(capability: Capability) -> ImportMode {
    if capability == Capability::Download {
        ImportMode::Download
    } else {
        ImportMode::Api
    }
}

fn from_entry(entry: &CacheEntry, now: Timestamp, freshness: Freshness) -> Fetched {
    Fetched {
        status: entry.status,
        content_type: entry.content_type.clone(),
        body: entry.body.clone(),
        provenance: Provenance {
            provider: entry.provider,
            queried_at: entry.queried_at,
            license: entry.license.clone(),
            mode: entry.mode,
            source: entry.source_url.clone(),
        },
        freshness,
        age: now.since(entry.queried_at),
    }
}

/// Como terminou um fluxo do lado de cá.
enum StreamEnd {
    Ended,
    Closed,
}

impl Shared {
    fn is_enabled(&self, profile: LocalProfileId, provider: ProviderId) -> bool {
        self.consents
            .read()
            .unwrap_or_else(PoisonError::into_inner)
            .contains(&(profile, provider))
    }

    /// O gate: consentimento, catálogo, Trava de fair play e autorização, nesta ordem. É
    /// chamado ao enviar e de novo logo antes de cada ida à rede.
    fn admit(&self, request: &Request, profile: LocalProfileId) -> Result<Admitted, NetError> {
        let provider = request.provider;
        if !self.is_enabled(profile, provider) {
            return Err(NetError::ProviderDisabled { provider });
        }
        let target = request.target()?;
        let info = provider.info();
        let endpoint = target
            .port
            .is_none()
            .then(|| info.endpoint_for(&target.host, &target.path))
            .flatten()
            .ok_or_else(|| NetError::EndpointNotAllowed {
                provider,
                url: sanitized(&request.url),
            })?;
        let capability = endpoint.capability;
        let service = info
            .service(capability)
            .filter(|s| s.status.allows_network())
            .ok_or(NetError::NotOffered {
                provider,
                capability,
            })?;
        self.guard.check(provider, capability)?;
        let authorization = request
            .connection
            .and_then(|c| self.vault.authorization(profile, provider, c))
            // Nunca entre hosts: o token de lichess.org não vai para explorer.lichess.org.
            .filter(|a| a.host() == target.host);
        if service.requires_connection() && authorization.is_none() {
            return Err(NetError::NotAuthorized {
                provider,
                capability,
            });
        }
        Ok(Admitted {
            target,
            endpoint,
            authorization,
        })
    }

    fn runtime<R>(&self, provider: ProviderId, f: impl FnOnce(&mut Runtime) -> R) -> R {
        let mut map = lock(&self.runtime);
        let limit = self
            .rate_limits
            .get(&provider)
            .copied()
            .unwrap_or(provider.info().rate_limit);
        let runtime = map.entry(provider).or_insert_with(|| Runtime {
            bucket: TokenBucket::new(limit),
            limited_until: None,
            failing: None,
        });
        f(runtime)
    }

    fn limited_until(&self, provider: ProviderId, now: Timestamp) -> Option<Timestamp> {
        self.runtime(provider, |r| r.limited_until.filter(|u| *u > now))
    }

    fn set_limited(&self, provider: ProviderId, until: Timestamp) {
        self.runtime(provider, |r| {
            r.limited_until = Some(r.limited_until.map_or(until, |u| u.max(until)));
        });
    }

    fn mark_ok(&self, provider: ProviderId) {
        self.runtime(provider, |r| r.failing = None);
    }

    fn mark_failing(&self, provider: ProviderId, now: Timestamp, error: &NetError) {
        self.runtime(provider, |r| {
            let since = r.failing.as_ref().map_or(now, |(s, _)| *s);
            r.failing = Some((since, error.clone()));
        });
    }

    /// Espera a vez no token bucket; `false` se cancelado.
    fn take_token(&self, provider: ProviderId, cancel: &AtomicBool) -> bool {
        let now = self.clock.now();
        let at = self.runtime(provider, |r| r.bucket.reserve(now));
        at <= now || self.clock.sleep_until(at, cancel)
    }

    fn record(
        &self,
        request: &Request,
        endpoint: Option<&'static Endpoint>,
        started: Timestamp,
        result: &Result<Fetched, NetError>,
    ) {
        let path = request.target().map_or_else(
            |_| {
                sanitized(&request.url)
                    .split_once("://")
                    .and_then(|(_, rest)| rest.find('/').map(|i| rest[i..].to_owned()))
                    .unwrap_or_default()
            },
            |t| t.path,
        );
        let operation = Operation {
            provider: request.provider,
            capability: endpoint.map(|e| e.capability),
            method: request.method,
            path,
            started_at: started,
            finished_at: self.clock.now(),
            http_status: match result {
                Ok(f) => Some(f.status),
                Err(e) => e.http_status(),
            },
            outcome: result
                .as_ref()
                .map(|f| f.freshness.clone())
                .map_err(Clone::clone),
        };
        let mut log = lock(&self.operations);
        if log.len() == OPERATION_LOG {
            log.pop_front();
        }
        log.push_back(operation);
    }

    fn routed_url(&self, request: &Request, target: &Target) -> String {
        let base = self
            .routes
            .get(&target.host)
            .cloned()
            .unwrap_or_else(|| format!("https://{}", target.authority()));
        let query = request.encoded_query();
        if query.is_empty() {
            format!("{base}{}", target.path)
        } else {
            format!("{base}{}?{query}", target.path)
        }
    }

    fn outgoing<'a>(
        &self,
        request: &'a Request,
        admitted: &'a Admitted,
        cached: Option<&CacheEntry>,
        cancel: &'a AtomicBool,
    ) -> Outgoing<'a> {
        let mut headers = request.headers.clone();
        if let Some(entry) = cached {
            if let Some(etag) = &entry.etag {
                headers.push(("if-none-match".to_owned(), etag.clone()));
            }
            if let Some(modified) = &entry.last_modified {
                headers.push(("if-modified-since".to_owned(), modified.clone()));
            }
        }
        Outgoing {
            method: request.method,
            url: self.routed_url(request, &admitted.target),
            headers,
            authorization: admitted
                .authorization
                .as_ref()
                .map(Authorization::header_value),
            body: request.body.as_ref(),
            timeout: request.timeout.unwrap_or(self.timeout),
            max_body: request.max_body.unwrap_or(self.max_body),
            cancel,
        }
    }

    /// Sem rede (ou limitado), devolve o cache vencido marcado `Stale`, se o pedido aceita.
    fn fallback(
        &self,
        request: &Request,
        cached: Option<&CacheEntry>,
        cause: NetError,
    ) -> Result<Fetched, NetError> {
        match cached {
            Some(entry) if request.allow_stale => Ok(from_entry(
                entry,
                self.clock.now(),
                Freshness::Stale { cause },
            )),
            _ => Err(cause),
        }
    }

    /// Executa um pedido admitido: cache, limites, rede, repetições e Origem.
    fn run(&self, job: &Job) -> (Result<Fetched, NetError>, Option<&'static Endpoint>) {
        let request = &job.request;
        let admitted = match self.admit(request, job.profile) {
            Ok(a) => a,
            Err(e) => return (Err(e), None),
        };
        let endpoint = admitted.endpoint;
        (self.run_admitted(job, admitted), Some(endpoint))
    }

    fn run_admitted(&self, job: &Job, mut admitted: Admitted) -> Result<Fetched, NetError> {
        let request = &job.request;
        let provider = request.provider;
        let cancel = &*job.cancel;
        let key = request.cache_key(&admitted.target);
        let source = request.canonical_url(&admitted.target);
        let cached = match request.cache_ttl {
            Some(_) => lock(&self.store).get(job.profile, &key)?,
            None => None,
        };
        if let Some(entry) = &cached
            && entry.is_fresh(self.clock.now())
        {
            return Ok(from_entry(entry, self.clock.now(), Freshness::Cache));
        }

        let mut retries = 0u32;
        let mut limited_hits = 0u32;
        loop {
            let now = self.clock.now();
            if let Some(until) = self.limited_until(provider, now) {
                if until.since(now) > request.patience || limited_hits >= MAX_RATE_LIMIT_HITS {
                    return self.fallback(
                        request,
                        cached.as_ref(),
                        NetError::RateLimited { provider, until },
                    );
                }
                if !self.clock.sleep_until(until, cancel) {
                    return Err(NetError::Cancelled);
                }
            }
            if !self.take_token(provider, cancel) {
                return Err(NetError::Cancelled);
            }
            // O consentimento, a trava ou a Conexão podem ter mudado durante a espera.
            admitted = self.admit(request, job.profile)?;
            let outgoing = self.outgoing(request, &admitted, cached.as_ref(), cancel);
            let outcome = self.http.send(&outgoing);
            let now = self.clock.now();
            let (head, body) = match outcome {
                Ok(response) => response,
                Err(Failure::Cancelled) => return Err(NetError::Cancelled),
                Err(Failure::BodyTooLarge) => {
                    return Err(NetError::BodyTooLarge {
                        provider,
                        limit: outgoing.max_body,
                    });
                }
                Err(Failure::Timeout) => {
                    let err = NetError::Timeout { provider };
                    self.mark_failing(provider, now, &err);
                    return self.fallback(request, cached.as_ref(), err);
                }
                Err(Failure::Unreachable(detail)) => {
                    let err = NetError::Unreachable {
                        provider,
                        detail: detail.to_owned(),
                    };
                    self.mark_failing(provider, now, &err);
                    return self.fallback(request, cached.as_ref(), err);
                }
                Err(Failure::Interrupted(detail)) => {
                    let err = NetError::Unreachable {
                        provider,
                        detail: detail.to_owned(),
                    };
                    if self.backoff(request, &mut retries, None, cancel)? {
                        continue;
                    }
                    self.mark_failing(provider, now, &err);
                    return self.fallback(request, cached.as_ref(), err);
                }
            };
            match head.status {
                200..=299 => {
                    self.mark_ok(provider);
                    return self.store_fresh(job, &key, source, &admitted, head, body);
                }
                304 => {
                    self.mark_ok(provider);
                    let Some(entry) = cached else {
                        return Err(NetError::Http {
                            provider,
                            status: 304,
                        });
                    };
                    let ttl = request.cache_ttl.unwrap_or_default();
                    let expires = now.plus(ttl);
                    lock(&self.store).renew(
                        job.profile,
                        &key,
                        now,
                        expires,
                        head.etag.as_deref(),
                    )?;
                    let renewed = CacheEntry {
                        queried_at: now,
                        expires_at: expires,
                        ..entry
                    };
                    return Ok(from_entry(&renewed, now, Freshness::Revalidated));
                }
                429 => {
                    let until = head
                        .retry_after
                        .as_deref()
                        .and_then(|v| retry_after(v, now))
                        .unwrap_or_else(|| now.plus(DEFAULT_RETRY_AFTER));
                    self.set_limited(provider, until);
                    limited_hits += 1;
                }
                500 | 502 | 503 | 504 => {
                    let err = NetError::Http {
                        provider,
                        status: head.status,
                    };
                    let hint = head
                        .retry_after
                        .as_deref()
                        .and_then(|v| retry_after(v, now));
                    if self.backoff(request, &mut retries, hint, cancel)? {
                        continue;
                    }
                    self.mark_failing(provider, now, &err);
                    return self.fallback(request, cached.as_ref(), err);
                }
                300..=399 => {
                    self.mark_ok(provider);
                    let location = head
                        .location
                        .as_deref()
                        .map(|l| sanitized(l).chars().take(80).collect())
                        .unwrap_or_default();
                    return Err(NetError::RedirectRefused { provider, location });
                }
                status => {
                    // O Provedor respondeu: está alcançável, o erro é do pedido.
                    self.mark_ok(provider);
                    return Err(NetError::Http { provider, status });
                }
            }
        }
    }

    /// Espera a próxima repetição de uma falha temporária; `Ok(false)` se não há mais
    /// repetições (ou o método não é idempotente).
    fn backoff(
        &self,
        request: &Request,
        retries: &mut u32,
        hint: Option<Timestamp>,
        cancel: &AtomicBool,
    ) -> Result<bool, NetError> {
        if request.method != Method::Get || *retries >= self.retry.max_retries {
            return Ok(false);
        }
        let delay = self.retry.delay(*retries, &mut lock(&self.rng));
        *retries += 1;
        let now = self.clock.now();
        let at = hint.map_or(now.plus(delay), |h| h.max(now.plus(delay)));
        if self.clock.sleep_until(at, cancel) {
            Ok(true)
        } else {
            Err(NetError::Cancelled)
        }
    }

    fn store_fresh(
        &self,
        job: &Job,
        key: &str,
        source: String,
        admitted: &Admitted,
        head: ResponseHead,
        body: Vec<u8>,
    ) -> Result<Fetched, NetError> {
        let request = &job.request;
        let now = self.clock.now();
        let entry = CacheEntry {
            provider: request.provider,
            source_url: source,
            status: head.status,
            content_type: head.content_type,
            body,
            queried_at: now,
            expires_at: now.plus(request.cache_ttl.unwrap_or_default()),
            etag: head.etag,
            last_modified: head.last_modified,
            license: admitted.endpoint.license.to_owned(),
            mode: mode_for(admitted.endpoint.capability),
        };
        if request.cache_ttl.is_some() && entry.status == 200 {
            lock(&self.store).put(job.profile, key, &entry)?;
        }
        Ok(from_entry(&entry, now, Freshness::Network))
    }

    fn run_stream(
        &self,
        request: &Request,
        profile: LocalProfileId,
        cancel: &AtomicBool,
        events: &Sender<StreamEvent>,
    ) {
        let started = self.clock.now();
        let result = self.stream_inner(request, profile, cancel, events);
        let endpoint = self.admit(request, profile).ok().map(|a| a.endpoint);
        let outcome = match &result {
            Ok(_) => Ok(empty_fetched(request, started)),
            Err(e) => Err(e.clone()),
        };
        self.record(request, endpoint, started, &outcome);
        match result {
            Ok(StreamEnd::Ended) => {
                let _ = events.send(StreamEvent::Ended);
            }
            Ok(StreamEnd::Closed) => {}
            Err(NetError::Cancelled) => {}
            Err(err) => {
                let _ = events.send(StreamEvent::Failed(err));
            }
        }
    }

    fn stream_inner(
        &self,
        request: &Request,
        profile: LocalProfileId,
        cancel: &AtomicBool,
        events: &Sender<StreamEvent>,
    ) -> Result<StreamEnd, NetError> {
        let provider = request.provider;
        let now = self.clock.now();
        if let Some(until) = self.limited_until(provider, now) {
            return Err(NetError::RateLimited { provider, until });
        }
        if !self.take_token(provider, cancel) {
            return Err(NetError::Cancelled);
        }
        let admitted = self.admit(request, profile)?;
        let outgoing = self.outgoing(request, &admitted, None, cancel);
        let now = self.clock.now();
        let (head, body) = match self.http.open(&outgoing, self.stream_read_timeout) {
            Ok(opened) => opened,
            Err(Failure::Cancelled) => return Err(NetError::Cancelled),
            Err(Failure::Timeout) => {
                let err = NetError::Timeout { provider };
                self.mark_failing(provider, now, &err);
                return Err(err);
            }
            Err(Failure::BodyTooLarge) => {
                return Err(NetError::BodyTooLarge {
                    provider,
                    limit: self.max_line,
                });
            }
            Err(Failure::Unreachable(detail) | Failure::Interrupted(detail)) => {
                let err = NetError::Unreachable {
                    provider,
                    detail: detail.to_owned(),
                };
                self.mark_failing(provider, now, &err);
                return Err(err);
            }
        };
        match head.status {
            200..=299 => self.mark_ok(provider),
            429 => {
                let until = head
                    .retry_after
                    .as_deref()
                    .and_then(|v| retry_after(v, now))
                    .unwrap_or_else(|| now.plus(DEFAULT_RETRY_AFTER));
                self.set_limited(provider, until);
                return Err(NetError::RateLimited { provider, until });
            }
            status => {
                return Err(NetError::Http { provider, status });
            }
        }
        let mut reader = NdjsonReader::new(body, self.max_line);
        loop {
            if cancel.load(Ordering::Acquire) {
                return Ok(StreamEnd::Closed);
            }
            match reader.next_item() {
                Ok(Some(NdjsonItem::Line(line))) => {
                    if events.send(StreamEvent::Line(line)).is_err() {
                        return Ok(StreamEnd::Closed);
                    }
                }
                Ok(Some(NdjsonItem::Heartbeat)) => {}
                Ok(None) => return Ok(StreamEnd::Ended),
                Err(NdjsonError::Io(ErrorKind::TimedOut | ErrorKind::WouldBlock)) => {
                    return Err(NetError::Timeout { provider });
                }
                Err(NdjsonError::LineTooLong { limit }) => {
                    return Err(NetError::BodyTooLarge { provider, limit });
                }
                Err(NdjsonError::InvalidUtf8) => {
                    return Err(NetError::Unreachable {
                        provider,
                        detail: "fluxo fora de UTF-8".to_owned(),
                    });
                }
                Err(NdjsonError::Truncated { .. } | NdjsonError::Io(_)) => {
                    if cancel.load(Ordering::Acquire) {
                        return Ok(StreamEnd::Closed);
                    }
                    return Err(NetError::Unreachable {
                        provider,
                        detail: "fluxo interrompido".to_owned(),
                    });
                }
            }
        }
    }
}

/// Registro de um fluxo que terminou bem (para o diário de operações).
fn empty_fetched(request: &Request, at: Timestamp) -> Fetched {
    Fetched {
        status: 200,
        content_type: None,
        body: Vec::new(),
        provenance: Provenance {
            provider: request.provider,
            queried_at: at,
            license: String::new(),
            mode: ImportMode::Api,
            source: String::new(),
        },
        freshness: Freshness::Network,
        age: Duration::ZERO,
    }
}

/// Resultado de [`Net::fetch`] a caminho. Descartar o `Pending` cancela o pedido.
#[derive(Debug)]
pub struct Pending {
    rx: Receiver<Result<Fetched, NetError>>,
    /// Acorda quem espera em [`Pending::wait`] quando o pedido é cancelado.
    wake: Sender<Result<Fetched, NetError>>,
    cancel: Arc<AtomicBool>,
    done: bool,
}

impl Pending {
    fn ready(result: Result<Fetched, NetError>) -> Self {
        let (tx, rx) = mpsc::channel();
        let _ = tx.send(result);
        Self {
            rx,
            wake: tx,
            cancel: Arc::new(AtomicBool::new(false)),
            done: false,
        }
    }

    /// Resultado, se já chegou (nunca bloqueia; chame a cada quadro). Entregue uma vez.
    pub fn try_take(&mut self) -> Option<Result<Fetched, NetError>> {
        if self.done {
            return None;
        }
        if self.cancel.load(Ordering::Acquire) {
            self.done = true;
            return Some(Err(NetError::Cancelled));
        }
        match self.rx.try_recv() {
            Ok(result) => {
                self.done = true;
                Some(result)
            }
            Err(TryRecvError::Empty) => None,
            Err(TryRecvError::Disconnected) => {
                self.done = true;
                Some(Err(NetError::Cancelled))
            }
        }
    }

    /// Espera até `timeout` (tempo real). Para threads de fundo e testes, não para a
    /// interface.
    pub fn wait_timeout(&mut self, timeout: Duration) -> Option<Result<Fetched, NetError>> {
        if let Some(result) = self.try_take() {
            return Some(result);
        }
        if self.done {
            return None;
        }
        match self.rx.recv_timeout(timeout) {
            Ok(result) => {
                self.done = true;
                if self.cancel.load(Ordering::Acquire) {
                    Some(Err(NetError::Cancelled))
                } else {
                    Some(result)
                }
            }
            Err(RecvTimeoutError::Timeout) => None,
            Err(RecvTimeoutError::Disconnected) => {
                self.done = true;
                Some(Err(NetError::Cancelled))
            }
        }
    }

    /// Bloqueia até o resultado. Para threads de fundo e testes, nunca para a interface.
    pub fn wait(mut self) -> Result<Fetched, NetError> {
        loop {
            if let Some(result) = self.wait_timeout(Duration::from_secs(3600)) {
                return result;
            }
            if self.done {
                return Err(NetError::Cancelled);
            }
        }
    }

    /// Cancela: quem espera recebe [`NetError::Cancelled`] na hora; um pedido na fila nem
    /// sai; um em andamento é abandonado (a thread se libera no fim do prazo do pedido).
    pub fn cancel(&self) {
        self.cancel.store(true, Ordering::Release);
        let _ = self.wake.send(Err(NetError::Cancelled));
    }

    pub fn is_cancelled(&self) -> bool {
        self.cancel.load(Ordering::Acquire)
    }
}

impl Drop for Pending {
    fn drop(&mut self) {
        self.cancel.store(true, Ordering::Release);
    }
}

/// Evento de um fluxo NDJSON.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum StreamEvent {
    /// Uma mensagem (JSON cru; o adaptador interpreta).
    Line(String),
    /// O Provedor encerrou o fluxo.
    Ended,
    /// O fluxo não abriu ou caiu.
    Failed(NetError),
}

/// Fluxo aberto por [`Net::stream`]. Descartar ou [`Stream::close`] encerra a thread.
#[derive(Debug)]
pub struct Stream {
    rx: Receiver<StreamEvent>,
    cancel: Arc<AtomicBool>,
}

impl Stream {
    /// Próximo evento, se já chegou (nunca bloqueia).
    pub fn try_next(&self) -> Option<StreamEvent> {
        self.rx.try_recv().ok()
    }

    /// Espera até `timeout` (tempo real) pelo próximo evento.
    pub fn next_timeout(&self, timeout: Duration) -> Option<StreamEvent> {
        self.rx.recv_timeout(timeout).ok()
    }

    /// Encerramento controlado: a thread para na próxima leitura (heartbeat ou mensagem).
    pub fn close(&self) {
        self.cancel.store(true, Ordering::Release);
    }
}

impl Drop for Stream {
    fn drop(&mut self) {
        self.close();
    }
}
