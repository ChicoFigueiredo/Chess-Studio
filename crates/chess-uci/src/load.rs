//! Governador de carga da análise (C10, UCI-06): decide quanto da máquina a Engine pode usar
//! para o FPS do app não cair. Módulo **puro**: recebe o tempo de quadro recente (p95, vindo
//! do Bevy) e devolve um [`LoadPlan`]; quem aplica o plano é o [`crate::AnalysisController`].
//!
//! A escada de níveis, do mais forte ao mais leve: `Full` (sem limite) → `Reduced`
//! (metade dos threads, lote e cache da rede menores) → `Minimal` (um thread, lote mínimo) →
//! `Intermittent` (a busca roda em rajadas e para entre elas: `stop` + retomar) → `Paused`
//! (a análise para até o FPS voltar). Para Engines de CPU (Stockfish) só o limite de threads
//! tem efeito: as opções de lote/cache não existem e são ignoradas.
//!
//! Histerese: sobe um nível quando o p95 fica acima da tolerância por um tempo, desce um nível
//! quando fica calmo por bem mais tempo, e uma volta rápida ao nível anterior (a carga
//! derrubou o FPS de novo) alonga a espera seguinte, para não oscilar.

use std::collections::VecDeque;
use std::time::Duration;

/// Quanto da máquina a análise pode usar, escolhido pelo usuário.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum LoadMode {
    /// Sem limite: a análise usa o que for configurado, mesmo que o FPS caia.
    Max,
    /// Reduz só quando o FPS cai abaixo da meta.
    #[default]
    Balanced,
    /// Já começa reduzida e reage cedo.
    Light,
}

impl LoadMode {
    pub const ALL: [Self; 3] = [Self::Max, Self::Balanced, Self::Light];
}

/// Meta de FPS (a qualidade gráfica escolhe).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FpsTarget {
    Fps30,
    Fps60,
}

impl FpsTarget {
    /// Tempo de quadro da meta, em ms.
    pub fn budget_ms(self) -> f64 {
        match self {
            Self::Fps30 => 1000.0 / 30.0,
            Self::Fps60 => 1000.0 / 60.0,
        }
    }

    pub fn fps(self) -> u32 {
        match self {
            Self::Fps30 => 30,
            Self::Fps60 => 60,
        }
    }
}

/// Resumo dos quadros recentes.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct FrameStats {
    pub p95_ms: f64,
    pub frames: usize,
}

/// Janela deslizante de tempos de quadro, medida em tempo real (soma dos próprios quadros).
#[derive(Debug, Clone)]
pub struct FrameWindow {
    span_ms: f64,
    samples: VecDeque<f64>,
    total_ms: f64,
}

impl FrameWindow {
    /// Janela que cobre os últimos `span` de quadros.
    pub fn new(span: Duration) -> Self {
        Self {
            span_ms: span.as_secs_f64() * 1000.0,
            samples: VecDeque::new(),
            total_ms: 0.0,
        }
    }

    pub fn push(&mut self, frame_ms: f64) {
        if !frame_ms.is_finite() || frame_ms < 0.0 {
            return;
        }
        self.samples.push_back(frame_ms);
        self.total_ms += frame_ms;
        // Sai o que ficou mais velho que a janela, mas sempre sobra o quadro mais recente.
        while self.samples.len() > 1 {
            let oldest = self.samples.front().copied().unwrap_or_default();
            if self.total_ms - oldest < self.span_ms {
                break;
            }
            self.samples.pop_front();
            self.total_ms -= oldest;
        }
    }

    pub fn clear(&mut self) {
        self.samples.clear();
        self.total_ms = 0.0;
    }

    pub fn stats(&self) -> FrameStats {
        let mut sorted: Vec<f64> = self.samples.iter().copied().collect();
        sorted.sort_by(f64::total_cmp);
        let p95_ms = match sorted.len() {
            0 => 0.0,
            n => {
                #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
                let rank = ((n as f64) * 0.95).ceil() as usize;
                sorted[rank.clamp(1, n) - 1]
            }
        };
        FrameStats {
            p95_ms,
            frames: sorted.len(),
        }
    }
}

/// Nível de redução da análise (ordenado: maior = mais leve para a máquina).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Level {
    Full,
    Reduced,
    Minimal,
    Intermittent,
    Paused,
}

impl Level {
    fn up(self) -> Self {
        match self {
            Self::Full => Self::Reduced,
            Self::Reduced => Self::Minimal,
            Self::Minimal => Self::Intermittent,
            Self::Intermittent | Self::Paused => Self::Paused,
        }
    }

    fn down(self) -> Self {
        match self {
            Self::Paused => Self::Intermittent,
            Self::Intermittent => Self::Minimal,
            Self::Minimal => Self::Reduced,
            Self::Reduced | Self::Full => Self::Full,
        }
    }
}

/// Rajadas da busca no nível `Intermittent`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Duty {
    pub run: Duration,
    pub pause: Duration,
}

/// Limites que a Engine de análise deve respeitar. Cada teto só vale se a Engine anuncia a
/// opção correspondente; `None` = sem teto.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LoadPlan {
    pub level: Level,
    /// Teto de `Threads`.
    pub threads_cap: Option<u32>,
    /// Teto de `MinibatchSize` (Lc0).
    pub minibatch_cap: Option<u32>,
    /// Teto de `NNCacheSize` (Lc0).
    pub nn_cache_cap: Option<u32>,
    pub duty: Option<Duty>,
    pub paused: bool,
}

impl LoadPlan {
    /// Sem limite nenhum.
    pub const FULL: Self = Self {
        level: Level::Full,
        threads_cap: None,
        minibatch_cap: None,
        nn_cache_cap: None,
        duty: None,
        paused: false,
    };

    /// Plano do nível, para uma análise configurada com `threads` threads.
    pub fn for_level(level: Level, threads: u32) -> Self {
        let half = (threads / 2).max(1);
        let base = Self {
            level,
            ..Self::FULL
        };
        match level {
            Level::Full => Self::FULL,
            Level::Reduced => Self {
                threads_cap: Some(half),
                minibatch_cap: Some(64),
                nn_cache_cap: Some(400_000),
                ..base
            },
            Level::Minimal => Self {
                threads_cap: Some(1),
                minibatch_cap: Some(16),
                nn_cache_cap: Some(100_000),
                ..base
            },
            Level::Intermittent => Self {
                threads_cap: Some(1),
                minibatch_cap: Some(16),
                nn_cache_cap: Some(100_000),
                duty: Some(Duty {
                    run: Duration::from_millis(200),
                    pause: Duration::from_millis(600),
                }),
                ..base
            },
            Level::Paused => Self {
                paused: true,
                ..base
            },
        }
    }
}

/// Quadros mínimos na janela para o p95 valer algo (app parado quase não desenha).
const MIN_FRAMES: usize = 20;
/// Tempo depois de uma mudança para a janela de quadros refletir o novo nível.
const SETTLE_S: f64 = 2.0;
/// Recaída dentro deste prazo depois de soltar o nível conta como oscilação.
const RELAPSE_S: f64 = 30.0;
const MAX_RECOVER_SCALE: f64 = 6.0;
/// Segundos de FPS ruim com a análise **pausada** até concluir que a culpa não é dela.
const FUTILE_AFTER_S: f64 = 8.0;
/// Quanto tempo a análise fica sem escalar depois dessa conclusão.
const FUTILE_HOLD_S: f64 = 60.0;

/// Sensibilidade de cada modo.
struct Tuning {
    /// p95 / meta acima disto é "FPS abaixo da meta".
    over: f64,
    /// p95 / meta até isto é "calmo".
    calm: f64,
    /// Segundos seguidos acima da tolerância para subir um nível.
    react_s: f64,
    /// Segundos seguidos calmos para descer um nível.
    recover_s: f64,
}

impl Tuning {
    fn of(mode: LoadMode) -> Self {
        match mode {
            LoadMode::Max | LoadMode::Balanced => Self {
                over: 1.25,
                calm: 1.10,
                react_s: 1.0,
                recover_s: 10.0,
            },
            LoadMode::Light => Self {
                over: 1.10,
                calm: 1.05,
                react_s: 0.5,
                recover_s: 20.0,
            },
        }
    }
}

/// O governador.
#[derive(Debug, Clone)]
pub struct Governor {
    mode: LoadMode,
    target: FpsTarget,
    threads: u32,
    level: Level,
    clock_s: f64,
    since_change_s: f64,
    over_s: f64,
    calm_s: f64,
    lowered_at_s: Option<f64>,
    recover_scale: f64,
    below: bool,
    /// Até quando uma pausa que não adiantou segura a análise no nível `Reduced`.
    futile_until_s: Option<f64>,
}

/// Nível mais baixo que o modo aceita.
fn floor(mode: LoadMode) -> Level {
    match mode {
        LoadMode::Max | LoadMode::Balanced => Level::Full,
        LoadMode::Light => Level::Reduced,
    }
}

impl Governor {
    pub fn new(mode: LoadMode, target: FpsTarget, threads: u32) -> Self {
        Self {
            mode,
            target,
            threads,
            level: floor(mode),
            clock_s: 0.0,
            since_change_s: SETTLE_S,
            over_s: 0.0,
            calm_s: 0.0,
            lowered_at_s: None,
            recover_scale: 1.0,
            below: false,
            futile_until_s: None,
        }
    }

    pub fn mode(&self) -> LoadMode {
        self.mode
    }

    pub fn target(&self) -> FpsTarget {
        self.target
    }

    pub fn level(&self) -> Level {
        self.level
    }

    /// Plano atual.
    pub fn plan(&self) -> LoadPlan {
        LoadPlan::for_level(self.level, self.threads)
    }

    /// O FPS está abaixo da meta agora (mesmo que o modo não permita reduzir)?
    pub fn below_target(&self) -> bool {
        self.below
    }

    /// Muda o modo; devolve o novo plano se ele mudou.
    pub fn set_mode(&mut self, mode: LoadMode) -> Option<LoadPlan> {
        if mode == self.mode {
            return None;
        }
        let before = self.plan();
        self.mode = mode;
        self.reset();
        (self.plan() != before).then(|| self.plan())
    }

    pub fn set_target(&mut self, target: FpsTarget) {
        self.target = target;
    }

    /// Threads configurados da análise (base do teto do nível `Reduced`).
    pub fn set_threads(&mut self, threads: u32) -> Option<LoadPlan> {
        let before = self.plan();
        self.threads = threads;
        (self.plan() != before).then(|| self.plan())
    }

    /// Volta ao nível inicial do modo (análise desligada ou reiniciada).
    pub fn reset(&mut self) {
        self.level = floor(self.mode);
        self.since_change_s = SETTLE_S;
        self.over_s = 0.0;
        self.calm_s = 0.0;
        self.lowered_at_s = None;
        self.recover_scale = 1.0;
        self.below = false;
        self.futile_until_s = None;
    }

    /// Passa `dt_s` segundos com os quadros `stats`; devolve o novo plano quando muda.
    pub fn observe(&mut self, dt_s: f64, stats: FrameStats) -> Option<LoadPlan> {
        let dt_s = dt_s.max(0.0);
        self.clock_s += dt_s;
        self.since_change_s += dt_s;
        let tuning = Tuning::of(self.mode);
        let budget = self.target.budget_ms();
        if stats.frames < MIN_FRAMES {
            self.below = false;
            self.over_s = 0.0;
            self.calm_s = 0.0;
            return None;
        }
        let over = stats.p95_ms > budget * tuning.over;
        self.below = over;
        if self.mode == LoadMode::Max || self.since_change_s < SETTLE_S {
            // Max só observa; depois de uma mudança, espera a janela refletir o novo nível.
            self.over_s = 0.0;
            self.calm_s = 0.0;
            return None;
        }
        if over {
            self.calm_s = 0.0;
            self.over_s += dt_s;
            if self.level == Level::Paused && self.over_s >= FUTILE_AFTER_S {
                // Pausada e o FPS continua ruim: a causa é outra (máquina ocupada). Devolve
                // a análise, reduzida, e não escala de novo por um tempo.
                self.level = Level::Reduced;
                self.futile_until_s = Some(self.clock_s + FUTILE_HOLD_S);
                return Some(self.changed());
            }
            let cap = if self.futile_until_s.is_some_and(|t| self.clock_s < t) {
                Level::Reduced
            } else {
                Level::Paused
            };
            if self.over_s >= tuning.react_s && self.level < cap {
                if self
                    .lowered_at_s
                    .is_some_and(|t| self.clock_s - t < RELAPSE_S)
                {
                    self.recover_scale = (self.recover_scale * 2.0).min(MAX_RECOVER_SCALE);
                }
                self.level = self.level.up();
                return Some(self.changed());
            }
        } else if stats.p95_ms <= budget * tuning.calm {
            self.over_s = 0.0;
            self.calm_s += dt_s;
            if self.calm_s >= tuning.recover_s * self.recover_scale && self.level > floor(self.mode)
            {
                self.level = self.level.down();
                self.lowered_at_s = Some(self.clock_s);
                return Some(self.changed());
            }
        } else {
            // Entre "calmo" e "acima da tolerância": segura onde está.
            self.over_s = 0.0;
            self.calm_s = 0.0;
        }
        None
    }

    fn changed(&mut self) -> LoadPlan {
        self.since_change_s = 0.0;
        self.over_s = 0.0;
        self.calm_s = 0.0;
        self.plan()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const GOOD: FrameStats = FrameStats {
        p95_ms: 16.9,
        frames: 120,
    };
    const BAD: FrameStats = FrameStats {
        p95_ms: 45.0,
        frames: 60,
    };

    fn gov(mode: LoadMode) -> Governor {
        Governor::new(mode, FpsTarget::Fps60, 8)
    }

    /// Alimenta o governador por `seconds` s em passos de 0,25 s; devolve os planos emitidos.
    fn run(g: &mut Governor, seconds: f64, stats: FrameStats) -> Vec<LoadPlan> {
        let mut plans = Vec::new();
        let steps = (seconds / 0.25).round() as usize;
        for _ in 0..steps {
            if let Some(plan) = g.observe(0.25, stats) {
                plans.push(plan);
            }
        }
        plans
    }

    #[test]
    fn the_window_reports_p95_over_the_recent_frames_only() {
        let mut w = FrameWindow::new(Duration::from_secs(2));
        assert_eq!(w.stats().frames, 0);
        for _ in 0..95 {
            w.push(10.0);
        }
        for _ in 0..5 {
            w.push(50.0);
        }
        let s = w.stats();
        assert_eq!(s.frames, 100);
        assert!((s.p95_ms - 50.0).abs() < 1e-9 || (s.p95_ms - 10.0).abs() < 1e-9);
        assert!(s.p95_ms >= 10.0);
        // 200 quadros de 20 ms = 4 s: a janela de 2 s guarda só os últimos ~100.
        let mut w = FrameWindow::new(Duration::from_secs(2));
        for _ in 0..50 {
            w.push(200.0); // fica velho
        }
        for _ in 0..200 {
            w.push(20.0);
        }
        let s = w.stats();
        assert!(s.frames <= 101 && s.frames >= 99, "{}", s.frames);
        assert!((s.p95_ms - 20.0).abs() < 1e-9, "quadros velhos saíram");
    }

    #[test]
    fn levels_map_to_concrete_limits() {
        assert_eq!(LoadPlan::for_level(Level::Full, 8), LoadPlan::FULL);
        let reduced = LoadPlan::for_level(Level::Reduced, 8);
        assert_eq!(reduced.threads_cap, Some(4));
        assert!(reduced.minibatch_cap.is_some() && reduced.nn_cache_cap.is_some());
        assert_eq!(LoadPlan::for_level(Level::Reduced, 1).threads_cap, Some(1));
        let minimal = LoadPlan::for_level(Level::Minimal, 8);
        assert_eq!(minimal.threads_cap, Some(1));
        assert!(minimal.minibatch_cap < reduced.minibatch_cap);
        assert!(minimal.nn_cache_cap < reduced.nn_cache_cap);
        let burst = LoadPlan::for_level(Level::Intermittent, 8);
        let duty = burst.duty.expect("rajadas");
        assert!(
            duty.pause > duty.run,
            "a pausa domina no nível intermitente"
        );
        assert!(!burst.paused);
        let paused = LoadPlan::for_level(Level::Paused, 8);
        assert!(paused.paused);
    }

    #[test]
    fn max_never_reduces_but_still_reports_the_low_fps() {
        let mut g = gov(LoadMode::Max);
        assert!(run(&mut g, 30.0, BAD).is_empty());
        assert_eq!(g.level(), Level::Full);
        assert!(g.below_target(), "o aviso existe mesmo sem reduzir");
        run(&mut g, 4.0, GOOD);
        assert!(!g.below_target());
    }

    #[test]
    fn balanced_reacts_to_sustained_low_fps_and_ignores_a_blip() {
        let mut g = gov(LoadMode::Balanced);
        assert!(run(&mut g, 10.0, GOOD).is_empty(), "FPS bom: nada muda");
        // Um tranco de meio segundo não reduz.
        assert!(run(&mut g, 0.5, BAD).is_empty());
        run(&mut g, 4.0, GOOD);
        assert_eq!(g.level(), Level::Full);
        // Sustentado: reduz.
        let plans = run(&mut g, 3.0, BAD);
        assert_eq!(plans.first().map(|p| p.level), Some(Level::Reduced));
        assert_eq!(g.level(), Level::Reduced);
    }

    #[test]
    fn it_escalates_one_level_at_a_time_up_to_pause_and_no_further() {
        let mut g = gov(LoadMode::Balanced);
        let plans = run(&mut g, 18.0, BAD);
        let levels: Vec<Level> = plans.iter().map(|p| p.level).collect();
        assert_eq!(
            levels,
            [
                Level::Reduced,
                Level::Minimal,
                Level::Intermittent,
                Level::Paused
            ]
        );
        assert_eq!(g.level(), Level::Paused);
        assert!(g.plan().paused);
    }

    #[test]
    fn each_step_waits_for_the_frame_window_to_reflect_the_last_change() {
        let mut g = gov(LoadMode::Balanced);
        let mut at = Vec::new();
        for i in 0..80 {
            if g.observe(0.25, BAD).is_some() {
                at.push(f64::from(i) * 0.25);
            }
        }
        for pair in at.windows(2) {
            assert!(pair[1] - pair[0] >= 2.0, "passos muito próximos: {at:?}");
        }
    }

    #[test]
    fn a_pause_that_does_not_help_is_not_kept_forever() {
        // O FPS fica ruim por outro motivo (máquina ocupada): pausar a análise não resolve.
        let mut g = gov(LoadMode::Balanced);
        let plans = run(&mut g, 120.0, BAD);
        let levels: Vec<Level> = plans.iter().map(|p| p.level).collect();
        let paused_at = levels.iter().position(|l| *l == Level::Paused).unwrap();
        assert_eq!(
            levels[paused_at + 1],
            Level::Reduced,
            "sem melhora, a análise volta ao nível reduzido: {levels:?}"
        );
        // Durante a espera não escala de novo; passada a espera, tenta de novo.
        let held = &levels[paused_at + 1..];
        assert!(held.iter().take(1).all(|l| *l == Level::Reduced));
        assert!(
            levels[paused_at + 2..].iter().any(|l| *l > Level::Reduced),
            "depois da espera volta a escalar: {levels:?}"
        );
    }

    #[test]
    fn the_hold_after_a_futile_pause_keeps_the_level_at_reduced() {
        let mut g = gov(LoadMode::Balanced);
        // Até pausar, mais o tempo de perceber que não adiantou.
        let mut elapsed = 0.0;
        while g.level() != Level::Paused {
            g.observe(0.25, BAD);
            elapsed += 0.25;
            assert!(elapsed < 60.0);
        }
        while g.level() == Level::Paused {
            g.observe(0.25, BAD);
        }
        assert_eq!(g.level(), Level::Reduced);
        // Os próximos 50 s de FPS ruim não escalam.
        let plans = run(&mut g, 50.0, BAD);
        assert!(plans.is_empty(), "{plans:?}");
        assert_eq!(g.level(), Level::Reduced);
    }

    #[test]
    fn it_recovers_one_level_after_a_long_calm_not_before() {
        let mut g = gov(LoadMode::Balanced);
        run(&mut g, 10.0, BAD);
        let high = g.level();
        assert!(high > Level::Reduced);
        assert!(run(&mut g, 5.0, GOOD).is_empty(), "calma curta não solta");
        let plans = run(&mut g, 20.0, GOOD);
        assert!(!plans.is_empty());
        assert!(plans[0].level < high);
        // Com calma sustentada volta ao pleno.
        run(&mut g, 200.0, GOOD);
        assert_eq!(g.level(), Level::Full);
    }

    #[test]
    fn a_quick_relapse_lengthens_the_next_recovery() {
        let mut g = gov(LoadMode::Balanced);
        run(&mut g, 2.5, BAD);
        assert_eq!(g.level(), Level::Reduced);
        // Solta depois da calma...
        let mut calm_to_release = 0.0;
        while g.level() == Level::Reduced && calm_to_release < 100.0 {
            g.observe(0.25, GOOD);
            calm_to_release += 0.25;
        }
        assert_eq!(g.level(), Level::Full);
        // ...o FPS cai de novo logo em seguida: reduz e a próxima espera é maior.
        run(&mut g, 3.5, BAD);
        assert_eq!(g.level(), Level::Reduced);
        let mut second = 0.0;
        while g.level() == Level::Reduced && second < 200.0 {
            g.observe(0.25, GOOD);
            second += 0.25;
        }
        assert!(second > calm_to_release, "{second} <= {calm_to_release}");
    }

    #[test]
    fn light_starts_reduced_and_never_goes_below_that_floor() {
        let mut g = gov(LoadMode::Light);
        assert_eq!(g.level(), Level::Reduced);
        assert_eq!(g.plan().threads_cap, Some(4));
        run(&mut g, 300.0, GOOD);
        assert_eq!(g.level(), Level::Reduced, "calma não leva a Full no Leve");
    }

    #[test]
    fn light_is_more_sensitive_than_balanced() {
        let mild = FrameStats {
            p95_ms: 19.0, // 53 FPS no p95: Equilibrado tolera, Leve não
            frames: 100,
        };
        let mut balanced = gov(LoadMode::Balanced);
        assert!(run(&mut balanced, 20.0, mild).is_empty());
        let mut light = gov(LoadMode::Light);
        run(&mut light, 8.0, mild);
        assert!(light.level() > Level::Reduced);
    }

    #[test]
    fn the_30_fps_target_tolerates_what_the_60_fps_target_does_not() {
        let slow = FrameStats {
            p95_ms: 30.0,
            frames: 60,
        };
        let mut g60 = Governor::new(LoadMode::Balanced, FpsTarget::Fps60, 4);
        run(&mut g60, 10.0, slow);
        assert!(g60.level() > Level::Full);
        let mut g30 = Governor::new(LoadMode::Balanced, FpsTarget::Fps30, 4);
        assert!(run(&mut g30, 10.0, slow).is_empty());
    }

    #[test]
    fn too_few_frames_say_nothing() {
        let mut g = gov(LoadMode::Balanced);
        let idle = FrameStats {
            p95_ms: 500.0,
            frames: 3,
        };
        assert!(run(&mut g, 30.0, idle).is_empty());
        assert!(!g.below_target());
    }

    #[test]
    fn changing_mode_or_resetting_restores_the_starting_plan() {
        let mut g = gov(LoadMode::Balanced);
        run(&mut g, 30.0, BAD);
        assert!(g.level() > Level::Full);
        assert_eq!(g.set_mode(LoadMode::Max), Some(LoadPlan::FULL));
        assert_eq!(g.set_mode(LoadMode::Max), None, "nada mudou");
        run(&mut g, 30.0, BAD);
        g.set_mode(LoadMode::Light);
        assert_eq!(g.level(), Level::Reduced);
        g.reset();
        assert_eq!(g.level(), Level::Reduced);
        let mut g = gov(LoadMode::Balanced);
        run(&mut g, 30.0, BAD);
        g.reset();
        assert_eq!(g.level(), Level::Full);
    }

    #[test]
    fn the_reduced_cap_follows_the_configured_threads() {
        let mut g = Governor::new(LoadMode::Light, FpsTarget::Fps60, 8);
        assert_eq!(g.plan().threads_cap, Some(4));
        let plan = g.set_threads(2).expect("o teto mudou");
        assert_eq!(plan.threads_cap, Some(1));
    }
}
