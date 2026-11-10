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
//! escolhida antes no assistente (B10, `engine_wizard`) ou do caminho digitado; tudo isso é
//! só **candidato**: nada executa sem consentimento (B25, `engine_consent`): a Engine
//! registrada e pronta no banco, a escolhida no assistente/gerenciador, ou o caminho cujo
//! aviso de execução o usuário confirmou e que passou na sonda UCI. Reabrir uma Partida
//! contra Engine não executa nada: o painel oferece "Conectar Engine e retomar". Sem
//! nenhuma, "Iniciar" abre o assistente de detecção/download.

use crate::engine_consent::{Consent, Intent, Request, Stage};
use crate::engine_manager::DbLocation;
use crate::i18n::I18n;
use crate::play::PlaySet;
use crate::session::Session;
use bevy::prelude::*;
use bevy_egui::{EguiContexts, EguiPrimaryContextPass, egui};
use chess_core::Color as Side;
use chess_game::EngineSeat;
use chess_uci::{EngineConfig, FailureKind, PlayerStatus, StrengthApplied};
use std::ffi::OsString;
use std::path::{Path, PathBuf};
use std::sync::mpsc::{self, Receiver, TryRecvError};
use std::sync::{Mutex, PoisonError};
use std::thread;

/// Variável de ambiente com o caminho do executável da Engine.
pub const ENGINE_ENV: &str = "CHESS_STUDIO_ENGINE";

pub struct EnginePlugin;

impl Plugin for EnginePlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<EngineUi>()
            .add_plugins(crate::engine_wizard::EngineWizardPlugin)
            .add_plugins(crate::engine_manager::EngineManagerPlugin)
            .add_systems(Update, (tick_engine, sync_trusted_default).in_set(PlaySet))
            .add_systems(EguiPrimaryContextPass, engine_window);
    }
}

/// Dá o pulso à Engine: nunca bloqueia (ela roda em thread própria).
fn tick_engine(mut session: ResMut<Session>) {
    // Leitura imutável primeiro: só toma `&mut` quando há Engine.
    if session.engine_active() {
        session.tick_engine();
    }
}

/// Quando o banco fica conhecido, o caminho candidato que já é Engine registrada e pronta
/// passa a valer sem novo aviso.
fn sync_trusted_default(mut ui: ResMut<EngineUi>, location: Option<Res<DbLocation>>) {
    if ui.db_path.is_some() {
        return;
    }
    let Some(location) = location else {
        return;
    };
    ui.db_path = Some(location.0.clone());
    let path = PathBuf::from(ui.path_text.trim());
    if !path.as_os_str().is_empty() && is_registered(ui.db_path.as_deref(), &path) {
        ui.consent.clear(&path);
    }
}

/// O caminho é de uma Engine registrada no banco, pronta e ainda existente?
pub fn is_registered(db: Option<&Path>, path: &Path) -> bool {
    let (Some(db), Some(text)) = (db, path.to_str()) else {
        return false;
    };
    path.is_file()
        && chess_store::Store::open(db)
            .ok()
            .and_then(|store| store.find_engine_by_path(text).ok().flatten())
            .is_some_and(|(_, record)| record.status == "ready")
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
    /// Consentimento para executar o caminho (B25).
    consent: Consent,
    /// Banco das Engines registradas, quando conhecido.
    db_path: Option<PathBuf>,
    /// Sonda UCI em andamento (thread), com o resultado do handshake.
    probe_rx: Option<Mutex<Receiver<Result<(), String>>>>,
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
            consent: Consent::default(),
            db_path: None,
            probe_rx: None,
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

    /// Lado do usuário contra a Engine (Opções > Engines).
    pub fn set_user_side(&mut self, side: Side) {
        self.user_side = side;
    }

    /// Elo da Engine adversária, dentro de `MIN_ELO..=MAX_ELO` (Opções > Engines).
    pub fn set_elo(&mut self, elo: u32) {
        self.elo = elo.clamp(MIN_ELO, MAX_ELO);
    }

    pub fn elo(&self) -> u32 {
        self.elo
    }

    /// Caminho escolhido no assistente ou no gerenciador de Engines (ambos já fizeram o
    /// handshake): vale como consentimento para executá-lo.
    pub fn set_path(&mut self, path: &Path) {
        self.path_text = path.display().to_string();
        self.not_found = false;
        self.consent.clear(path);
    }

    /// Opções UCI (threads, hash, nível…) a enviar à Engine ao iniciar.
    pub fn set_options(&mut self, options: Vec<(String, String)>) {
        self.options = options;
    }

    /// Consome o pedido de abrir o assistente.
    pub fn take_wizard_request(&mut self) -> bool {
        std::mem::take(&mut self.wizard_requested)
    }

    /// Há uma Engine liberada para executar (registrada, escolhida ou confirmada)?
    pub fn has_engine(&self) -> bool {
        self.engine_config().is_some()
    }

    /// Configuração da Engine a partir do caminho atual, só se ele estiver liberado: um
    /// caminho apenas digitado ou herdado do ambiente não vira processo sem passar pelo
    /// aviso e pela sonda (`engine_consent`).
    pub fn engine_config(&self) -> Option<EngineConfig> {
        let path = self.path_text.trim();
        self.consent
            .is_cleared(Path::new(path))
            .then(|| self.config_for(path))
            .flatten()
    }

    /// Caminho atual, se já liberado pelo consentimento (base da Engine padrão da análise).
    pub fn cleared_path(&self) -> Option<PathBuf> {
        let path = PathBuf::from(self.path_text.trim());
        (!path.as_os_str().is_empty() && self.consent.is_cleared(&path)).then_some(path)
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

    fn config_for(&self, text: &str) -> Option<EngineConfig> {
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

/// Executa a ação liberada: Partida nova, retomada da reaberta ou troca após falha.
fn apply_launch(session: &mut Session, state: &EngineUi, path: &Path, intent: Intent) {
    let Some(config) = state.config_for(&path.display().to_string()) else {
        return;
    };
    match intent {
        Intent::NewGame => {
            let seat = EngineSeat {
                color: state.user_side.opposite(),
                elo: state.elo,
            };
            session.start_engine_game(seat, config);
        }
        Intent::Resume => session.resume_engine(config),
        Intent::Replace => session.engine_replace(config),
    }
}

/// Pede a liberação do caminho atual para `intent` e, se já liberado, aplica na hora.
fn request_launch(session: &mut Session, state: &mut EngineUi, intent: Intent) {
    let db = state.db_path.clone();
    let text = state.path_text.clone();
    match state
        .consent
        .request(&text, intent, |p| is_registered(db.as_deref(), p))
    {
        Request::Ready(path, intent) => {
            state.not_found = false;
            apply_launch(session, state, &path, intent);
        }
        Request::Unconfigured => {
            state.not_found = true;
            state.wizard_requested = true;
        }
        Request::Pending => state.not_found = false,
    }
}

/// Recolhe o resultado da sonda e, se a Engine falou UCI, aplica a ação pendente.
fn poll_probe(session: &mut Session, state: &mut EngineUi) {
    let Some(rx) = &state.probe_rx else {
        return;
    };
    let result = match rx.lock().unwrap_or_else(PoisonError::into_inner).try_recv() {
        Ok(result) => result,
        Err(TryRecvError::Empty) => return,
        Err(TryRecvError::Disconnected) => Err("a sonda terminou sem resposta".to_owned()),
    };
    state.probe_rx = None;
    if let Some((path, intent)) = state.consent.probed(result) {
        apply_launch(session, state, &path, intent);
    }
}

/// Aviso de execução e andamento da sonda para um caminho ainda não validado.
fn consent_prompt(ui: &mut egui::Ui, i18n: &I18n, state: &mut EngineUi) {
    if let Some(error) = state.consent.error() {
        ui.colored_label(
            egui::Color32::LIGHT_RED,
            i18n.t_with("engine-probe-failed", &[("reason", error)]),
        );
    }
    let Some(launch) = state.consent.launch().cloned() else {
        return;
    };
    match launch.stage {
        Stage::Confirming => {
            ui.group(|ui| {
                ui.strong(i18n.t("engine-confirm-title"));
                ui.label(i18n.t_with(
                    "engine-confirm-body",
                    &[("path", &launch.path.display().to_string())],
                ));
                ui.horizontal(|ui| {
                    if ui.button(i18n.t("engine-confirm-run")).clicked()
                        && let Some(path) = state.consent.confirm()
                    {
                        let (tx, rx) = mpsc::channel();
                        state.probe_rx = Some(Mutex::new(rx));
                        thread::spawn(move || {
                            let result = chess_uci::probe_engine(&path, chess_uci::PROBE_TIMEOUT)
                                .map(|_| ())
                                .map_err(|err| err.to_string());
                            let _ = tx.send(result);
                        });
                    }
                    if ui.button(i18n.t("engine-confirm-cancel")).clicked() {
                        state.consent.cancel();
                    }
                });
            });
        }
        Stage::Probing => {
            ui.horizontal(|ui| {
                ui.spinner();
                ui.label(i18n.t("engine-probing"));
            });
            ui.ctx()
                .request_repaint_after(std::time::Duration::from_millis(100));
        }
    }
}

/// O que a Partida contra Engine exige na tela, sem painel esquerdo: o andamento da Engine
/// (pensando, falha com "Tentar de novo" e "Trocar Engine"), a retomada da Partida reaberta
/// ("Conectar Engine e retomar"), o aviso de execução e a sonda. A janela só existe enquanto
/// houver algo a mostrar; como as demais janelas que a Partida exige, continua no modo Foco.
/// Lado, Elo e assistente ficam nas Opções e na Nova Partida.
fn engine_window(
    mut contexts: EguiContexts,
    i18n: Res<I18n>,
    mut session: ResMut<Session>,
    mut state: ResMut<EngineUi>,
    capture: Option<Res<crate::capture::CaptureMode>>,
) -> Result {
    // O resultado da sonda precisa ser recolhido mesmo com a janela fechada.
    poll_probe(&mut session, &mut state);
    let pending = session.engine_pending().is_some();
    let busy = session.engine_active()
        || pending
        || state.consent.launch().is_some()
        || state.consent.error().is_some()
        || state.not_found;
    if !busy || capture.is_some_and(|c| !c.show_panel) {
        return Ok(());
    }
    let ctx = contexts.ctx_mut()?;
    let top = crate::ribbon::usable_rect(ctx).min;
    egui::Window::new(i18n.t("engine-heading"))
        .id(egui::Id::new("engine-status-window"))
        .collapsible(true)
        .resizable(false)
        .default_pos(top + egui::vec2(16.0, 16.0))
        .show(ctx, |ui| {
            if session.engine_active() {
                playing_status(ui, &i18n, &mut session, &mut state);
            }
            if pending {
                ui.colored_label(egui::Color32::YELLOW, i18n.t("engine-pending-title"));
                ui.small(i18n.t("engine-pending-hint"));
                if ui.button(i18n.t("engine-resume")).clicked() {
                    request_launch(&mut session, &mut state, Intent::Resume);
                }
            }
            consent_prompt(ui, &i18n, &mut state);
            if state.not_found {
                ui.colored_label(egui::Color32::YELLOW, i18n.t("engine-not-found"));
            }
        });
    Ok(())
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
                // "Trocar Engine": usa o caminho do campo abaixo, após o aviso e a sonda.
                if ui.button(i18n.t("engine-replace")).clicked() {
                    request_launch(session, state, Intent::Replace);
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
