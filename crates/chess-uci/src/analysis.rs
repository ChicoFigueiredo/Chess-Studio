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

use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::mpsc::{self, Receiver, Sender};
use std::sync::{Arc, Mutex, PoisonError};
use std::thread;
use std::time::{Duration, Instant};

use chess_core::{CanonicalPosition, Color, Move};

use crate::controller::{Failure, FailureKind};
use crate::{
    Bound, Control, Engine, EngineConfig, EngineError, GoParams, InfoLine, Limits, PositionSpec,
    Stage, Timeouts, UciScore, effective_bounds, parse_options,
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
}

impl AnalysisConfig {
    pub fn new(engine: EngineConfig) -> Self {
        Self {
            engine,
            threads: None,
            hash_mb: None,
            limits: Limits::for_host(),
            interval: DEFAULT_INTERVAL,
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

#[derive(Debug)]
pub struct AnalysisController {
    jobs: Sender<Job>,
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
        thread::spawn(move || worker(&config, &jobs_rx, &events_tx, &worker_current));
        Self {
            jobs: jobs_tx,
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

fn worker(
    config: &AnalysisConfig,
    jobs: &Receiver<Job>,
    out: &Sender<AnalysisEvent>,
    current: &AtomicU64,
) {
    let fail = |e: &EngineError| {
        let _ = out.send(AnalysisEvent::Failed(Failure::from(e)));
    };
    let timeouts = config.engine.timeouts;
    let mut engine = match Engine::spawn(config.engine.clone()).and_then(|mut e| {
        e.new_game()?;
        Ok(e)
    }) {
        Ok(e) => e,
        Err(e) => return fail(&e),
    };
    let (threads, hash_mb) = match apply_resources(&mut engine, config) {
        Ok(applied) => applied,
        Err(e) => return fail(&e),
    };
    let _ = out.send(AnalysisEvent::Ready {
        name: engine.info().name.clone(),
        threads,
        hash_mb,
    });
    let mut multipv_sent: Option<u8> = None;
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
            timeouts,
        };
        if let Err(e) = run_analysis(&mut engine, &run, &mut multipv_sent, current, out) {
            return fail(&e); // `engine` cai aqui e o processo é encerrado
        }
    }
    let _ = engine.quit();
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

fn run_analysis(
    engine: &mut Engine,
    run: &Run<'_>,
    multipv_sent: &mut Option<u8>,
    current: &AtomicU64,
    out: &Sender<AnalysisEvent>,
) -> Result<(), EngineError> {
    let Some(position) = resolve(run.position) else {
        // Posição que o núcleo não aceita: nada a analisar.
        let _ = out.send(AnalysisEvent::Update(AnalysisUpdate {
            id: run.id,
            lines: Vec::new(),
        }));
        return Ok(());
    };
    if *multipv_sent != Some(run.multipv)
        && engine.info().options.iter().any(|o| o.name == "MultiPV")
    {
        engine.set_option("MultiPV", Some(&run.multipv.to_string()))?;
        *multipv_sent = Some(run.multipv);
    }
    engine.set_position(run.position)?;
    engine.go(&GoParams::infinite())?;
    let mut collector = Collector::new(position, usize::from(run.multipv));
    let mut last_emit: Option<Instant> = None;
    loop {
        if current.load(Ordering::SeqCst) != run.id {
            return stop_search(engine, run.timeouts);
        }
        let result = engine.wait_bestmove_with(SLICE, |info| collector.feed(info));
        let finished = match result {
            Ok(_) => true,
            Err(EngineError::Timeout { killed: false, .. }) => false,
            Err(e) => return Err(e),
        };
        let due = last_emit.is_none_or(|t| t.elapsed() >= run.interval);
        if collector.dirty && (due || finished) {
            collector.dirty = false;
            last_emit = Some(Instant::now());
            let _ = out.send(AnalysisEvent::Update(AnalysisUpdate {
                id: run.id,
                lines: collector.lines(),
            }));
        }
        if finished {
            return Ok(());
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
}

impl Collector {
    fn new(root: CanonicalPosition, multipv: usize) -> Self {
        Self {
            root,
            slots: vec![None; multipv],
            dirty: false,
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
