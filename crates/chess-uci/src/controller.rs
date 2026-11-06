//! Controlador de Engine: o processo e **toda** a conversa UCI (inclusive a escrita no
//! stdin) vivem numa thread de trabalho própria; quem chama só enfileira pedidos e
//! consulta resultados sem bloquear (nunca a thread da UI).
//!
//! Garantias:
//! - handshake, `setoption`, `position`, `go` e `stop` rodam no worker;
//! - todo pedido tem prazo: passado `limit` + folga o worker manda `stop`; sem resposta
//!   no `stop_grace` o processo é encerrado e o pedido falha ([`FailureKind::Timeout`]);
//! - um vigia do lado do chamador ([`EngineController::poll`]) cobre o pior caso, em que
//!   o próprio worker trava (ex.: pipe cheio): falha com [`FailureKind::Unresponsive`]
//!   e abandona a thread;
//! - pedidos obsoletos (`cancel`, ou um novo `search`) são descartados pelo `id` e a busca
//!   em curso recebe `stop`.

use std::collections::VecDeque;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::mpsc::{self, Receiver, Sender};
use std::sync::{Arc, Mutex, PoisonError};
use std::thread;
use std::time::{Duration, Instant};

use crate::{
    BestMove, Engine, EngineConfig, EngineError, EngineInfo, GoParams, PositionSpec, Stage,
    Timeouts,
};

/// Fatia de espera do worker: é a latência para notar `cancel` durante a busca.
const SLICE: Duration = Duration::from_millis(25);
/// Margem do vigia além dos prazos do worker.
const WATCHDOG_MARGIN: Duration = Duration::from_millis(750);

/// Força pedida à Engine (DEC-29).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Strength {
    /// Sem limitação.
    Full,
    /// Elo simples: `UCI_LimitStrength`/`UCI_Elo`, ou `Skill Level` se a Engine não
    /// anunciar Elo.
    Elo(u32),
}

/// O que de fato foi configurado na Engine.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StrengthApplied {
    Full,
    /// `UCI_Elo`; `used` difere de `requested` quando o pedido saiu da faixa da Engine.
    Elo {
        requested: u32,
        used: u32,
    },
    /// A Engine não anuncia Elo: `Skill Level` derivado do Elo pedido.
    SkillLevel {
        requested: u32,
        level: u32,
    },
    /// A Engine não anuncia nenhuma opção de força.
    Unsupported,
}

/// Classe de falha, para a interface escolher mensagem e ação.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FailureKind {
    /// O executável não iniciou.
    Spawn,
    /// O processo morreu ou fechou a saída.
    Crashed,
    /// Estourou o prazo de handshake/busca (o processo foi encerrado).
    Timeout,
    /// O worker parou de responder (vigia).
    Unresponsive,
    /// A Engine falou fora do protocolo.
    Protocol,
    /// A Engine propôs um Lance que a Partida recusou (a Engine segue viva).
    IllegalMove,
}

/// Falha da Engine, já com texto para mostrar ao usuário.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Failure {
    pub kind: FailureKind,
    pub message: String,
}

impl Failure {
    pub(crate) fn new(kind: FailureKind, message: impl Into<String>) -> Self {
        Self {
            kind,
            message: message.into(),
        }
    }
}

impl From<&EngineError> for Failure {
    fn from(e: &EngineError) -> Self {
        let kind = match e {
            EngineError::Spawn { .. } => FailureKind::Spawn,
            EngineError::Timeout { .. } => FailureKind::Timeout,
            EngineError::Crashed { .. } | EngineError::Terminated | EngineError::Io(_) => {
                FailureKind::Crashed
            }
            EngineError::IllegalMove { .. } => FailureKind::IllegalMove,
            _ => FailureKind::Protocol,
        };
        Self::new(kind, e.to_string())
    }
}

/// Um pedido de busca.
#[derive(Debug, Clone)]
pub struct SearchRequest {
    pub position: PositionSpec,
    pub go: GoParams,
    /// Tempo máximo de pensar antes do `stop` (a folga do adaptador é somada).
    pub limit: Duration,
}

/// O que o controlador entrega ao chamador.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ControllerEvent {
    /// Handshake feito e força configurada.
    Ready {
        name: Option<String>,
        strength: StrengthApplied,
    },
    /// `bestmove` (texto UCI cru: a Partida é quem valida).
    Move { id: u64, uci: String },
    /// `bestmove (none)`.
    NoMove { id: u64 },
    /// A Engine não serve mais. `id` é o pedido afetado, se havia. Depois disto o
    /// controlador está morto: crie outro (nova instância) para tentar de novo.
    Failed { id: Option<u64>, failure: Failure },
}

#[derive(Debug)]
enum Job {
    Search { id: u64, req: SearchRequest },
    Shutdown,
}

#[derive(Debug)]
struct Active {
    id: u64,
    watchdog: Instant,
}

#[derive(Debug)]
pub struct EngineController {
    jobs: Sender<Job>,
    /// Em `Mutex` só para o controlador ser `Sync` (recurso do Bevy); sem disputa.
    events: Mutex<Receiver<ControllerEvent>>,
    current: Arc<AtomicU64>,
    next_id: u64,
    active: Option<Active>,
    pending: VecDeque<ControllerEvent>,
    ready: bool,
    dead: bool,
    startup_budget: Duration,
    timeouts: Timeouts,
}

impl EngineController {
    /// Inicia o worker, que sobe a Engine (handshake) e configura a força. Retorna na
    /// hora; o resultado chega por [`EngineController::poll`].
    pub fn start(config: EngineConfig, strength: Strength) -> Self {
        let (jobs_tx, jobs_rx) = mpsc::channel();
        let (events_tx, events_rx) = mpsc::channel();
        let current = Arc::new(AtomicU64::new(0));
        let timeouts = config.timeouts;
        let worker_current = Arc::clone(&current);
        thread::spawn(move || worker(config, strength, &jobs_rx, &events_tx, &worker_current));
        Self {
            jobs: jobs_tx,
            events: Mutex::new(events_rx),
            current,
            next_id: 1,
            active: None,
            pending: VecDeque::new(),
            ready: false,
            dead: false,
            startup_budget: timeouts.handshake + timeouts.ready * 2,
            timeouts,
        }
    }

    /// Enfileira uma busca (substitui a anterior, se houver) e devolve seu `id`.
    pub fn search(&mut self, request: SearchRequest) -> u64 {
        let id = self.next_id;
        self.next_id += 1;
        self.current.store(id, Ordering::SeqCst);
        let mut watchdog = Instant::now()
            + request.limit
            + self.timeouts.search_slack
            + self.timeouts.stop_grace
            + WATCHDOG_MARGIN;
        if !self.ready {
            watchdog += self.startup_budget;
        }
        self.active = Some(Active { id, watchdog });
        if self.jobs.send(Job::Search { id, req: request }).is_err() && !self.dead {
            self.fail_now(
                Some(id),
                Failure::new(FailureKind::Crashed, "a Engine não está mais em execução"),
            );
        }
        id
    }

    /// Abandona a busca atual: a Engine recebe `stop` e a resposta é descartada.
    pub fn cancel(&mut self) {
        self.current.store(0, Ordering::SeqCst);
        self.active = None;
    }

    /// Há busca pedida e ainda sem resposta?
    pub fn is_searching(&self) -> bool {
        self.active.is_some()
    }

    /// A Engine já completou o handshake?
    pub fn is_ready(&self) -> bool {
        self.ready
    }

    /// A Engine falhou e o controlador não aceita mais pedidos.
    pub fn is_dead(&self) -> bool {
        self.dead
    }

    /// Próximo evento, sem bloquear. Respostas de buscas canceladas/substituídas são
    /// descartadas aqui.
    pub fn poll(&mut self) -> Option<ControllerEvent> {
        loop {
            let event = match self.pending.pop_front() {
                Some(e) => e,
                None => match self
                    .events
                    .get_mut()
                    .unwrap_or_else(PoisonError::into_inner)
                    .try_recv()
                {
                    Ok(e) => e,
                    Err(mpsc::TryRecvError::Empty) => return self.watchdog(),
                    Err(mpsc::TryRecvError::Disconnected) => {
                        if self.dead {
                            return None;
                        }
                        self.dead = true;
                        let id = self.active.take().map(|a| a.id);
                        return Some(ControllerEvent::Failed {
                            id,
                            failure: Failure::new(
                                FailureKind::Crashed,
                                "o worker da Engine terminou inesperadamente",
                            ),
                        });
                    }
                },
            };
            match &event {
                ControllerEvent::Ready { .. } => self.ready = true,
                ControllerEvent::Move { id, .. } | ControllerEvent::NoMove { id } => {
                    if self.active.as_ref().map(|a| a.id) != Some(*id) {
                        continue;
                    }
                    self.active = None;
                }
                ControllerEvent::Failed { id, .. } => {
                    if self.dead {
                        continue;
                    }
                    if let Some(id) = id
                        && self.active.as_ref().map(|a| a.id) != Some(*id)
                    {
                        // Falha de uma busca já abandonada: a Engine morreu mesmo assim.
                        self.dead = true;
                        continue;
                    }
                    self.dead = true;
                    self.active = None;
                }
            }
            return Some(event);
        }
    }

    fn watchdog(&mut self) -> Option<ControllerEvent> {
        let overdue = self
            .active
            .as_ref()
            .is_some_and(|a| Instant::now() >= a.watchdog);
        if !overdue || self.dead {
            return None;
        }
        let id = self.active.take().map(|a| a.id);
        self.dead = true;
        // Abandona o worker: ele encerra a Engine quando destravar e vir o canal fechado.
        let (dead_tx, _) = mpsc::channel();
        let _ = std::mem::replace(&mut self.jobs, dead_tx).send(Job::Shutdown);
        Some(ControllerEvent::Failed {
            id,
            failure: Failure::new(
                FailureKind::Unresponsive,
                "a Engine parou de responder e foi abandonada",
            ),
        })
    }

    fn fail_now(&mut self, id: Option<u64>, failure: Failure) {
        self.active = None;
        self.pending
            .push_back(ControllerEvent::Failed { id, failure });
    }
}

impl Drop for EngineController {
    fn drop(&mut self) {
        self.current.store(0, Ordering::SeqCst);
        let _ = self.jobs.send(Job::Shutdown);
    }
}

// --- worker ------------------------------------------------------------------

fn worker(
    config: EngineConfig,
    strength: Strength,
    jobs: &Receiver<Job>,
    out: &Sender<ControllerEvent>,
    current: &AtomicU64,
) {
    let timeouts = config.timeouts;
    let fail = |e: &EngineError| {
        let _ = out.send(ControllerEvent::Failed {
            id: None,
            failure: Failure::from(e),
        });
    };
    let mut engine = match Engine::spawn(config).and_then(|mut e| e.new_game().map(|()| e)) {
        Ok(e) => e,
        Err(e) => return fail(&e),
    };
    let applied = match apply_strength(&mut engine, strength) {
        Ok(a) => a,
        Err(e) => return fail(&e),
    };
    let _ = out.send(ControllerEvent::Ready {
        name: engine.info().name.clone(),
        strength: applied,
    });
    while let Ok(Job::Search { id, req }) = jobs.recv() {
        // Pedido já substituído/cancelado antes de começar: nem envia à Engine.
        if current.load(Ordering::SeqCst) != id {
            continue;
        }
        match run_search(&mut engine, id, &req, current, timeouts) {
            Ok(BestMove::Move { text, .. }) => {
                let _ = out.send(ControllerEvent::Move { id, uci: text });
            }
            Ok(BestMove::None) => {
                let _ = out.send(ControllerEvent::NoMove { id });
            }
            Err(e) => {
                let _ = out.send(ControllerEvent::Failed {
                    id: Some(id),
                    failure: Failure::from(&e),
                });
                return; // `engine` cai aqui e o processo é encerrado
            }
        }
    }
    let _ = engine.quit();
}

fn run_search(
    engine: &mut Engine,
    id: u64,
    req: &SearchRequest,
    current: &AtomicU64,
    timeouts: Timeouts,
) -> Result<BestMove, EngineError> {
    engine.set_position(&req.position)?;
    engine.go(&req.go)?;
    let soft = Instant::now() + req.limit + timeouts.search_slack;
    let mut stop_deadline: Option<Instant> = None;
    loop {
        if stop_deadline.is_none()
            && (current.load(Ordering::SeqCst) != id || Instant::now() >= soft)
        {
            engine.stop()?;
            stop_deadline = Some(Instant::now() + timeouts.stop_grace);
        }
        match engine.wait_bestmove(SLICE) {
            Ok(best) => return Ok(best),
            Err(EngineError::Timeout { killed: false, .. }) => {
                if let Some(deadline) = stop_deadline
                    && Instant::now() >= deadline
                {
                    return Err(EngineError::Timeout {
                        stage: Stage::Stop,
                        after: req.limit + timeouts.search_slack + timeouts.stop_grace,
                        killed: true,
                    });
                }
            }
            Err(e) => return Err(e),
        }
    }
}

// --- força ---------------------------------------------------------------------

/// `min`/`max` de uma opção `spin` (`default 1320 min 1320 max 3190`).
fn spin_range(details: &str) -> Option<(i64, i64)> {
    let mut min = None;
    let mut max = None;
    let mut words = details.split_whitespace();
    while let Some(w) = words.next() {
        match w {
            "min" => min = words.next().and_then(|v| v.parse().ok()),
            "max" => max = words.next().and_then(|v| v.parse().ok()),
            _ => {}
        }
    }
    Some((min?, max?))
}

/// Piso e teto do Elo mapeados em `Skill Level` quando a Engine não anuncia Elo.
const SKILL_ELO_MIN: u32 = 800;
const SKILL_ELO_MAX: u32 = 2800;

fn skill_for_elo(elo: u32, max_level: u32) -> u32 {
    let span = SKILL_ELO_MAX - SKILL_ELO_MIN;
    let clamped = elo.clamp(SKILL_ELO_MIN, SKILL_ELO_MAX) - SKILL_ELO_MIN;
    (clamped * max_level + span / 2) / span
}

fn apply_strength(engine: &mut Engine, strength: Strength) -> Result<StrengthApplied, EngineError> {
    let info: &EngineInfo = engine.info();
    let find = |name: &str| info.options.iter().find(|o| o.name == name).cloned();
    let (limit, elo_opt, skill) = (
        find("UCI_LimitStrength"),
        find("UCI_Elo"),
        find("Skill Level"),
    );
    match strength {
        Strength::Full => {
            if limit.is_some() {
                engine.set_option("UCI_LimitStrength", Some("false"))?;
            }
            Ok(StrengthApplied::Full)
        }
        Strength::Elo(requested) => {
            if let (Some(_), Some(elo)) = (&limit, &elo_opt) {
                let used = match spin_range(&elo.details) {
                    Some((lo, hi)) => i64::from(requested).clamp(lo, hi),
                    None => i64::from(requested),
                };
                engine.set_option("UCI_LimitStrength", Some("true"))?;
                engine.set_option("UCI_Elo", Some(&used.to_string()))?;
                return Ok(StrengthApplied::Elo {
                    requested,
                    used: u32::try_from(used).unwrap_or(requested),
                });
            }
            if let Some(skill) = skill {
                let max = spin_range(&skill.details)
                    .and_then(|(_, hi)| u32::try_from(hi).ok())
                    .unwrap_or(20);
                let level = skill_for_elo(requested, max);
                engine.set_option("Skill Level", Some(&level.to_string()))?;
                return Ok(StrengthApplied::SkillLevel { requested, level });
            }
            Ok(StrengthApplied::Unsupported)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn spin_range_reads_min_and_max() {
        assert_eq!(
            spin_range("default 1320 min 1320 max 3190"),
            Some((1320, 3190))
        );
        assert_eq!(spin_range("default false"), None);
    }

    #[test]
    fn skill_maps_elo_linearly_and_clamps() {
        assert_eq!(skill_for_elo(100, 20), 0);
        assert_eq!(skill_for_elo(800, 20), 0);
        assert_eq!(skill_for_elo(1800, 20), 10);
        assert_eq!(skill_for_elo(2800, 20), 20);
        assert_eq!(skill_for_elo(3500, 20), 20);
    }
}
