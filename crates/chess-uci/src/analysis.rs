//! Análise contínua (C8): uma segunda Engine, em **slot próprio**, rodando `go infinite`
//! sobre a Posição que o usuário estuda. Nunca é a Engine que joga a Partida (outro
//! processo, outra thread de trabalho) e por isso não interfere no relógio nem no Lance
//! adversário.
//!
//! O processo e toda a conversa UCI vivem numa thread de trabalho; quem chama só enfileira
//! pedidos e consulta eventos sem bloquear (nunca a thread da UI). As `info` da Engine são
//! interpretadas aqui: placar sempre do ponto de vista das **brancas**, variante principal
//! validada pelo `chess-core` (Lance ilegal corta a linha) e escrita em SAN. As
//! atualizações saem no máximo a cada [`AnalysisConfig::interval`] (padrão ~7 Hz); a
//! cada troca de Posição a busca recebe `stop`, a nova `position` é enviada e o que sobrou
//! da busca antiga é descartado pelo `id`.

use std::collections::BTreeMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::mpsc::{self, Receiver, Sender};
use std::sync::{Arc, Mutex, PoisonError};
use std::thread;
use std::time::{Duration, Instant};

use chess_core::{CanonicalPosition, Color, Move};

use crate::controller::{Failure, FailureKind};
use crate::{
    Bound, Control, Duty, Engine, EngineConfig, EngineError, GoParams, Hardware, InfoLine, Limits,
    LoadPlan, OptionSpec, PositionSpec, Stage, Timeouts, UciScore, advertised_backends,
    classify_failure, describe_failure, detect_hardware, effective_bounds, fallback_backend,
    is_gpu_backend, parse_options, recommended_backend,
};

/// Fatia de espera do worker: é a latência para notar uma troca de Posição.
const SLICE: Duration = Duration::from_millis(25);
/// Intervalo padrão entre atualizações (~7 Hz).
pub const DEFAULT_INTERVAL: Duration = Duration::from_millis(150);
/// Máximo de variantes principais (`MultiPV`) que a análise pede.
pub const MAX_MULTIPV: u8 = 3;

/// Placar do ponto de vista das **brancas**.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Score {
    /// Centipeões; positivo favorece as brancas.
    Cp(i32),
    /// Mate em N lances; positivo: as brancas dão mate, negativo: as pretas dão.
    Mate(i32),
}

/// Uma variante principal.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AnalysisLine {
    /// Posição na classificação da Engine (1 = melhor).
    pub multipv: usize,
    pub depth: u32,
    pub seldepth: Option<u32>,
    pub score: Score,
    pub nodes: Option<u64>,
    pub nps: Option<u64>,
    pub time_ms: Option<u64>,
    /// Lances da variante, todos legais a partir da Posição analisada.
    pub pv: Vec<Move>,
    /// Os mesmos Lances em SAN (mesmo tamanho de `pv`).
    pub san: Vec<String>,
}

/// Estado atual da análise de uma Posição.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AnalysisUpdate {
    /// Pedido (`analyze`) a que as linhas pertencem.
    pub id: u64,
    /// Ordenadas por `multipv`. Vazia se a Posição não tem Lances ou não foi aceita.
    pub lines: Vec<AnalysisLine>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AnalysisEvent {
    /// Engine pronta; `threads` e `hash_mb` são o que de fato foi configurado (após os
    /// limites do app), quando a Engine anuncia a opção.
    Ready {
        name: Option<String>,
        threads: Option<u32>,
        hash_mb: Option<u32>,
        /// Backend em uso (Lc0); `None` para Engines sem a opção `Backend`.
        backend: Option<String>,
        /// Backends que a Engine anuncia (vazio se não tem a opção).
        backends: Vec<String>,
    },
    /// O backend de GPU não subiu e a análise recuou para um de CPU (UCI-06).
    BackendFallback {
        from: String,
        to: String,
        /// Frase em pt-BR com o motivo provável.
        message: String,
    },
    Update(AnalysisUpdate),
    /// A Engine de análise não serve mais; crie outro controlador para tentar de novo.
    Failed(Failure),
}

/// Como iniciar a análise.
#[derive(Debug, Clone)]
pub struct AnalysisConfig {
    pub engine: EngineConfig,
    /// Threads desejadas; reduzidas ao menor entre o anunciado pela Engine e `limits`.
    pub threads: Option<u32>,
    /// Hash (MB) desejado; mesma regra.
    pub hash_mb: Option<u32>,
    pub limits: Limits,
    /// Intervalo mínimo entre duas atualizações.
    pub interval: Duration,
    /// Backend escolhido à mão (Lc0); `None` = escolha automática pelo que a Engine anuncia
    /// e pela máquina. Um `Backend` nas opções da Engine vale como escolha manual.
    pub backend: Option<String>,
    /// Máquina (GPU); `None` = detectar ao subir a Engine.
    pub hardware: Option<Hardware>,
    /// Sistema operacional (`std::env::consts::OS`); fixável nos testes.
    pub os: &'static str,
    /// Recuar para a CPU quando o backend de GPU não sobe.
    pub cpu_fallback: bool,
}

impl AnalysisConfig {
    pub fn new(engine: EngineConfig) -> Self {
        Self {
            engine,
            threads: None,
            hash_mb: None,
            limits: Limits::for_host(),
            interval: DEFAULT_INTERVAL,
            backend: None,
            hardware: None,
            os: std::env::consts::OS,
            cpu_fallback: true,
        }
    }
}

#[derive(Debug)]
enum Job {
    Analyze {
        id: u64,
        position: PositionSpec,
        multipv: u8,
    },
    Shutdown,
}

/// Plano de carga compartilhado com o worker: troca atômica, com contador de geração para o
/// worker notar a mudança sem travar.
#[derive(Debug)]
struct LoadShared {
    generation: AtomicU64,
    plan: Mutex<LoadPlan>,
}

impl LoadShared {
    fn new() -> Self {
        Self {
            generation: AtomicU64::new(0),
            plan: Mutex::new(LoadPlan::FULL),
        }
    }

    fn generation(&self) -> u64 {
        self.generation.load(Ordering::SeqCst)
    }

    fn snapshot(&self) -> (u64, LoadPlan) {
        let plan = self.plan.lock().unwrap_or_else(PoisonError::into_inner);
        (self.generation(), *plan)
    }

    fn set(&self, new: LoadPlan) {
        let mut plan = self.plan.lock().unwrap_or_else(PoisonError::into_inner);
        if *plan != new {
            *plan = new;
            self.generation.fetch_add(1, Ordering::SeqCst);
        }
    }
}

#[derive(Debug)]
pub struct AnalysisController {
    jobs: Sender<Job>,
    load: Arc<LoadShared>,
    /// Em `Mutex` só para ser `Sync` (recurso do Bevy); sem disputa.
    events: Mutex<Receiver<AnalysisEvent>>,
    current: Arc<AtomicU64>,
    next_id: u64,
    dead: bool,
}

impl AnalysisController {
    /// Inicia o worker, que sobe a Engine e aplica Threads/Hash. Retorna na hora.
    pub fn start(config: AnalysisConfig) -> Self {
        let (jobs_tx, jobs_rx) = mpsc::channel();
        let (events_tx, events_rx) = mpsc::channel();
        let current = Arc::new(AtomicU64::new(0));
        let worker_current = Arc::clone(&current);
        let load = Arc::new(LoadShared::new());
        let worker_load = Arc::clone(&load);
        thread::spawn(move || {
            worker(&config, &jobs_rx, &events_tx, &worker_current, &worker_load);
        });
        Self {
            jobs: jobs_tx,
            load,
            events: Mutex::new(events_rx),
            current,
            next_id: 1,
            dead: false,
        }
    }

    /// Analisa `position` com `multipv` variantes (1 a [`MAX_MULTIPV`]); substitui a
    /// análise anterior e devolve o `id` do pedido.
    pub fn analyze(&mut self, position: PositionSpec, multipv: u8) -> u64 {
        let id = self.next_id;
        self.next_id += 1;
        self.current.store(id, Ordering::SeqCst);
        let job = Job::Analyze {
            id,
            position,
            multipv: multipv.clamp(1, MAX_MULTIPV),
        };
        let _ = self.jobs.send(job);
        id
    }

    /// Aplica o plano do governador de carga (C10): a busca em curso recebe `stop`, os tetos de
    /// Threads/lote/cache são reenviados e a busca recomeça; plano pausado deixa a Engine
    /// parada (o pedido continua valendo) e o intermitente roda em rajadas. Plano igual ao
    /// atual não toca na Engine.
    pub fn set_load(&mut self, plan: LoadPlan) {
        self.load.set(plan);
    }

    /// Para a análise (a Engine recebe `stop`); o que ainda chegar é descartado.
    pub fn stop(&mut self) {
        self.current.store(0, Ordering::SeqCst);
    }

    /// Há uma análise pedida e não parada?
    pub fn is_analyzing(&self) -> bool {
        self.current.load(Ordering::SeqCst) != 0
    }

    /// A Engine falhou e o controlador não aceita mais pedidos.
    pub fn is_dead(&self) -> bool {
        self.dead
    }

    /// Próximo evento, sem bloquear. Atualizações de pedidos obsoletos são descartadas.
    pub fn poll(&mut self) -> Option<AnalysisEvent> {
        loop {
            let received = self
                .events
                .get_mut()
                .unwrap_or_else(PoisonError::into_inner)
                .try_recv();
            match received {
                Ok(AnalysisEvent::Update(u)) => {
                    if u.id == self.current.load(Ordering::SeqCst) {
                        return Some(AnalysisEvent::Update(u));
                    }
                }
                Ok(event @ AnalysisEvent::Failed(_)) => {
                    if self.dead {
                        continue;
                    }
                    self.dead = true;
                    self.current.store(0, Ordering::SeqCst);
                    return Some(event);
                }
                Ok(event) => return Some(event),
                Err(mpsc::TryRecvError::Empty) => return None,
                Err(mpsc::TryRecvError::Disconnected) => {
                    if self.dead {
                        return None;
                    }
                    self.dead = true;
                    self.current.store(0, Ordering::SeqCst);
                    return Some(AnalysisEvent::Failed(Failure::new(
                        FailureKind::Crashed,
                        "o worker da análise terminou inesperadamente",
                    )));
                }
            }
        }
    }
}

impl Drop for AnalysisController {
    fn drop(&mut self) {
        self.current.store(0, Ordering::SeqCst);
        let _ = self.jobs.send(Job::Shutdown);
    }
}

// --- worker ------------------------------------------------------------------

/// Engine de análise pronta (backend escolhido, recursos aplicados).
struct Started {
    engine: Engine,
    specs: Vec<OptionSpec>,
    threads: Option<u32>,
    hash_mb: Option<u32>,
    backend: Option<String>,
    backends: Vec<String>,
}

/// Estado do worker que dura mais que um pedido.
struct State {
    multipv_sent: Option<u8>,
    /// Última geração do plano de carga aplicada à Engine.
    applied_gen: u64,
    /// Valor numérico enviado por último de cada opção de carga.
    sent: BTreeMap<String, i64>,
}

fn worker(
    config: &AnalysisConfig,
    jobs: &Receiver<Job>,
    out: &Sender<AnalysisEvent>,
    current: &AtomicU64,
    load: &LoadShared,
) {
    let started = match start_engine(config, out) {
        Ok(started) => started,
        Err(failure) => {
            let _ = out.send(AnalysisEvent::Failed(failure));
            return;
        }
    };
    let Started {
        mut engine,
        specs,
        threads,
        hash_mb,
        backend,
        backends,
    } = started;
    let _ = out.send(AnalysisEvent::Ready {
        name: engine.info().name.clone(),
        threads,
        hash_mb,
        backend,
        backends,
    });
    let mut state = State {
        multipv_sent: None,
        applied_gen: 0,
        sent: BTreeMap::new(),
    };
    if let Some(t) = threads {
        state.sent.insert("Threads".into(), i64::from(t));
    }
    while let Ok(job) = jobs.recv() {
        let Job::Analyze {
            id,
            position,
            multipv,
        } = job
        else {
            break;
        };
        // Pedido já substituído/parado antes de começar: nem chega à Engine.
        if current.load(Ordering::SeqCst) != id {
            continue;
        }
        let run = Run {
            id,
            position: &position,
            multipv,
            interval: config.interval,
            timeouts: config.engine.timeouts,
        };
        let ctx = Ctx {
            config,
            specs: &specs,
            current,
            load,
            out,
        };
        if let Err(e) = serve(&mut engine, &ctx, &run, &mut state) {
            let _ = out.send(AnalysisEvent::Failed(Failure::from(&e)));
            return; // `engine` cai aqui e o processo é encerrado
        }
    }
    let _ = engine.quit();
}

/// O que o worker compartilha com o controlador, para passar de uma vez.
struct Ctx<'a> {
    config: &'a AnalysisConfig,
    specs: &'a [OptionSpec],
    current: &'a AtomicU64,
    load: &'a LoadShared,
    out: &'a Sender<AnalysisEvent>,
}

/// Atende um pedido até ele ser substituído, parado ou terminar, obedecendo ao plano de
/// carga: reaplica os tetos quando o plano muda, espera enquanto pausado e roda em rajadas
/// quando o plano pede.
fn serve(
    engine: &mut Engine,
    ctx: &Ctx<'_>,
    run: &Run<'_>,
    state: &mut State,
) -> Result<(), EngineError> {
    loop {
        if ctx.current.load(Ordering::SeqCst) != run.id {
            return Ok(());
        }
        let (generation, plan) = ctx.load.snapshot();
        if generation != state.applied_gen {
            apply_load(engine, ctx.specs, ctx.config, &plan, &mut state.sent)?;
            state.applied_gen = generation;
        }
        if plan.paused {
            thread::sleep(SLICE);
            continue;
        }
        let burst = plan.duty;
        match run_analysis(engine, run, ctx, state, burst)? {
            Outcome::Finished => return Ok(()),
            Outcome::Reconfigure => {}
            Outcome::Burst(pause) => {
                // Descansa entre as rajadas, atento a pedido novo e a mudança de plano.
                let until = Instant::now() + pause;
                while Instant::now() < until
                    && ctx.current.load(Ordering::SeqCst) == run.id
                    && ctx.load.generation() == state.applied_gen
                {
                    thread::sleep(SLICE.min(until.saturating_duration_since(Instant::now())));
                }
            }
        }
    }
}

/// Sobe a Engine, escolhe o backend e aplica Threads/Hash. Se o backend de GPU não sobe,
/// recua para um de CPU (quando permitido) e avisa com um evento.
fn start_engine(config: &AnalysisConfig, out: &Sender<AnalysisEvent>) -> Result<Started, Failure> {
    let hardware = config.hardware.unwrap_or_else(detect_hardware);
    let mut base = config.engine.clone();
    let mut manual = config.backend.clone();
    base.options.retain(|(name, value)| {
        if name.eq_ignore_ascii_case("backend") {
            if manual.is_none() {
                manual = Some(value.clone());
            }
            false
        } else {
            true
        }
    });
    let manual = manual.filter(|b| !matches!(b.trim(), "" | "<autodiscover>" | "auto"));
    let mut forced: Option<String> = None;
    loop {
        let fail = |e: &EngineError| Failure::from(e);
        let mut engine = Engine::spawn(base.clone()).map_err(|e| fail(&e))?;
        engine.new_game().map_err(|e| fail(&e))?;
        let specs = parse_options(&engine.info().options);
        let backends = advertised_backends(&engine.info().options);
        let chosen = if backends.is_empty() {
            None
        } else if let Some(wanted) = forced.clone().or_else(|| manual.clone()) {
            if !backends.iter().any(|b| b == &wanted) {
                return Err(Failure::new(
                    FailureKind::Protocol,
                    format!(
                        "O backend {wanted:?} não existe nesta Engine (disponíveis: {}). \
                         Escolha outro no gerenciador de Engines.",
                        backends.join(", ")
                    ),
                ));
            }
            Some(wanted)
        } else {
            recommended_backend(config.os, hardware, &backends)
        };
        let attempt = (|| {
            if let Some(b) = &chosen {
                engine.set_option("Backend", Some(b))?;
            }
            let (threads, hash_mb) = apply_resources(&mut engine, config)?;
            if chosen.is_some() {
                warm_up(&mut engine, config.engine.timeouts)?;
            }
            Ok((threads, hash_mb))
        })();
        match attempt {
            Ok((threads, hash_mb)) => {
                return Ok(Started {
                    engine,
                    specs,
                    threads,
                    hash_mb,
                    backend: chosen,
                    backends,
                });
            }
            Err(e) => {
                let Some(failed) = chosen else {
                    return Err(Failure::from(&e));
                };
                // O leitor de stderr pode atrasar alguns ms em relação à morte do processo.
                thread::sleep(Duration::from_millis(60));
                let log: String = engine
                    .log()
                    .iter()
                    .map(|entry| entry.line.as_str())
                    .collect::<Vec<_>>()
                    .join("\n");
                let problem = classify_failure(&log);
                let fallback = (config.cpu_fallback && forced.is_none() && is_gpu_backend(&failed))
                    .then(|| fallback_backend(config.os, &backends, &failed))
                    .flatten();
                match fallback {
                    Some(cpu) => {
                        let message = describe_failure(&failed, problem, Some(&cpu));
                        let _ = out.send(AnalysisEvent::BackendFallback {
                            from: failed,
                            to: cpu.clone(),
                            message,
                        });
                        forced = Some(cpu);
                    }
                    None => {
                        let mut failure = Failure::from(&e);
                        failure.message = describe_failure(&failed, problem, None);
                        return Err(failure);
                    }
                }
            }
        }
    }
}

/// O Lc0 só carrega a rede na primeira busca: uma busca curta força a falha de backend a
/// aparecer agora (e recuar para a CPU) em vez de no meio da análise.
fn warm_up(engine: &mut Engine, timeouts: Timeouts) -> Result<(), EngineError> {
    engine.set_position(&PositionSpec::startpos())?;
    engine.go(&GoParams::movetime(Duration::from_millis(50)))?;
    engine.wait_bestmove(timeouts.ready.saturating_mul(3))?;
    engine.new_game()
}

/// Threads e Hash: só se a Engine os anuncia, sempre dentro dos limites do app.
fn apply_resources(
    engine: &mut Engine,
    config: &AnalysisConfig,
) -> Result<(Option<u32>, Option<u32>), EngineError> {
    let specs = parse_options(&engine.info().options);
    let mut set = |name: &str, wanted: Option<u32>| -> Result<Option<u32>, EngineError> {
        let Some(wanted) = wanted else {
            return Ok(None);
        };
        let Some(spec) = specs.iter().find(|s| {
            s.name.eq_ignore_ascii_case(name) && matches!(s.control, Control::Spin { .. })
        }) else {
            return Ok(None);
        };
        let (min, max) = effective_bounds(spec, &config.limits).unwrap_or((1, i64::MAX));
        let value = i64::from(wanted).clamp(min, max);
        engine.set_option(&spec.name, Some(&value.to_string()))?;
        Ok(u32::try_from(value).ok())
    };
    let threads = set("Threads", config.threads)?;
    let hash = set("Hash", config.hash_mb)?;
    engine.is_ready()?;
    Ok((threads, hash))
}

/// Aplica os tetos do plano de carga: o valor configurado (ou o padrão da Engine) limitado
/// pelo teto. Só envia o que mudou. A Engine está parada (`stop` já feito).
fn apply_load(
    engine: &mut Engine,
    specs: &[OptionSpec],
    config: &AnalysisConfig,
    plan: &LoadPlan,
    sent: &mut BTreeMap<String, i64>,
) -> Result<(), EngineError> {
    let knobs: [(&str, Option<u32>, Option<u32>); 3] = [
        ("Threads", plan.threads_cap, config.threads),
        ("MinibatchSize", plan.minibatch_cap, None),
        ("NNCacheSize", plan.nn_cache_cap, None),
    ];
    let mut changed = false;
    for (name, cap, wanted) in knobs {
        let Some(spec) = specs.iter().find(|s| {
            s.name.eq_ignore_ascii_case(name) && matches!(s.control, Control::Spin { .. })
        }) else {
            continue;
        };
        let Control::Spin { default, .. } = spec.control else {
            continue;
        };
        let (min, max) = effective_bounds(spec, &config.limits).unwrap_or((0, i64::MAX));
        let base = wanted.map_or(default, i64::from).clamp(min, max);
        // Valor 0 é "automático" no Lc0: sob um teto, o teto vale.
        let value = match (cap.map(i64::from), base) {
            (Some(c), 0) => c,
            (Some(c), b) => b.min(c),
            (None, b) => b,
        }
        .clamp(min, max);
        if sent.get(&spec.name).copied().unwrap_or(default) != value {
            engine.set_option(&spec.name, Some(&value.to_string()))?;
            sent.insert(spec.name.clone(), value);
            changed = true;
        }
    }
    if changed {
        engine.is_ready()?;
    }
    Ok(())
}

struct Run<'a> {
    id: u64,
    position: &'a PositionSpec,
    multipv: u8,
    interval: Duration,
    timeouts: Timeouts,
}

fn resolve(spec: &PositionSpec) -> Option<CanonicalPosition> {
    let mut position = match &spec.fen {
        None => CanonicalPosition::start(),
        Some(fen) => CanonicalPosition::from_fen(fen).ok()?,
    };
    for &mv in &spec.moves {
        position = position.play(mv).ok()?;
    }
    Some(position)
}

/// Como uma rodada de busca terminou.
enum Outcome {
    /// Pedido substituído, parado ou a Engine encerrou a busca sozinha.
    Finished,
    /// O plano de carga mudou: reaplicar e buscar de novo.
    Reconfigure,
    /// Fim de uma rajada: descansar este tempo e retomar.
    Burst(Duration),
}

fn run_analysis(
    engine: &mut Engine,
    run: &Run<'_>,
    ctx: &Ctx<'_>,
    state: &mut State,
    burst: Option<Duty>,
) -> Result<Outcome, EngineError> {
    let Some(position) = resolve(run.position) else {
        // Posição que o núcleo não aceita: nada a analisar.
        let _ = ctx.out.send(AnalysisEvent::Update(AnalysisUpdate {
            id: run.id,
            lines: Vec::new(),
        }));
        return Ok(Outcome::Finished);
    };
    if state.multipv_sent != Some(run.multipv)
        && engine.info().options.iter().any(|o| o.name == "MultiPV")
    {
        engine.set_option("MultiPV", Some(&run.multipv.to_string()))?;
        state.multipv_sent = Some(run.multipv);
    }
    engine.set_position(run.position)?;
    engine.go(&GoParams::infinite())?;
    let began = Instant::now();
    let mut collector = Collector::new(position, usize::from(run.multipv));
    let mut last_emit: Option<Instant> = None;
    let emit = |collector: &mut Collector, last_emit: &mut Option<Instant>| {
        collector.dirty = false;
        *last_emit = Some(Instant::now());
        let _ = ctx.out.send(AnalysisEvent::Update(AnalysisUpdate {
            id: run.id,
            lines: collector.lines(),
        }));
    };
    loop {
        // Pedido novo, plano novo ou fim da rajada: para a busca (sem perder o que já veio).
        let interrupt = if ctx.current.load(Ordering::SeqCst) != run.id {
            Some(Outcome::Finished)
        } else if ctx.load.generation() != state.applied_gen {
            Some(Outcome::Reconfigure)
        } else {
            // A rajada só termina depois de devolver ao menos uma linha: com a máquina
            // carregada, a Engine pode demorar mais que a rajada para responder, e uma
            // rajada vazia seria só uma pausa a mais. O teto (4x) evita esperar para sempre
            // uma Engine muda.
            burst
                .filter(|d| {
                    let elapsed = began.elapsed();
                    elapsed >= d.run && (collector.seen || elapsed >= d.run.saturating_mul(4))
                })
                .map(|d| Outcome::Burst(d.pause))
        };
        if let Some(outcome) = interrupt {
            if collector.dirty && ctx.current.load(Ordering::SeqCst) == run.id {
                emit(&mut collector, &mut last_emit);
            }
            stop_search(engine, run.timeouts)?;
            return Ok(outcome);
        }
        let result = engine.wait_bestmove_with(SLICE, |info| collector.feed(info));
        let finished = match result {
            Ok(_) => true,
            Err(EngineError::Timeout { killed: false, .. }) => false,
            Err(e) => return Err(e),
        };
        let due = last_emit.is_none_or(|t| t.elapsed() >= run.interval);
        if collector.dirty && (due || finished) {
            emit(&mut collector, &mut last_emit);
        }
        if finished {
            return Ok(Outcome::Finished);
        }
    }
}

/// `stop` e espera o `bestmove` (descartado) por até `stop_grace`.
fn stop_search(engine: &mut Engine, timeouts: Timeouts) -> Result<(), EngineError> {
    engine.stop()?;
    match engine.wait_bestmove(timeouts.stop_grace) {
        Ok(_) => Ok(()),
        Err(EngineError::Timeout { .. }) => Err(EngineError::Timeout {
            stage: Stage::Stop,
            after: timeouts.stop_grace,
            killed: true,
        }),
        Err(e) => Err(e),
    }
}

// --- interpretação das `info` -----------------------------------------------

/// Acumula a melhor linha conhecida de cada `multipv`.
struct Collector {
    root: CanonicalPosition,
    slots: Vec<Option<AnalysisLine>>,
    dirty: bool,
    /// Já veio ao menos uma linha aproveitável nesta busca.
    seen: bool,
}

impl Collector {
    fn new(root: CanonicalPosition, multipv: usize) -> Self {
        Self {
            root,
            slots: vec![None; multipv],
            dirty: false,
            seen: false,
        }
    }

    fn lines(&self) -> Vec<AnalysisLine> {
        self.slots.iter().flatten().cloned().collect()
    }

    fn feed(&mut self, info: &InfoLine) {
        // Limites (`lowerbound`/`upperbound`) piscam o placar: só linhas exatas.
        if info.bound != Bound::Exact {
            return;
        }
        let (Some(depth), Some(score)) = (info.depth, info.score) else {
            return;
        };
        let index = usize::try_from(info.multipv.unwrap_or(1))
            .unwrap_or(usize::MAX)
            .saturating_sub(1);
        if index >= self.slots.len() {
            return;
        }
        let (pv, san) = self.validate(&info.pv);
        if pv.is_empty() {
            return;
        }
        let white = self.root.side_to_move() == Color::White;
        let flip = |v: i32| if white { v } else { v.saturating_neg() };
        let score = match score {
            UciScore::Cp(v) => Score::Cp(flip(v)),
            UciScore::Mate(v) => Score::Mate(flip(v)),
        };
        self.slots[index] = Some(AnalysisLine {
            multipv: index + 1,
            depth,
            seldepth: info.seldepth,
            score,
            nodes: info.nodes,
            nps: info.nps,
            time_ms: info.time_ms,
            pv,
            san,
        });
        self.dirty = true;
        self.seen = true;
    }

    /// Lances legais em sequência a partir da raiz; para no primeiro inválido.
    fn validate(&self, text: &[String]) -> (Vec<Move>, Vec<String>) {
        let mut position = self.root.clone();
        let (mut moves, mut san) = (Vec::new(), Vec::new());
        for word in text {
            let Ok(mv) = word.parse::<Move>() else { break };
            let Ok(written) = position.san(mv) else { break };
            let Ok(next) = position.play(mv) else { break };
            moves.push(mv);
            san.push(written);
            position = next;
        }
        (moves, san)
    }
}
