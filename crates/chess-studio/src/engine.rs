//! Engine adversária no app (B9): escolha de lado e Elo, "Engine pensando…", falhas com
//! "Tentar de novo" e "Trocar Engine". O desenho da conversa UCI está em `chess-uci`
//! (`EnginePlayer`, thread própria); aqui só há o plugin que dá o pulso a cada quadro e
//! o trecho do painel.
//!
//! Ponto de extensão para a tela de Nova Partida (B7+B8): ela só precisa chamar
//! [`Session::start_engine_game`] com um `EngineSeat` (lado e Elo) e uma
//! `EngineConfig` — por exemplo, a de [`resolve_engine`].
//!
//! O executável vem da variável `CHESS_STUDIO_ENGINE`, de `stockfish` no `PATH`, da Engine
//! escolhida antes no assistente (B10, `engine_wizard`) ou do caminho digitado. Sem
//! nenhuma, "Iniciar" abre o assistente de detecção/download.

use crate::i18n::I18n;
use crate::play::PlaySet;
use crate::session::Session;
use bevy::prelude::*;
use bevy_egui::egui;
use chess_core::Color as Side;
use chess_game::EngineSeat;
use chess_uci::{EngineConfig, FailureKind, PlayerStatus, StrengthApplied};
use std::ffi::OsString;
use std::path::{Path, PathBuf};

/// Variável de ambiente com o caminho do executável da Engine.
pub const ENGINE_ENV: &str = "CHESS_STUDIO_ENGINE";

pub struct EnginePlugin;

impl Plugin for EnginePlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<EngineUi>()
            .add_plugins(crate::engine_wizard::EngineWizardPlugin)
            .add_plugins(crate::engine_manager::EngineManagerPlugin)
            .add_systems(Update, tick_engine.in_set(PlaySet));
    }
}

/// Dá o pulso à Engine: nunca bloqueia (ela roda em thread própria).
fn tick_engine(mut session: ResMut<Session>) {
    // Leitura imutável primeiro: só toma `&mut` quando há Engine.
    if session.engine_active() {
        session.tick_engine();
    }
}

/// Executável da Engine: `CHESS_STUDIO_ENGINE` se existir; senão `stockfish` no `PATH`.
pub fn find_engine(
    env: Option<OsString>,
    path_var: Option<OsString>,
    is_file: impl Fn(&Path) -> bool,
) -> Option<PathBuf> {
    if let Some(explicit) = env.filter(|v| !v.is_empty()) {
        return Some(PathBuf::from(explicit));
    }
    let names: &[&str] = if cfg!(windows) {
        &["stockfish.exe"]
    } else {
        &["stockfish"]
    };
    std::env::split_paths(&path_var?)
        .flat_map(|dir| names.iter().map(move |name| dir.join(name)))
        .find(|candidate| is_file(candidate))
}

/// [`find_engine`] sobre o ambiente real; sem resultado, a Engine escolhida/instalada pelo
/// assistente (B10) em outra sessão.
pub fn resolve_engine() -> Option<PathBuf> {
    find_engine(
        std::env::var_os(ENGINE_ENV),
        std::env::var_os("PATH"),
        Path::is_file,
    )
    .or_else(|| chess_uci::engines_dir().and_then(|dir| chess_uci::load_preferred(&dir)))
}

/// Faixa de Elo do controle do painel.
pub const MIN_ELO: u32 = 800;
pub const MAX_ELO: u32 = 3190;

/// Escolhas do painel (lado, Elo, caminho).
#[derive(Resource, Debug)]
pub struct EngineUi {
    user_side: Side,
    elo: u32,
    path_text: String,
    not_found: bool,
    /// Pedido de abertura do assistente de Engine (B10), atendido por `engine_wizard`.
    wizard_requested: bool,
    /// Opções UCI configuradas no gerenciador para a Engine escolhida (já validadas).
    options: Vec<(String, String)>,
}

impl Default for EngineUi {
    fn default() -> Self {
        Self {
            user_side: Side::White,
            elo: 1500,
            path_text: resolve_engine()
                .map(|p| p.display().to_string())
                .unwrap_or_default(),
            not_found: false,
            wizard_requested: false,
            options: Vec::new(),
        }
    }
}

impl EngineUi {
    /// Escolhas gravadas (lado do usuário e Elo) sobre o padrão.
    pub fn with_saved(mut self, user_side: Side, elo: u32) -> Self {
        self.user_side = user_side;
        self.elo = elo.clamp(MIN_ELO, MAX_ELO);
        self
    }

    /// Reaplica as escolhas gravadas de outro perfil local, sem tocar no resto do estado.
    pub fn apply_saved(&mut self, user_side: Side, elo: u32) {
        self.user_side = user_side;
        self.elo = elo.clamp(MIN_ELO, MAX_ELO);
    }

    pub fn user_side(&self) -> Side {
        self.user_side
    }

    pub fn elo(&self) -> u32 {
        self.elo
    }

    /// Caminho escolhido no assistente de Engine.
    pub fn set_path(&mut self, path: &Path) {
        self.path_text = path.display().to_string();
        self.not_found = false;
    }

    /// Opções UCI (threads, hash, nível…) a enviar à Engine ao iniciar.
    pub fn set_options(&mut self, options: Vec<(String, String)>) {
        self.options = options;
    }

    /// Consome o pedido de abrir o assistente.
    pub fn take_wizard_request(&mut self) -> bool {
        std::mem::take(&mut self.wizard_requested)
    }

    /// Há um executável de Engine configurado (variável, `PATH`, assistente ou digitado)?
    pub fn has_engine(&self) -> bool {
        self.engine_config().is_some()
    }

    /// Configuração da Engine a partir do caminho atual.
    pub fn engine_config(&self) -> Option<EngineConfig> {
        self.config()
    }

    /// Pede a abertura do assistente de Engine (B10).
    pub fn request_wizard(&mut self) {
        self.wizard_requested = true;
    }

    /// Guarda o lado do usuário e o Elo escolhidos na Nova Partida (a persistência os grava).
    pub fn set_choice(&mut self, user_side: Side, elo: u32) {
        self.user_side = user_side;
        self.elo = elo.clamp(MIN_ELO, MAX_ELO);
    }

    fn config(&self) -> Option<EngineConfig> {
        let text = self.path_text.trim();
        (!text.is_empty()).then(|| {
            self.options
                .iter()
                .fold(EngineConfig::new(text), |config, (name, value)| {
                    config.option(name, value)
                })
        })
    }
}

fn failure_key(kind: FailureKind) -> &'static str {
    match kind {
        FailureKind::Spawn => "engine-failed-spawn",
        FailureKind::Crashed => "engine-failed-crashed",
        FailureKind::Timeout => "engine-failed-timeout",
        FailureKind::Unresponsive => "engine-failed-unresponsive",
        FailureKind::Protocol => "engine-failed-protocol",
        FailureKind::IllegalMove => "engine-failed-illegal",
    }
}

/// Trecho do painel: escolha para começar contra a Engine e, durante a Partida, o estado.
pub fn engine_section(ui: &mut egui::Ui, i18n: &I18n, session: &mut Session, state: &mut EngineUi) {
    ui.separator();
    ui.strong(i18n.t("engine-heading"));
    if session.engine_active() {
        playing_status(ui, i18n, session, state);
    }
    ui.horizontal(|ui| {
        ui.label(i18n.t("engine-side-label"));
        ui.selectable_value(
            &mut state.user_side,
            Side::White,
            i18n.t("engine-side-white"),
        );
        ui.selectable_value(
            &mut state.user_side,
            Side::Black,
            i18n.t("engine-side-black"),
        );
    });
    ui.add(egui::Slider::new(&mut state.elo, MIN_ELO..=MAX_ELO).text(i18n.t("engine-elo-label")));
    ui.horizontal(|ui| {
        ui.label(i18n.t("engine-path-label"));
        ui.add(
            egui::TextEdit::singleline(&mut state.path_text).hint_text(i18n.t("engine-path-hint")),
        );
    });
    if ui.button(i18n.t("engine-start")).clicked() {
        match state.config() {
            Some(config) => {
                state.not_found = false;
                let seat = EngineSeat {
                    color: state.user_side.opposite(),
                    elo: state.elo,
                };
                session.start_engine_game(seat, config);
            }
            None => {
                state.not_found = true;
                state.wizard_requested = true;
            }
        }
    }
    if ui.small_button(i18n.t("wizard-open")).clicked() {
        state.wizard_requested = true;
    }
    if state.not_found {
        ui.colored_label(egui::Color32::YELLOW, i18n.t("engine-not-found"));
    }
}

fn playing_status(ui: &mut egui::Ui, i18n: &I18n, session: &mut Session, state: &mut EngineUi) {
    let Some(engine) = session.engine() else {
        return;
    };
    let elo = engine.seat().elo.to_string();
    let heading = match engine.engine_name() {
        Some(name) => i18n.t_with("engine-playing", &[("name", name), ("elo", &elo)]),
        None => i18n.t_with("engine-playing-unnamed", &[("elo", &elo)]),
    };
    ui.label(heading);
    match engine.strength() {
        Some(StrengthApplied::Elo { requested, used }) if requested != used => {
            ui.small(i18n.t_with("engine-strength-clamped", &[("used", &used.to_string())]));
        }
        Some(StrengthApplied::SkillLevel { level, .. }) => {
            ui.small(i18n.t_with("engine-strength-skill", &[("level", &level.to_string())]));
        }
        Some(StrengthApplied::Unsupported) => {
            ui.small(i18n.t("engine-strength-unsupported"));
        }
        _ => {}
    }
    match engine.status() {
        PlayerStatus::Thinking => {
            ui.horizontal(|ui| {
                ui.spinner();
                ui.label(i18n.t("engine-thinking"));
            });
        }
        PlayerStatus::Waiting => {}
        PlayerStatus::Failed(failure) => {
            ui.colored_label(egui::Color32::LIGHT_RED, i18n.t("engine-failed-title"));
            ui.label(i18n.t(failure_key(failure.kind)));
            ui.small(&failure.message);
            ui.small(i18n.t("engine-game-kept"));
            ui.horizontal(|ui| {
                if ui.button(i18n.t("engine-retry")).clicked() {
                    session.engine_retry();
                }
                // "Trocar Engine": usa o caminho digitado no campo abaixo.
                if ui.button(i18n.t("engine-replace")).clicked()
                    && let Some(config) = state.config()
                {
                    session.engine_replace(config);
                }
            });
            ui.small(i18n.t("engine-replace-hint"));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::ffi::OsString;

    fn os(s: &str) -> Option<OsString> {
        Some(OsString::from(s))
    }

    #[test]
    fn explicit_variable_wins_over_path() {
        let found = find_engine(os("/opt/eng"), os("/usr/bin"), |_| true);
        assert_eq!(found, Some(PathBuf::from("/opt/eng")));
    }

    #[test]
    fn empty_variable_falls_back_to_path_search() {
        let found = find_engine(os(""), os("/a:/b"), |p| p == Path::new("/b/stockfish"));
        assert_eq!(found, Some(PathBuf::from("/b/stockfish")));
    }

    #[test]
    fn nothing_found_is_none() {
        assert_eq!(find_engine(None, os("/a:/b"), |_| false), None);
        assert_eq!(find_engine(None, None, |_| true), None);
    }
}
