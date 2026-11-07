//! Ponte entre o Bevy e o governador de carga (`chess_uci::Governor`, C10, UCI-06): junta o
//! tempo de cada quadro numa janela, consulta o governador e entrega o plano de carga que o
//! painel Análise aplica à Engine. Sem Bevy aqui dentro (só dados), para ser testado e para
//! o benchmark usar a mesma lógica do app.

use std::time::Duration;

use chess_uci::{FpsTarget, FrameWindow, Governor, Level, LoadMode, LoadPlan};

use crate::presentation::GraphicsQuality;

/// Janela de quadros que o p95 enxerga.
const WINDOW: Duration = Duration::from_millis(2000);
/// De quanto em quanto tempo o governador é consultado.
const EVAL_EVERY_S: f64 = 0.1;

/// Meta de FPS de cada qualidade gráfica: Econômica mira 30; Equilibrada e Ultra, 60.
pub fn target_for(quality: GraphicsQuality) -> FpsTarget {
    match quality {
        GraphicsQuality::Economic => FpsTarget::Fps30,
        GraphicsQuality::Balanced | GraphicsQuality::Ultra => FpsTarget::Fps60,
    }
}

/// O aviso que o painel mostra.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Notice {
    /// "Análise reduzida para manter o FPS."
    Reduced,
    /// A análise está parada para manter o FPS.
    Paused,
    /// O modo Máximo não reduz, mas o FPS está abaixo da meta.
    BelowTarget,
}

#[derive(Debug)]
pub struct LoadControl {
    governor: Governor,
    window: FrameWindow,
    since_eval_s: f64,
    /// Último plano entregue à Engine (`None` = a Engine é nova e ainda não recebeu nada).
    sent: Option<LoadPlan>,
}

impl LoadControl {
    pub fn new(mode: LoadMode, target: FpsTarget, threads: u32) -> Self {
        Self {
            governor: Governor::new(mode, target, threads),
            window: FrameWindow::new(WINDOW),
            since_eval_s: 0.0,
            sent: None,
        }
    }

    pub fn mode(&self) -> LoadMode {
        self.governor.mode()
    }

    pub fn set_mode(&mut self, mode: LoadMode) {
        self.governor.set_mode(mode);
    }

    pub fn set_target(&mut self, target: FpsTarget) {
        if self.governor.target() != target {
            self.governor.set_target(target);
        }
    }

    pub fn target(&self) -> FpsTarget {
        self.governor.target()
    }

    pub fn set_threads(&mut self, threads: u32) {
        self.governor.set_threads(threads);
    }

    pub fn level(&self) -> Level {
        self.governor.level()
    }

    pub fn plan(&self) -> LoadPlan {
        self.governor.plan()
    }

    /// Um quadro de `frame_ms`. Com a análise parada (`running = false`) nada é medido: o
    /// governador volta ao início e a janela esvazia, para a próxima análise começar limpa.
    pub fn frame(&mut self, frame_ms: f64, running: bool) {
        if !running {
            self.reset();
            return;
        }
        self.window.push(frame_ms);
        self.since_eval_s += frame_ms / 1000.0;
        if self.since_eval_s >= EVAL_EVERY_S {
            let dt = std::mem::take(&mut self.since_eval_s);
            self.governor.observe(dt, self.window.stats());
        }
    }

    pub fn reset(&mut self) {
        self.governor.reset();
        self.window.clear();
        self.since_eval_s = 0.0;
    }

    /// A Engine acabou de subir: o próximo plano pedido será entregue mesmo se igual.
    pub fn mark_unsent(&mut self) {
        self.sent = None;
    }

    /// O plano atual, se ainda não foi entregue à Engine.
    pub fn take_plan_change(&mut self) -> Option<LoadPlan> {
        let plan = self.plan();
        (self.sent != Some(plan)).then(|| {
            self.sent = Some(plan);
            plan
        })
    }

    pub fn notice(&self) -> Option<Notice> {
        match self.level() {
            Level::Paused => Some(Notice::Paused),
            // No Leve a análise já nasce reduzida: aviso só quando o governador piorou.
            Level::Reduced if self.mode() == LoadMode::Light => None,
            Level::Full => (self.mode() == LoadMode::Max && self.governor.below_target())
                .then_some(Notice::BelowTarget),
            _ => Some(Notice::Reduced),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn control(mode: LoadMode) -> LoadControl {
        LoadControl::new(mode, FpsTarget::Fps60, 8)
    }

    /// `seconds` s de quadros de `frame_ms` cada.
    fn feed(c: &mut LoadControl, seconds: f64, frame_ms: f64, running: bool) {
        let frames = (seconds * 1000.0 / frame_ms).round() as usize;
        for _ in 0..frames {
            c.frame(frame_ms, running);
        }
    }

    #[test]
    fn quality_picks_the_fps_target() {
        assert_eq!(target_for(GraphicsQuality::Economic), FpsTarget::Fps30);
        assert_eq!(target_for(GraphicsQuality::Balanced), FpsTarget::Fps60);
        assert_eq!(target_for(GraphicsQuality::Ultra), FpsTarget::Fps60);
    }

    #[test]
    fn smooth_frames_leave_the_analysis_alone() {
        let mut c = control(LoadMode::Balanced);
        feed(&mut c, 20.0, 16.7, true);
        assert_eq!(c.level(), Level::Full);
        assert_eq!(c.notice(), None);
        assert_eq!(
            c.take_plan_change(),
            Some(LoadPlan::FULL),
            "primeira entrega"
        );
        assert_eq!(c.take_plan_change(), None, "e só uma vez");
    }

    #[test]
    fn slow_frames_reduce_the_analysis_and_raise_the_notice() {
        let mut c = control(LoadMode::Balanced);
        c.take_plan_change();
        feed(&mut c, 6.0, 40.0, true);
        assert!(c.level() > Level::Full);
        assert_eq!(c.notice(), Some(Notice::Reduced));
        let plan = c.take_plan_change().expect("o plano mudou");
        assert!(plan.threads_cap.is_some());
    }

    #[test]
    fn a_long_enough_slowdown_pauses_and_says_so() {
        let mut c = control(LoadMode::Balanced);
        feed(&mut c, 15.0, 50.0, true);
        assert_eq!(c.level(), Level::Paused);
        assert_eq!(c.notice(), Some(Notice::Paused));
        assert!(c.plan().paused);
    }

    #[test]
    fn frames_are_ignored_while_the_analysis_is_not_running() {
        let mut c = control(LoadMode::Balanced);
        feed(&mut c, 30.0, 50.0, false);
        assert_eq!(
            c.level(),
            Level::Full,
            "o jogo lento não é culpa da análise"
        );
        feed(&mut c, 6.0, 40.0, true);
        assert!(c.level() > Level::Full);
        // Parou: o próximo uso recomeça do início.
        c.frame(16.0, false);
        assert_eq!(c.level(), Level::Full);
    }

    #[test]
    fn max_only_warns_and_light_starts_reduced_without_a_warning() {
        let mut max = control(LoadMode::Max);
        feed(&mut max, 10.0, 40.0, true);
        assert_eq!(max.level(), Level::Full);
        assert_eq!(max.notice(), Some(Notice::BelowTarget));
        let light = control(LoadMode::Light);
        assert_eq!(light.level(), Level::Reduced);
        assert_eq!(light.notice(), None);
        assert!(light.plan().threads_cap.is_some());
    }

    #[test]
    fn a_new_engine_gets_the_current_plan_again() {
        let mut c = control(LoadMode::Light);
        assert!(c.take_plan_change().is_some());
        assert!(c.take_plan_change().is_none());
        c.mark_unsent();
        assert!(c.take_plan_change().is_some());
    }

    #[test]
    fn switching_the_mode_takes_effect_at_once() {
        let mut c = control(LoadMode::Balanced);
        c.take_plan_change();
        feed(&mut c, 6.0, 40.0, true);
        assert!(c.level() > Level::Full);
        assert!(
            c.take_plan_change().is_some(),
            "a Engine recebeu o plano reduzido"
        );
        c.set_mode(LoadMode::Max);
        assert_eq!(c.plan(), LoadPlan::FULL);
        assert_eq!(c.take_plan_change(), Some(LoadPlan::FULL));
    }
}
