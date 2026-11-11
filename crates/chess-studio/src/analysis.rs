//! Painel Análise (C8, US-17/18, UI-08): avaliação contínua da Engine sobre a Posição em
//! estudo. Roda numa **segunda** Engine (`chess_uci::AnalysisController`, outro processo e
//! outra thread), nunca na que joga a Partida, então não toca relógio nem Lance adversário.
//!
//! - **Leitura apenas.** A análise segue a Posição mostrada (revisão ou Partida ao vivo) e
//!   clicar num Lance da linha só mostra a variante no tabuleiro (`Session::show_variation`):
//!   nenhum Comando e nenhum Evento (ADR-0001). "Jogada ruim" não é "jogada ilegal": a
//!   avaliação nunca impede um Lance.
//! - **Bloqueio competitivo (UI-08).** Com Partida competitiva em andamento o painel fica
//!   inacessível (o dock mostra o aviso) e `Analysis::drive` desliga e descarta tudo; depois
//!   do resultado a análise é liberada para revisão, mas volta desligada.
//! - **Carga.** Threads e Hash dentro dos limites do gerenciador de Engines (C9); a análise
//!   pode pausar sozinha enquanto a Engine adversária pensa (Assistido contra a Engine).
//! - **Controle de carga (C10, UCI-06).** O governador (`chess_uci::Governor`) lê o p95 do
//!   tempo de quadro e reduz a análise quando o FPS cai abaixo da meta da qualidade gráfica
//!   (threads, lote e cache da rede, rajadas, pausa); o painel avisa "Análise reduzida para
//!   manter o FPS" e o usuário escolhe Máximo, Equilibrado ou Leve.
//! - **Backend (Lc0).** Automático pelo que a Engine anuncia e pela máquina, ou escolhido à
//!   mão; se a GPU não sobe, a análise recua para a CPU e o painel diz por quê.

use std::path::PathBuf;
use std::time::{Duration, Instant};

use bevy::prelude::*;
use bevy_egui::egui;
use chess_core::{CanonicalPosition, Color as Side};
use chess_game::{HistoryEntry, ReviewView};
use chess_uci::{
    AnalysisConfig, AnalysisEvent, AnalysisLine, AnalysisUpdate, EngineConfig, Hardware, Limits,
    LoadMode, OptionGroup, PositionSpec, Score,
};

use crate::dock::{PanelAccess, PanelDef, ids};
use crate::engine_manager::{DbLocation, EngineManager};
use crate::fair_play::{FairPlay, GuardedAnalysis, Subject};
use crate::i18n::I18n;
use crate::load_control::{LoadControl, Notice, target_for};
use crate::notation;
use crate::presentation::GraphicsQuality;
use crate::session::Session;
use crate::training_ui::TrainingUi;

/// Intervalo entre releituras da lista de Engines do gerenciador.
const CHOICES_REFRESH: Duration = Duration::from_secs(2);
/// Hash padrão da análise (MB).
const DEFAULT_HASH_MB: u32 = 128;

/// A GPU da máquina, detectada uma vez (chama `nvidia-smi -L`, barato mas não de graça).
pub fn host_hardware() -> Hardware {
    static HARDWARE: std::sync::OnceLock<Hardware> = std::sync::OnceLock::new();
    *HARDWARE.get_or_init(chess_uci::detect_hardware)
}

// --- política: quando a análise pode rodar -----------------------------------------------

/// Por que a análise está travada.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Lock {
    /// Partida competitiva sem resultado (UI-08).
    Competitive,
    /// Exercício de Treino na tela.
    Training,
    /// Trava de fair play: Partida online contra pessoa em andamento (ADR-0007, PO-13).
    FairPlay,
}

#[cfg(test)]
pub fn lock_reason(competitive_in_progress: bool, training: bool) -> Option<Lock> {
    lock_with(competitive_in_progress, false, training)
}

/// O bloqueio da análise, da Política local, da Trava de fair play e do Treino, nessa ordem.
pub fn lock_with(competitive_in_progress: bool, fair_play: bool, training: bool) -> Option<Lock> {
    if competitive_in_progress {
        Some(Lock::Competitive)
    } else if fair_play {
        Some(Lock::FairPlay)
    } else if training {
        Some(Lock::Training)
    } else {
        None
    }
}

/// O que a análise deve fazer agora.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Gate {
    Locked(Lock),
    Off,
    /// Ligada, mas em espera porque a Engine adversária está pensando.
    Paused,
    Run,
}

pub fn gate(
    lock: Option<Lock>,
    enabled: bool,
    pause_option: bool,
    opponent_thinking: bool,
) -> Gate {
    match lock {
        Some(lock) => Gate::Locked(lock),
        None if !enabled => Gate::Off,
        None if pause_option && opponent_thinking => Gate::Paused,
        None => Gate::Run,
    }
}

// --- apresentação (funções puras) --------------------------------------------------------

/// Fração da barra ocupada pelas brancas (0 a 1): curva logística em centipeões.
pub fn white_share(score: Score) -> f32 {
    match score {
        Score::Cp(cp) => 1.0 / (1.0 + (-(cp as f32) / 400.0).exp()),
        Score::Mate(n) if n > 0 => 1.0,
        Score::Mate(_) => 0.0,
    }
}

/// Texto curto do placar: `+0.35`, `-1.20`, `M3` (brancas dão mate), `-M3` (pretas dão).
pub fn format_score(score: Score) -> String {
    match score {
        Score::Cp(cp) => format!("{:+.2}", f64::from(cp) / 100.0),
        Score::Mate(n) if n >= 0 => format!("M{n}"),
        Score::Mate(n) => format!("-M{}", n.unsigned_abs()),
    }
}

/// `1234` -> `1,2k`; `5_300_000` -> `5,3M`.
pub fn compact(n: u64) -> String {
    #[allow(clippy::cast_precision_loss)]
    let f = n as f64;
    if n >= 1_000_000_000 {
        format!("{:.1}G", f / 1e9)
    } else if n >= 1_000_000 {
        format!("{:.1}M", f / 1e6)
    } else if n >= 1_000 {
        format!("{:.1}k", f / 1e3)
    } else {
        n.to_string()
    }
    .replace('.', ",")
}

/// Um Lance da variante na tela: número do lance (quando cabe) e índice em `pv`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MoveToken {
    pub number: Option<String>,
    pub index: usize,
}

/// Numera a variante: `12.` antes de um Lance das brancas e `12...` no início, se começa
/// pelas pretas.
pub fn line_tokens(first_number: u16, white_to_move: bool, count: usize) -> Vec<MoveToken> {
    let mut number = first_number;
    let mut white = white_to_move;
    (0..count)
        .map(|index| {
            let label = if white {
                Some(format!("{number}."))
            } else if index == 0 {
                Some(format!("{number}..."))
            } else {
                None
            };
            if !white {
                number = number.saturating_add(1);
            }
            white = !white;
            MoveToken {
                number: label,
                index,
            }
        })
        .collect()
}

/// A Posição depois dos `k + 1` primeiros Lances da linha, como revisão somente leitura.
/// `base_ply` é o meio-lance da Posição analisada.
pub fn variation_view(
    base: &CanonicalPosition,
    base_ply: usize,
    total_plies: usize,
    line: &AnalysisLine,
    k: usize,
) -> Option<ReviewView> {
    let mut position = base.clone();
    let mut last = None;
    for index in 0..=k {
        let mv = *line.pv.get(index)?;
        let side = position.side_to_move();
        let san = line.san.get(index)?.clone();
        position = position.play(mv).ok()?;
        last = Some(HistoryEntry {
            ply: base_ply + index + 1,
            side,
            mv,
            san,
        });
    }
    Some(ReviewView {
        ply: base_ply,
        total_plies,
        side_to_move: position.side_to_move(),
        in_check: position.is_check(),
        position,
        last_move: last,
        is_start: false,
        is_end: false,
    })
}

// --- estado ------------------------------------------------------------------------------

/// Uma Engine do gerenciador que a análise pode usar.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Choice {
    pub name: String,
    pub path: PathBuf,
    /// Opções avançadas já validadas pelo gerenciador. Força (Elo, nível) e recursos
    /// (Threads, Hash) ficam de fora: a análise joga sempre com força total e tem os
    /// próprios limites.
    pub options: Vec<(String, String)>,
}

/// O que identifica a Engine de análise em execução: mudou, reinicia.
#[derive(Clone, Debug, PartialEq, Eq)]
struct Launch {
    path: PathBuf,
    options: Vec<(String, String)>,
    threads: u32,
    hash_mb: u32,
    /// Backend escolhido à mão (Lc0); `None` = automático.
    backend: Option<String>,
}

/// Qual Posição está sendo analisada.
#[derive(Clone, Debug, PartialEq, Eq)]
struct TargetKey {
    epoch: u64,
    ply: usize,
    total: usize,
    last: Option<chess_core::Move>,
    multipv: u8,
}

#[derive(Debug)]
struct Target {
    key: TargetKey,
    base: CanonicalPosition,
    ply: usize,
    total: usize,
}

/// A última atualização, com o contexto para exibi-la.
#[derive(Debug, Clone)]
pub struct Shown {
    pub update: AnalysisUpdate,
    pub base: CanonicalPosition,
    pub base_ply: usize,
    pub total: usize,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Status {
    Off,
    Locked(Lock),
    Starting,
    Running,
    Paused,
    NoMoves,
    NoEngine,
    Failed(String),
}

#[derive(Resource, Debug)]
pub struct Analysis {
    /// O usuário ligou a análise.
    enabled: bool,
    multipv: u8,
    /// Engine escolhida entre as do gerenciador; `None` = a padrão do app.
    engine: Option<PathBuf>,
    /// Caminho liberado pelo consentimento (`engine_consent`); nunca o do ambiente cru.
    default_engine: Option<PathBuf>,
    /// Valores confirmados (a Engine só reinicia ao soltar o controle deslizante).
    threads: u32,
    hash_mb: u32,
    pause_while_thinking: bool,
    choices: Vec<Choice>,
    /// A lista foi fixada de fora (benchmark): o gerenciador não a substitui.
    choices_pinned: bool,
    choices_at: Option<Instant>,
    /// A Engine de análise, sempre atrás da Trava de fair play.
    controller: Option<GuardedAnalysis>,
    launched: Option<Launch>,
    failure: Option<String>,
    ready: bool,
    applied: (Option<u32>, Option<u32>),
    engine_name: Option<String>,
    target: Option<Target>,
    request: u64,
    view: Option<Shown>,
    status: Status,
    /// Variante em exibição: (índice da linha, índice do Lance).
    preview: Option<(usize, usize)>,
    limits: Limits,
    interval: Duration,
    /// Governador de carga por FPS (C10).
    load: LoadControl,
    /// Backend do Lc0 escolhido pelo usuário; `None` = automático.
    backend: Option<String>,
    /// Backends que a Engine em uso anuncia (vazio: não tem a opção).
    backends: Vec<String>,
    /// Backend em uso agora.
    backend_active: Option<String>,
    /// Aviso de que a GPU não subiu e a análise recuou para a CPU.
    backend_note: Option<String>,
}

impl Default for Analysis {
    fn default() -> Self {
        let limits = Limits::for_host();
        Self {
            enabled: false,
            multipv: 1,
            engine: None,
            default_engine: None,
            threads: (limits.max_threads / 2).clamp(1, 4),
            hash_mb: DEFAULT_HASH_MB,
            pause_while_thinking: true,
            choices: Vec::new(),
            choices_pinned: false,
            choices_at: None,
            controller: None,
            launched: None,
            failure: None,
            ready: false,
            applied: (None, None),
            engine_name: None,
            target: None,
            request: 0,
            view: None,
            status: Status::Off,
            preview: None,
            load: LoadControl::new(
                LoadMode::default(),
                target_for(GraphicsQuality::default()),
                (limits.max_threads / 2).clamp(1, 4),
            ),
            limits,
            interval: chess_uci::DEFAULT_INTERVAL,
            backend: None,
            backends: Vec::new(),
            backend_active: None,
            backend_note: None,
        }
    }
}

impl Analysis {
    pub fn is_enabled(&self) -> bool {
        self.enabled
    }

    pub fn status(&self) -> &Status {
        &self.status
    }

    pub fn shown(&self) -> Option<&Shown> {
        self.view.as_ref()
    }

    pub fn load_mode(&self) -> LoadMode {
        self.load.mode()
    }

    /// Modo de carga escolhido pelo usuário (Máximo, Equilibrado ou Leve).
    pub fn set_load_mode(&mut self, mode: LoadMode) {
        self.load.set_mode(mode);
    }

    /// Aviso de carga a mostrar no painel.
    pub fn load_notice(&self) -> Option<Notice> {
        self.load.notice()
    }

    #[cfg(test)]
    pub fn load_level(&self) -> chess_uci::Level {
        self.load.level()
    }

    pub fn backend_active(&self) -> Option<&str> {
        self.backend_active.as_deref()
    }

    pub fn backend_note(&self) -> Option<&str> {
        self.backend_note.as_deref()
    }

    /// Escolhe o backend do Lc0 (`None` = automático); a Engine reinicia.
    pub fn set_backend(&mut self, backend: Option<String>) {
        self.backend = backend;
    }

    /// Um quadro de `frame_ms`: alimenta o governador enquanto a análise roda e entrega à
    /// Engine o plano de carga novo. Chamado uma vez por quadro, antes de [`Self::drive`].
    pub fn frame(&mut self, frame_ms: f64, quality: GraphicsQuality) {
        self.load.set_target(target_for(quality));
        self.load.set_threads(self.threads);
        let running = self.controller.is_some()
            && self.ready
            && matches!(self.status, Status::Starting | Status::Running);
        self.load.frame(frame_ms, running);
        if running
            && let Some(plan) = self.load.take_plan_change()
            && let Some(controller) = &mut self.controller
        {
            controller.set_load(plan);
        }
    }

    /// Ligada e com Engine de análise de pé (usado pelo benchmark).
    pub fn is_active(&self) -> bool {
        self.enabled && self.controller.is_some()
    }

    /// Nível do governador como número (0 = pleno … 4 = pausado), para o benchmark.
    pub fn load_level_index(&self) -> u8 {
        match self.load.level() {
            chess_uci::Level::Full => 0,
            chess_uci::Level::Reduced => 1,
            chess_uci::Level::Minimal => 2,
            chess_uci::Level::Intermittent => 3,
            chess_uci::Level::Paused => 4,
        }
    }

    /// Profundidade e nós/s da melhor linha mostrada agora.
    pub fn latest_metrics(&self) -> Option<(u32, Option<u64>)> {
        let line = self.view.as_ref()?.update.lines.first()?;
        Some((line.depth, line.nps))
    }

    /// Benchmark (`--engine ativa`): usa esta Engine, com estas opções, no modo de carga
    /// pedido. A lista do gerenciador deixa de sobrescrever a escolha.
    pub fn bench_use(&mut self, path: PathBuf, options: Vec<(String, String)>, mode: LoadMode) {
        self.choices = vec![Choice {
            name: path.display().to_string(),
            path: path.clone(),
            options,
        }];
        self.choices_pinned = true;
        self.engine = Some(path);
        self.load.set_mode(mode);
        self.failure = None;
    }

    /// Há uma Engine liberada (pelo consentimento ou escolhida no gerenciador) para analisar.
    pub fn engine_available(&self) -> bool {
        self.default_engine.is_some() || self.engine.is_some()
    }

    /// Nome da Engine para a barra de status: o que ela anunciou ou o nome do arquivo.
    pub fn engine_label(&self) -> Option<String> {
        self.engine_name.clone().or_else(|| {
            let path = self.engine.as_ref().or(self.default_engine.as_ref())?;
            Some(path.file_stem()?.to_string_lossy().into_owned())
        })
    }

    /// Há uma Engine de análise em execução (processo vivo ou subindo)?
    pub fn has_engine(&self) -> bool {
        self.controller.is_some()
    }

    pub fn set_enabled(&mut self, on: bool) {
        self.enabled = on;
        if on {
            self.failure = None;
        }
    }

    pub fn set_multipv(&mut self, lines: u8) {
        self.multipv = lines.clamp(1, chess_uci::MAX_MULTIPV);
    }

    /// Threads da análise, até o limite do app.
    pub fn set_threads(&mut self, threads: u32) {
        self.threads = threads.clamp(1, self.limits.max_threads);
    }

    pub fn set_hash_mb(&mut self, hash_mb: u32) {
        self.hash_mb = hash_mb.clamp(1, self.limits.max_hash_mb);
    }

    #[cfg(test)]
    pub fn set_limits(&mut self, limits: Limits) {
        self.limits = limits;
        self.set_threads(self.threads);
        self.set_hash_mb(self.hash_mb);
    }

    /// Escolhe a Engine de análise (`None` = a padrão).
    pub fn set_engine(&mut self, path: Option<PathBuf>) {
        self.engine = path;
    }

    /// Engine padrão da análise: o caminho já liberado pelo consentimento, se houver.
    pub fn set_default_engine(&mut self, path: Option<PathBuf>) {
        self.default_engine = path;
    }

    pub fn set_choices(&mut self, choices: Vec<Choice>) {
        self.choices = choices;
    }

    pub fn set_pause_while_thinking(&mut self, on: bool) {
        self.pause_while_thinking = on;
    }

    fn launch(&self) -> Option<Launch> {
        let registered: Vec<PathBuf> = self.choices.iter().map(|c| c.path.clone()).collect();
        let path = crate::engine_consent::analysis_engine(
            self.engine.as_deref(),
            self.default_engine.as_deref(),
            &registered,
        )?;
        let options = self
            .choices
            .iter()
            .find(|c| c.path == path)
            .map(|c| c.options.clone())
            .unwrap_or_default();
        Some(Launch {
            path,
            options,
            threads: self.threads,
            hash_mb: self.hash_mb,
            backend: self.backend.clone(),
        })
    }

    fn shutdown(&mut self) {
        self.controller = None;
        self.launched = None;
        self.ready = false;
        self.target = None;
        self.view = None;
        self.preview = None;
        self.applied = (None, None);
        self.backend_active = None;
    }

    fn start(&mut self, launch: &Launch, fair: &FairPlay, subject: &Subject) {
        self.shutdown();
        let engine = launch
            .options
            .iter()
            .fold(EngineConfig::new(&launch.path), |c, (name, value)| {
                c.option(name, value)
            });
        let mut config = AnalysisConfig::new(engine);
        config.threads = Some(launch.threads);
        config.hash_mb = Some(launch.hash_mb);
        config.limits = self.limits;
        config.interval = self.interval;
        config.backend.clone_from(&launch.backend);
        config.hardware = Some(host_hardware());
        self.backend_note = None;
        self.load.mark_unsent();
        match GuardedAnalysis::start(fair, subject, config) {
            Ok(controller) => {
                self.controller = Some(controller);
                self.launched = Some(launch.clone());
            }
            Err(_) => self.lock_now(Lock::FairPlay),
        }
    }

    fn poll(&mut self) {
        let Some(controller) = &mut self.controller else {
            return;
        };
        let mut failed = None;
        while let Some(event) = controller.poll() {
            match event {
                AnalysisEvent::Ready {
                    name,
                    threads,
                    hash_mb,
                    backend,
                    backends,
                } => {
                    self.ready = true;
                    self.engine_name = name;
                    self.applied = (threads, hash_mb);
                    self.backend_active = backend;
                    self.backends = backends;
                }
                AnalysisEvent::BackendFallback { message, .. } => {
                    self.backend_note = Some(message);
                }
                AnalysisEvent::Update(update) => {
                    if update.id == self.request
                        && let Some(target) = &self.target
                    {
                        self.view = Some(Shown {
                            update,
                            base: target.base.clone(),
                            base_ply: target.ply,
                            total: target.total,
                        });
                    }
                }
                AnalysisEvent::Failed(failure) => failed = Some(failure.message),
            }
        }
        if let Some(message) = failed {
            self.shutdown();
            self.failure = Some(message);
        }
    }

    /// Bloqueio: a análise some por completo e só volta quando o usuário a ligar de novo.
    fn lock_now(&mut self, lock: Lock) {
        self.enabled = false;
        self.shutdown();
        self.failure = None;
        self.status = Status::Locked(lock);
    }

    /// Passo por quadro: aplica a política (bloqueio, Trava de fair play, pausa) e mantém a
    /// Engine de análise e a Posição analisada em dia. Nunca bloqueia.
    pub fn drive(&mut self, session: &Session, training: bool, fair: &FairPlay) {
        let subject = Subject::at(session.game(), notation::current_ply(session));
        let refused = subject.as_ref().map_or(fair.any(), |s| {
            fair.check(crate::fair_play::Computation::EngineAnalysis, s)
                .is_err()
        });
        let lock = lock_with(session.competitive_in_progress(), refused, training);
        match gate(
            lock,
            self.enabled,
            self.pause_while_thinking,
            session.engine_turn(),
        ) {
            Gate::Locked(lock) => {
                // UI-08 e ADR-0007: durante a Partida competitiva (ou online contra pessoa) a
                // análise some por completo e só volta quando o usuário a ligar de novo.
                self.lock_now(lock);
                return;
            }
            Gate::Off => {
                self.shutdown();
                self.status = Status::Off;
                return;
            }
            Gate::Paused => {
                if let Some(controller) = &mut self.controller {
                    controller.stop();
                }
                self.poll();
                self.target = None;
                self.view = None;
                self.status = Status::Paused;
                return;
            }
            Gate::Run => {}
        }
        if let Some(message) = &self.failure {
            self.status = Status::Failed(message.clone());
            return;
        }
        let Some(launch) = self.launch() else {
            self.status = Status::NoEngine;
            return;
        };
        let Some(subject) = subject else {
            return;
        };
        if self.controller.is_none() || self.launched.as_ref() != Some(&launch) {
            self.start(&launch, fair, &subject);
            if self.controller.is_none() {
                return;
            }
        }
        self.poll();
        if let Some(message) = &self.failure {
            self.status = Status::Failed(message.clone());
            return;
        }
        self.follow_position(session, fair, &subject);
    }

    /// Envia a Posição mostrada à Engine quando ela muda.
    fn follow_position(&mut self, session: &Session, fair: &FairPlay, subject: &Subject) {
        let ply = notation::current_ply(session);
        let snapshot = session.snapshot();
        let key = TargetKey {
            epoch: session.epoch(),
            ply,
            total: snapshot.history.len(),
            last: ply
                .checked_sub(1)
                .and_then(|i| snapshot.history.get(i))
                .map(|h| h.mv),
            multipv: self.multipv,
        };
        if self.target.as_ref().map(|t| &t.key) != Some(&key) {
            self.view = None;
            self.preview = None;
            let Some(view) = session.game().review_at_ply(ply) else {
                self.target = None;
                return;
            };
            let start = session.game().review_at_ply(0).map(|v| v.position);
            let spec = start.map(|start| {
                PositionSpec::from_fen(start.to_fen())
                    .with_moves(snapshot.history.iter().take(ply).map(|h| h.mv))
            });
            let base = view.position;
            let total = snapshot.history.len();
            let has_moves = !base.legal_moves().is_empty();
            self.target = Some(Target {
                key,
                base,
                ply,
                total,
            });
            if let (true, Some(spec), Some(controller)) = (has_moves, spec, &mut self.controller) {
                match controller.analyze(fair, subject, spec, self.multipv) {
                    Ok(request) => self.request = request,
                    Err(_) => return self.lock_now(Lock::FairPlay),
                }
            } else if let Some(controller) = &mut self.controller {
                controller.stop();
                self.request = 0;
            }
        }
        let no_moves = self
            .target
            .as_ref()
            .is_some_and(|t| t.base.legal_moves().is_empty());
        self.status = if no_moves {
            Status::NoMoves
        } else if !self.ready {
            Status::Starting
        } else if self.view.is_some() {
            Status::Running
        } else {
            Status::Starting
        };
    }
}

/// Melhor Lance (MultiPV 1) da análise exibida, para a seta da Engine do HUD. Vedado pela
/// política (`engine_line`) nas competitivas em andamento; o HUD ainda confere que o FEN
/// bate com a posição mostrada.
pub fn hud_best(
    shown: Option<&Shown>,
    allowed: &crate::hud_policy::Allowance,
) -> Option<crate::hud::AnalysisBest> {
    if !allowed.engine_line {
        return None;
    }
    let shown = shown?;
    let line = shown.update.lines.iter().find(|l| l.multipv == 1)?;
    let mv = line.pv.first()?;
    Some(crate::hud::AnalysisBest {
        fen: shown.base.to_fen(),
        from: mv.from(),
        to: mv.to(),
    })
}

/// Copia o melhor Lance da análise para o HUD.
fn sync_hud_analysis(
    session: Res<Session>,
    analysis: Res<Analysis>,
    mut hud: ResMut<crate::hud::HudAnalysis>,
) {
    let allowed = crate::hud_policy::allowance(&session);
    let best = hud_best(analysis.shown(), &allowed);
    if hud.best != best {
        hud.best = best;
    }
}

/// Engines do gerenciador utilizáveis na análise.
fn choices_from(manager: &EngineManager) -> Vec<Choice> {
    let limits = Limits::for_host();
    let dir = chess_uci::engines_dir();
    manager
        .rows()
        .iter()
        .filter(|r| r.exists)
        .map(|r| Choice {
            name: r.record.name.clone(),
            path: PathBuf::from(&r.record.path),
            options: chess_uci::launch_options(&r.record.settings, &limits, dir.as_deref())
                .into_iter()
                .filter(|(name, _)| chess_uci::group_of(name) == OptionGroup::Advanced)
                .collect(),
        })
        .collect()
}

// --- plugin e sistemas -------------------------------------------------------------------

pub struct AnalysisPlugin;

impl Plugin for AnalysisPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<Analysis>().add_systems(
            Update,
            (capture_setup, feed_load, drive_analysis, sync_hud_analysis)
                .chain()
                .before(crate::hud::HudSet),
        );
        crate::dock::register_panel(
            app,
            PanelDef {
                id: ids::ANALYSIS,
                label_key: "dock-panel-analysis",
                access: PanelAccess::OutsideCompetitive,
                ui: analysis_panel_ui,
            },
        );
    }
}

/// Todo quadro: o tempo do quadro alimenta o governador de carga (C10).
fn feed_load(
    time: Res<Time<Real>>,
    quality: Option<Res<GraphicsQuality>>,
    mut analysis: ResMut<Analysis>,
) {
    let quality = quality.map_or_else(GraphicsQuality::default, |q| *q);
    analysis.frame(time.delta_secs_f64() * 1000.0, quality);
}

/// Todo quadro: aplica o bloqueio competitivo e mantém a Engine de análise.
fn drive_analysis(
    session: Res<Session>,
    training: Option<Res<TrainingUi>>,
    manager: Option<Res<EngineManager>>,
    engine_ui: Option<Res<crate::engine::EngineUi>>,
    fair: Res<FairPlay>,
    mut analysis: ResMut<Analysis>,
) {
    if let Some(ui) = engine_ui {
        let cleared = ui.cleared_path();
        if cleared != analysis.default_engine {
            analysis.set_default_engine(cleared);
        }
    }
    if let Some(manager) = manager
        && !analysis.choices_pinned
        && analysis
            .choices_at
            .is_none_or(|t| t.elapsed() >= CHOICES_REFRESH)
    {
        analysis.choices_at = Some(Instant::now());
        let choices = choices_from(&manager);
        if choices != analysis.choices {
            analysis.set_choices(choices);
        }
    }
    let training = training.is_some_and(|t| t.session_swapped());
    analysis.drive(&session, training, &fair);
}

/// Evidência (`--screenshot … --panel --tabs analysis --analysis [--multipv 3]`): liga a
/// análise uma vez, para a captura mostrar o painel funcionando.
fn capture_setup(
    capture: Option<Res<crate::capture::CaptureMode>>,
    mut analysis: ResMut<Analysis>,
    mut done: Local<bool>,
) {
    if capture.is_none() || *done {
        return;
    }
    *done = true;
    if std::env::args().any(|a| a == "--analysis") {
        analysis.set_enabled(true);
        if let Some(lines) = crate::capture::option_value("--multipv").and_then(|v| v.parse().ok())
        {
            analysis.set_multipv(lines);
        }
    }
}

// --- painel ------------------------------------------------------------------------------

fn analysis_panel_ui(ui: &mut egui::Ui, i18n: &I18n, world: &mut World) {
    // Linhas da Engine em SAN são resposta pronta: ocultas com a Súmula manual aberta.
    if world
        .get_resource::<crate::scoresheet::ScoresheetUi>()
        .is_some_and(crate::scoresheet::ScoresheetUi::hides_notation)
    {
        crate::scoresheet::hidden_notice(ui, i18n);
        return;
    }
    let db = world.get_resource::<DbLocation>().map(|d| d.0.clone());
    if let Some(mut manager) = world.get_resource_mut::<EngineManager>() {
        manager.ensure_loaded(db);
    }
    let mut cloud_actions = Vec::new();
    world.resource_scope(|world, mut session: Mut<Session>| {
        // A avaliação em cache do Lichess é consultada aqui, mas desenhada em linha própria
        // (nunca misturada à Engine local): ver `explorer::cloud_prepare`.
        let cloud = crate::explorer::cloud_prepare(world, &session);
        world.resource_scope(|_, mut analysis: Mut<Analysis>| {
            let mut cloud_row = |ui: &mut egui::Ui| {
                crate::explorer::cloud_draw(ui, i18n, &cloud, &mut cloud_actions)
            };
            panel(ui, i18n, &mut analysis, &mut session, &mut cloud_row);
        });
    });
    crate::explorer::cloud_finish(world, i18n, &cloud_actions);
    if let Some(wait) = crate::explorer::cloud_wait(world) {
        ui.ctx().request_repaint_after(wait);
    }
}

fn panel(
    ui: &mut egui::Ui,
    i18n: &I18n,
    analysis: &mut Analysis,
    session: &mut Session,
    cloud_row: &mut dyn FnMut(&mut egui::Ui),
) {
    if let Status::Locked(Lock::Training) = analysis.status {
        ui.weak(i18n.t("analysis-locked-training"));
        return;
    }
    toggle_row(ui, i18n, analysis);
    load_notice_ui(ui, i18n, analysis);
    status_line(ui, i18n, analysis);
    if analysis.enabled
        && let Some(shown) = analysis.view.clone()
    {
        results(ui, i18n, analysis, session, &shown);
    }
    ui.add_space(8.0);
    cloud_row(ui);
    ui.add_space(4.0);
    ui.separator();
    settings(ui, i18n, analysis);
}

fn toggle_row(ui: &mut egui::Ui, i18n: &I18n, analysis: &mut Analysis) {
    ui.horizontal(|ui| {
        let key = if analysis.enabled {
            "analysis-toggle-off"
        } else {
            "analysis-toggle-on"
        };
        if ui.button(i18n.t(key)).clicked() {
            let on = !analysis.enabled;
            analysis.set_enabled(on);
        }
        if analysis.enabled
            && matches!(analysis.status, Status::Starting | Status::Running)
            && analysis.view.is_none()
        {
            ui.spinner();
        }
    });
}

/// Aviso de carga (C10) e, se a GPU não subiu, o recuo para a CPU.
fn load_notice_ui(ui: &mut egui::Ui, i18n: &I18n, analysis: &Analysis) {
    if !analysis.enabled {
        return;
    }
    let (key, color) = match analysis.load_notice() {
        Some(Notice::Reduced) => ("analysis-load-reduced", egui::Color32::YELLOW),
        Some(Notice::Paused) => ("analysis-load-paused", egui::Color32::YELLOW),
        Some(Notice::BelowTarget) => ("analysis-load-below-target", egui::Color32::YELLOW),
        None => ("", egui::Color32::YELLOW),
    };
    if !key.is_empty() {
        ui.colored_label(color, i18n.t(key));
    }
    if let Some(note) = analysis.backend_note() {
        ui.colored_label(egui::Color32::YELLOW, note);
    }
}

fn status_line(ui: &mut egui::Ui, i18n: &I18n, analysis: &mut Analysis) {
    match &analysis.status {
        Status::Off | Status::Locked(_) => {
            ui.weak(i18n.t("analysis-status-off"));
        }
        Status::Starting => {
            ui.weak(i18n.t(if analysis.ready {
                "analysis-status-thinking"
            } else {
                "analysis-status-starting"
            }));
        }
        Status::Running => {}
        Status::Paused => {
            ui.weak(i18n.t("analysis-status-paused"));
        }
        Status::NoMoves => {
            ui.weak(i18n.t("analysis-status-no-moves"));
        }
        Status::NoEngine => {
            ui.colored_label(egui::Color32::YELLOW, i18n.t("analysis-status-no-engine"));
        }
        Status::Failed(message) => {
            ui.colored_label(egui::Color32::LIGHT_RED, i18n.t("analysis-status-failed"));
            ui.small(message);
            if ui.button(i18n.t("analysis-retry")).clicked() {
                analysis.failure = None;
            }
        }
    }
}

fn results(
    ui: &mut egui::Ui,
    i18n: &I18n,
    analysis: &mut Analysis,
    session: &mut Session,
    shown: &Shown,
) {
    let lines = &shown.update.lines;
    let Some(best) = lines.first() else {
        return;
    };
    eval_bar(ui, i18n, best.score);
    let mut facts = match best.seldepth {
        Some(sel) => i18n.t_with(
            "analysis-depth",
            &[
                ("depth", &best.depth.to_string()),
                ("seldepth", &sel.to_string()),
            ],
        ),
        None => i18n.t_with(
            "analysis-depth-plain",
            &[("depth", &best.depth.to_string())],
        ),
    };
    if let (Some(nodes), Some(nps)) = (best.nodes, best.nps) {
        facts.push_str(" · ");
        facts.push_str(&i18n.t_with(
            "analysis-speed",
            &[("nodes", &compact(nodes)), ("nps", &compact(nps))],
        ));
    }
    ui.small(facts);
    if !session.showing_variation() {
        analysis.preview = None;
    }
    ui.add_space(4.0);
    let mut clicked = None;
    for (line_index, line) in lines.iter().enumerate() {
        principal_line(ui, analysis.preview, shown, line_index, line, &mut clicked);
    }
    if let Some((line_index, k)) = clicked
        && let Some(line) = lines.get(line_index)
        && let Some(view) = variation_view(&shown.base, shown.base_ply, shown.total, line, k)
    {
        session.show_variation(view);
        analysis.preview = Some((line_index, k));
    }
    if session.showing_variation() {
        ui.small(i18n.t("analysis-showing-line"));
        if ui.button(i18n.t("analysis-back-to-game")).clicked() {
            notation::navigate(session, shown.base_ply);
            analysis.preview = None;
        }
    } else {
        ui.weak(i18n.t("analysis-line-hint"));
    }
}

/// Barra de avaliação (brancas à esquerda) com o número sobre ela.
fn eval_bar(ui: &mut egui::Ui, i18n: &I18n, score: Score) {
    let height = 18.0;
    let (rect, response) = ui.allocate_exact_size(
        egui::vec2(ui.available_width().max(60.0), height),
        egui::Sense::hover(),
    );
    let painter = ui.painter_at(rect);
    painter.rect_filled(rect, 3.0, egui::Color32::from_gray(40));
    let split = rect.left() + rect.width() * white_share(score);
    painter.rect_filled(
        egui::Rect::from_min_max(rect.min, egui::pos2(split, rect.bottom())),
        3.0,
        egui::Color32::from_gray(235),
    );
    let text = format_score(score);
    // Cor do texto contrasta com o lado da barra em que ele cai (esquerda: claro).
    let white_side = white_share(score) > 0.5;
    let (anchor, align, color) = if white_side {
        (
            egui::pos2(rect.left() + 6.0, rect.center().y),
            egui::Align2::LEFT_CENTER,
            egui::Color32::from_gray(20),
        )
    } else {
        (
            egui::pos2(rect.right() - 6.0, rect.center().y),
            egui::Align2::RIGHT_CENTER,
            egui::Color32::from_gray(240),
        )
    };
    painter.text(anchor, align, text, egui::FontId::proportional(13.0), color);
    let hover = match score {
        Score::Mate(n) if n > 0 => i18n.t_with("analysis-mate-white", &[("n", &n.to_string())]),
        Score::Mate(n) => i18n.t_with(
            "analysis-mate-black",
            &[("n", &n.unsigned_abs().to_string())],
        ),
        Score::Cp(_) => i18n.t("analysis-white-pov"),
    };
    response.on_hover_text(hover);
}

fn principal_line(
    ui: &mut egui::Ui,
    preview: Option<(usize, usize)>,
    shown: &Shown,
    line_index: usize,
    line: &AnalysisLine,
    clicked: &mut Option<(usize, usize)>,
) {
    let white = shown.base.side_to_move() == Side::White;
    let tokens = line_tokens(shown.base.fullmove_number(), white, line.san.len());
    ui.horizontal_wrapped(|ui| {
        ui.spacing_mut().item_spacing.x = 4.0;
        ui.label(
            egui::RichText::new(format_score(line.score))
                .strong()
                .monospace(),
        );
        for token in tokens {
            if let Some(number) = &token.number {
                ui.weak(number);
            }
            let selected = preview == Some((line_index, token.index));
            let mut text = egui::RichText::new(&line.san[token.index]);
            if selected {
                text = text.strong().underline();
            }
            let response = ui
                .add(egui::Label::new(text).sense(egui::Sense::click()))
                .on_hover_cursor(egui::CursorIcon::PointingHand);
            if response.clicked() {
                *clicked = Some((line_index, token.index));
            }
        }
    });
}

fn settings(ui: &mut egui::Ui, i18n: &I18n, analysis: &mut Analysis) {
    engine_picker(ui, i18n, analysis);
    ui.horizontal(|ui| {
        ui.label(i18n.t("analysis-lines-label"));
        let mut lines = analysis.multipv;
        for n in 1..=chess_uci::MAX_MULTIPV {
            ui.selectable_value(&mut lines, n, n.to_string());
        }
        if lines != analysis.multipv {
            analysis.set_multipv(lines);
        }
    });
    let mut threads = analysis.threads;
    let response = ui.add(
        egui::Slider::new(&mut threads, 1..=analysis.limits.max_threads)
            .text(i18n.t("analysis-threads-label")),
    );
    if response.drag_stopped() || (response.changed() && !response.dragged()) {
        analysis.set_threads(threads);
    }
    let mut hash = analysis.hash_mb;
    let response = ui.add(
        egui::Slider::new(&mut hash, 16..=analysis.limits.max_hash_mb.min(4096))
            .logarithmic(true)
            .text(i18n.t("analysis-hash-label")),
    );
    if response.drag_stopped() || (response.changed() && !response.dragged()) {
        analysis.set_hash_mb(hash);
    }
    let mut pause = analysis.pause_while_thinking;
    if ui
        .checkbox(&mut pause, i18n.t("analysis-pause-while-thinking"))
        .changed()
    {
        analysis.set_pause_while_thinking(pause);
    }
    load_mode_row(ui, i18n, analysis);
    backend_row(ui, i18n, analysis);
    ui.small(i18n.t("analysis-load-note"));
}

/// Máximo / Equilibrado / Leve (UCI-06): quanto o governador pode tirar da análise.
fn load_mode_row(ui: &mut egui::Ui, i18n: &I18n, analysis: &mut Analysis) {
    ui.horizontal(|ui| {
        ui.label(i18n.t("analysis-load-label"));
        let mut mode = analysis.load_mode();
        for (value, key) in [
            (LoadMode::Max, "analysis-load-max"),
            (LoadMode::Balanced, "analysis-load-balanced"),
            (LoadMode::Light, "analysis-load-light"),
        ] {
            ui.selectable_value(&mut mode, value, i18n.t(key));
        }
        if mode != analysis.load_mode() {
            analysis.set_load_mode(mode);
        }
    });
    let key = match analysis.load_mode() {
        LoadMode::Max => "analysis-load-max-hint",
        LoadMode::Balanced => "analysis-load-balanced-hint",
        LoadMode::Light => "analysis-load-light-hint",
    };
    ui.small(i18n.t_with(key, &[("fps", &analysis.load.target().fps().to_string())]));
}

/// Backend do Lc0: automático ou escolhido à mão. Só aparece para Engines que o anunciam.
fn backend_row(ui: &mut egui::Ui, i18n: &I18n, analysis: &mut Analysis) {
    let known: Vec<String> = analysis
        .backends
        .iter()
        .filter(|b| chess_uci::backend_info(b).is_some())
        .cloned()
        .collect();
    if known.is_empty() || !analysis.has_engine() {
        return;
    }
    let label_of = |id: &str| {
        let info = chess_uci::backend_info(id);
        let kind = match info.map(|i| i.class) {
            Some(chess_uci::BackendClass::Gpu) => "GPU",
            _ => "CPU",
        };
        format!("{id} · {kind}")
    };
    let current = analysis.backend.clone();
    let mut picked: Option<Option<String>> = None;
    ui.horizontal(|ui| {
        ui.label(i18n.t("analysis-backend-label"));
        let selected = current
            .as_deref()
            .map_or_else(|| i18n.t("analysis-backend-auto"), label_of);
        egui::ComboBox::from_id_salt("analysis_backend_picker")
            .selected_text(selected)
            .show_ui(ui, |ui| {
                if ui
                    .selectable_label(current.is_none(), i18n.t("analysis-backend-auto"))
                    .clicked()
                {
                    picked = Some(None);
                }
                for id in &known {
                    let supported = chess_uci::supported_on(id, std::env::consts::OS);
                    let text = if supported {
                        label_of(id)
                    } else {
                        format!(
                            "{} ({})",
                            label_of(id),
                            i18n.t("analysis-backend-unsupported")
                        )
                    };
                    if ui
                        .selectable_label(current.as_deref() == Some(id.as_str()), text)
                        .clicked()
                    {
                        picked = Some(Some(id.clone()));
                    }
                }
            });
    });
    if let Some(choice) = picked {
        analysis.set_backend(choice);
    }
    if let Some(active) = analysis.backend_active() {
        ui.small(i18n.t_with("analysis-backend-active", &[("backend", &label_of(active))]));
    }
}

fn engine_picker(ui: &mut egui::Ui, i18n: &I18n, analysis: &mut Analysis) {
    let current = analysis
        .engine
        .as_ref()
        .and_then(|p| analysis.choices.iter().find(|c| &c.path == p))
        .map_or_else(|| i18n.t("analysis-engine-default"), |c| c.name.clone());
    let mut picked: Option<Option<PathBuf>> = None;
    ui.horizontal(|ui| {
        ui.label(i18n.t("analysis-engine-label"));
        egui::ComboBox::from_id_salt("analysis_engine_picker")
            .selected_text(current)
            .show_ui(ui, |ui| {
                if ui
                    .selectable_label(analysis.engine.is_none(), i18n.t("analysis-engine-default"))
                    .clicked()
                {
                    picked = Some(None);
                }
                for choice in &analysis.choices {
                    let selected = analysis.engine.as_ref() == Some(&choice.path);
                    if ui.selectable_label(selected, &choice.name).clicked() {
                        picked = Some(Some(choice.path.clone()));
                    }
                }
            });
    });
    if let Some(path) = picked {
        analysis.set_engine(path);
    }
    if let Some(name) = &analysis.engine_name
        && analysis.has_engine()
    {
        let (threads, hash) = analysis.applied;
        let detail = match (threads, hash) {
            (Some(t), Some(h)) => format!("{name} · {t} threads · {h} MB"),
            (Some(t), None) => format!("{name} · {t} threads"),
            _ => name.clone(),
        };
        ui.small(detail);
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;
    use crate::new_game::NewGameConfig;
    use chess_game::PolicyKind;

    fn unlocked() -> FairPlay {
        FairPlay::new(chess_net::FairPlayGuard::new())
    }

    fn competitive_session() -> Session {
        let mut session = Session::new();
        let config = NewGameConfig {
            policy: PolicyKind::FideSimulation,
            ..NewGameConfig::default()
        };
        session.start(&config, None);
        session
    }

    /// Análise ligada com uma Engine inexistente: o processo nem chega a existir, mas o
    /// controlador (a thread) sim, e é isso que a política precisa derrubar.
    fn armed() -> Analysis {
        let mut analysis = Analysis::default();
        analysis.set_default_engine(Some(PathBuf::from("/nao/existe/engine")));
        analysis.set_enabled(true);
        analysis
    }

    fn shown_with(session: &Session, lines: Vec<chess_uci::AnalysisLine>) -> Shown {
        Shown {
            update: chess_uci::AnalysisUpdate { id: 1, lines },
            base: session.snapshot().position.clone(),
            base_ply: 0,
            total: 0,
        }
    }

    fn pv_line(multipv: usize, mv: &str, position: &CanonicalPosition) -> chess_uci::AnalysisLine {
        let mv = position
            .legal_moves()
            .into_iter()
            .find(|m| m.to_string() == mv)
            .unwrap();
        chess_uci::AnalysisLine {
            multipv,
            depth: 10,
            seldepth: None,
            score: chess_uci::Score::Cp(10),
            nodes: None,
            nps: None,
            time_ms: None,
            pv: vec![mv],
            san: vec!["x".into()],
        }
    }

    #[test]
    fn hud_arrow_comes_from_multipv_one_and_is_forbidden_in_live_competitive() {
        use crate::hud_policy::{Allowance, allowance};
        let session = Session::new();
        let pos = session.snapshot().position.clone();
        let shown = shown_with(
            &session,
            vec![pv_line(2, "d2d4", &pos), pv_line(1, "e2e4", &pos)],
        );
        let best = hud_best(Some(&shown), &allowance(&session)).unwrap();
        assert_eq!(best.fen, pos.to_fen());
        assert_eq!(format!("{}{}", best.from, best.to), "e2e4");
        assert!(hud_best(None, &Allowance::ALL).is_none());
        assert!(hud_best(Some(&shown), &Allowance::COMPETITIVE_LIVE).is_none());
        let live = competitive_session();
        assert!(hud_best(Some(&shown), &allowance(&live)).is_none());
    }

    #[test]
    fn gate_locks_over_everything_and_pauses_only_when_asked() {
        let lock = Some(Lock::Competitive);
        assert_eq!(
            gate(lock, true, true, true),
            Gate::Locked(Lock::Competitive)
        );
        assert_eq!(
            gate(lock, false, false, false),
            Gate::Locked(Lock::Competitive)
        );
        assert_eq!(gate(None, false, true, true), Gate::Off);
        assert_eq!(gate(None, true, true, true), Gate::Paused);
        assert_eq!(gate(None, true, false, true), Gate::Run, "pausa é opção");
        assert_eq!(gate(None, true, true, false), Gate::Run);
    }

    #[test]
    fn lock_reason_prefers_competitive_then_training() {
        assert_eq!(lock_reason(true, true), Some(Lock::Competitive));
        assert_eq!(lock_reason(false, true), Some(Lock::Training));
        assert_eq!(lock_reason(false, false), None);
    }

    #[test]
    fn competitive_game_in_progress_blocks_and_switches_the_analysis_off() {
        let session = competitive_session();
        assert!(session.competitive_in_progress());
        let mut analysis = armed();
        analysis.drive(&session, false, &unlocked());
        assert_eq!(analysis.status(), &Status::Locked(Lock::Competitive));
        assert!(!analysis.is_enabled(), "desligada, não apenas escondida");
        assert!(
            !analysis.has_engine(),
            "nenhum processo de Engine de análise"
        );
        assert!(analysis.shown().is_none());
        // Ligar à força não adianta: o próximo quadro desliga de novo.
        analysis.set_enabled(true);
        analysis.drive(&session, false, &unlocked());
        assert!(!analysis.is_enabled());
        assert!(!analysis.has_engine());
    }

    #[test]
    fn competitive_lock_clears_a_running_analysis_immediately() {
        let assisted = Session::new();
        let mut analysis = armed();
        analysis.drive(&assisted, false, &unlocked());
        assert!(analysis.has_engine(), "Assistido: a análise sobe");
        let competitive = competitive_session();
        analysis.drive(&competitive, false, &unlocked());
        assert!(!analysis.has_engine(), "competitiva: derrubada");
        assert!(!analysis.is_enabled());
    }

    #[test]
    fn the_result_releases_the_analysis_for_review() {
        let mut session = competitive_session();
        let mut analysis = armed();
        analysis.drive(&session, false, &unlocked());
        assert_eq!(analysis.status(), &Status::Locked(Lock::Competitive));
        session.resign_side_to_move();
        assert!(session.snapshot().outcome.is_some());
        assert!(
            !session.competitive_in_progress(),
            "com resultado, revisão livre"
        );
        // Continua desligada até o usuário ligar; então roda.
        analysis.drive(&session, false, &unlocked());
        assert_eq!(analysis.status(), &Status::Off);
        analysis.set_enabled(true);
        analysis.drive(&session, false, &unlocked());
        assert!(analysis.has_engine());
        assert!(!matches!(analysis.status(), Status::Locked(_)));
    }

    #[test]
    fn the_dock_panel_is_inaccessible_until_the_result() {
        let def = PanelDef {
            id: ids::ANALYSIS,
            label_key: "dock-panel-analysis",
            access: PanelAccess::OutsideCompetitive,
            ui: |_, _, _| {},
        };
        let mut session = competitive_session();
        assert!(!def.accessible(&session));
        session.resign_side_to_move();
        assert!(def.accessible(&session));
        assert!(def.accessible(&Session::new()));
    }

    #[test]
    fn training_exercise_locks_the_analysis_too() {
        let session = Session::new();
        let mut analysis = armed();
        analysis.drive(&session, true, &unlocked());
        assert_eq!(analysis.status(), &Status::Locked(Lock::Training));
        assert!(!analysis.has_engine());
    }

    #[test]
    fn missing_engine_is_reported_without_starting_anything() {
        let session = Session::new();
        let mut analysis = Analysis::default();
        analysis.set_default_engine(None);
        analysis.set_enabled(true);
        analysis.drive(&session, false, &unlocked());
        assert_eq!(analysis.status(), &Status::NoEngine);
        assert!(!analysis.has_engine());
    }

    #[test]
    fn spawn_failure_becomes_a_failed_status_with_retry() {
        let session = Session::new();
        let mut analysis = armed();
        let deadline = Instant::now() + Duration::from_secs(5);
        while !matches!(analysis.status(), Status::Failed(_)) && Instant::now() < deadline {
            analysis.drive(&session, false, &unlocked());
            std::thread::sleep(Duration::from_millis(10));
        }
        assert!(matches!(analysis.status(), Status::Failed(_)));
        assert!(!analysis.has_engine());
        // "Tentar de novo" limpa a falha e sobe outra instância.
        analysis.failure = None;
        analysis.drive(&session, false, &unlocked());
        assert!(analysis.has_engine());
    }

    #[test]
    fn analysis_is_paused_while_the_opponent_engine_thinks_and_resumes_after() {
        let mut session = Session::new();
        session.start_engine_game(
            chess_game::EngineSeat {
                color: Side::White,
                elo: 1500,
            },
            EngineConfig::new("/nao/existe/engine"),
        );
        assert!(session.engine_turn(), "a Engine adversária joga de brancas");
        let mut analysis = armed();
        analysis.drive(&session, false, &unlocked());
        assert_eq!(analysis.status(), &Status::Paused);
        analysis.set_pause_while_thinking(false);
        analysis.drive(&session, false, &unlocked());
        assert!(analysis.has_engine(), "sem a opção, analisa mesmo assim");
        assert_ne!(analysis.status(), &Status::Paused);
    }

    #[test]
    fn limits_clamp_threads_and_hash() {
        let mut analysis = Analysis::default();
        analysis.set_limits(Limits {
            max_threads: 3,
            max_hash_mb: 256,
        });
        analysis.set_threads(64);
        analysis.set_hash_mb(100_000);
        let launch = {
            analysis.set_default_engine(Some(PathBuf::from("/x")));
            analysis.launch().unwrap()
        };
        assert_eq!((launch.threads, launch.hash_mb), (3, 256));
        analysis.set_threads(0);
        assert_eq!(analysis.launch().unwrap().threads, 1);
    }

    #[test]
    fn chosen_engine_options_come_from_the_manager_choice() {
        let mut analysis = Analysis::default();
        analysis.set_choices(vec![Choice {
            name: "Lc0".into(),
            path: PathBuf::from("/e/lc0"),
            options: vec![("WeightsFile".into(), "/e/net".into())],
        }]);
        analysis.set_default_engine(Some(PathBuf::from("/e/stockfish")));
        assert!(analysis.launch().unwrap().options.is_empty());
        analysis.set_engine(Some(PathBuf::from("/e/lc0")));
        let launch = analysis.launch().unwrap();
        assert_eq!(launch.path, PathBuf::from("/e/lc0"));
        assert_eq!(launch.options.len(), 1);
    }

    /// Análise "rodando" sem Engine real: o controlador existe, a Engine está pronta.
    fn running() -> (Analysis, Session) {
        let session = Session::new();
        let mut analysis = armed();
        analysis.drive(&session, false, &unlocked());
        assert!(analysis.has_engine());
        analysis.ready = true;
        analysis.status = Status::Running;
        (analysis, session)
    }

    fn frames(analysis: &mut Analysis, seconds: f64, frame_ms: f64) {
        for _ in 0..(seconds * 1000.0 / frame_ms) as usize {
            analysis.frame(frame_ms, GraphicsQuality::Balanced);
        }
    }

    #[test]
    fn low_fps_lightens_the_running_analysis_and_raises_the_notice() {
        let (mut analysis, _session) = running();
        frames(&mut analysis, 10.0, 16.7);
        assert_eq!(analysis.load_notice(), None, "FPS bom: sem aviso");
        frames(&mut analysis, 8.0, 40.0);
        assert!(analysis.load_level() > chess_uci::Level::Full);
        assert_eq!(analysis.load_notice(), Some(Notice::Reduced));
    }

    #[test]
    fn the_user_can_choose_maximum_balanced_or_light() {
        let (mut analysis, _session) = running();
        assert_eq!(analysis.load_mode(), LoadMode::Balanced, "padrão");
        analysis.set_load_mode(LoadMode::Max);
        frames(&mut analysis, 20.0, 40.0);
        assert_eq!(analysis.load_level(), chess_uci::Level::Full);
        assert_eq!(analysis.load_notice(), Some(Notice::BelowTarget));
        analysis.set_load_mode(LoadMode::Light);
        assert_eq!(analysis.load_level(), chess_uci::Level::Reduced);
    }

    #[test]
    fn a_slow_app_without_analysis_is_not_blamed_on_the_analysis() {
        let mut analysis = Analysis::default();
        frames(&mut analysis, 30.0, 50.0);
        assert_eq!(analysis.load_level(), chess_uci::Level::Full);
        assert_eq!(analysis.load_notice(), None);
    }

    #[test]
    fn turning_the_analysis_off_resets_the_governor() {
        let (mut analysis, session) = running();
        frames(&mut analysis, 12.0, 40.0);
        assert!(analysis.load_level() > chess_uci::Level::Full);
        analysis.set_enabled(false);
        analysis.drive(&session, false, &unlocked());
        analysis.frame(16.0, GraphicsQuality::Balanced);
        assert_eq!(analysis.load_level(), chess_uci::Level::Full);
    }

    #[test]
    fn the_economic_quality_targets_30_fps() {
        let (mut analysis, _session) = running();
        // 25 ms = 40 FPS: abaixo de 60, mas dentro da meta de 30.
        for _ in 0..800 {
            analysis.frame(25.0, GraphicsQuality::Economic);
        }
        assert_eq!(analysis.load_level(), chess_uci::Level::Full);
        let (mut analysis, _session) = running();
        for _ in 0..800 {
            analysis.frame(25.0, GraphicsQuality::Balanced);
        }
        assert!(analysis.load_level() > chess_uci::Level::Full);
    }

    #[test]
    fn a_manual_backend_goes_into_the_launch_and_restarts_the_engine() {
        let mut analysis = Analysis::default();
        analysis.set_default_engine(Some(PathBuf::from("/e/lc0")));
        let auto = analysis.launch().unwrap();
        assert_eq!(auto.backend, None);
        analysis.set_backend(Some("blas".into()));
        let manual = analysis.launch().unwrap();
        assert_eq!(manual.backend.as_deref(), Some("blas"));
        assert_ne!(auto, manual, "mudou o backend: a Engine reinicia");
    }

    #[test]
    fn score_text_and_bar_share() {
        assert_eq!(format_score(Score::Cp(35)), "+0.35");
        assert_eq!(format_score(Score::Cp(-120)), "-1.20");
        assert_eq!(format_score(Score::Cp(0)), "+0.00");
        assert_eq!(format_score(Score::Mate(3)), "M3");
        assert_eq!(format_score(Score::Mate(-2)), "-M2");
        assert!((white_share(Score::Cp(0)) - 0.5).abs() < 1e-6);
        assert!(white_share(Score::Cp(300)) > 0.65);
        assert!(white_share(Score::Cp(-300)) < 0.35);
        assert_eq!(white_share(Score::Mate(1)), 1.0);
        assert_eq!(white_share(Score::Mate(-1)), 0.0);
    }

    #[test]
    fn compact_numbers() {
        assert_eq!(compact(999), "999");
        assert_eq!(compact(1_234), "1,2k");
        assert_eq!(compact(5_300_000), "5,3M");
        assert_eq!(compact(2_000_000_000), "2,0G");
    }

    #[test]
    fn tokens_number_white_moves_and_a_leading_black_move() {
        let t = line_tokens(12, true, 3);
        let numbers: Vec<_> = t.iter().map(|t| t.number.as_deref()).collect();
        assert_eq!(numbers, [Some("12."), None, Some("13.")]);
        let t = line_tokens(12, false, 3);
        let numbers: Vec<_> = t.iter().map(|t| t.number.as_deref()).collect();
        assert_eq!(numbers, [Some("12..."), Some("13."), None]);
        assert_eq!(t[2].index, 2);
    }

    fn line(base: &CanonicalPosition, moves: &[&str]) -> AnalysisLine {
        let mut position = base.clone();
        let (mut pv, mut san) = (Vec::new(), Vec::new());
        for text in moves {
            let mv: chess_core::Move = text.parse().unwrap();
            san.push(position.san(mv).unwrap());
            position = position.play(mv).unwrap();
            pv.push(mv);
        }
        AnalysisLine {
            multipv: 1,
            depth: 10,
            seldepth: None,
            score: Score::Cp(10),
            nodes: None,
            nps: None,
            time_ms: None,
            pv,
            san,
        }
    }

    #[test]
    fn clicking_a_line_move_shows_the_variation_without_changing_the_game() {
        let mut session = Session::new();
        let base = session.snapshot().position.clone();
        let events_before = session.game().events().len();
        let history_before = session.snapshot().history.clone();
        let l = line(&base, &["e2e4", "e7e5", "g1f3"]);
        let view = variation_view(&base, 0, 0, &l, 2).unwrap();
        assert_eq!(view.last_move.as_ref().unwrap().san, "Nf3");
        assert_eq!(view.last_move.as_ref().unwrap().ply, 3);
        assert_eq!(view.ply, 0, "parte do meio-lance analisado");
        session.show_variation(view);
        assert!(session.showing_variation());
        assert_eq!(
            session.review().unwrap().position.to_fen(),
            "rnbqkbnr/pppp1ppp/8/4p3/4P3/5N2/PPPP1PPP/RNBQKB1R b KQkq - 1 2"
        );
        // A Partida em si não mudou: nem Eventos, nem histórico.
        assert_eq!(session.game().events().len(), events_before);
        assert_eq!(session.snapshot().history, history_before);
        // Navegar pela revisão (ou sair dela) substitui a variante.
        session.set_review_ply(None);
        assert!(!session.showing_variation());
        assert!(session.review().is_none());
    }

    #[test]
    fn variation_view_rejects_out_of_range_indexes() {
        let base = CanonicalPosition::start();
        let l = line(&base, &["e2e4"]);
        assert!(variation_view(&base, 0, 0, &l, 1).is_none());
    }
}
