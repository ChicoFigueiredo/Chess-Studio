//! Assistente de Engine (B10, DEC-27/DEC-28, ADR-0003): detecta Engines UCI instaladas,
//! aceita um executável escolhido e, só com consentimento explícito, baixa o Stockfish
//! fixado. Abre quando o usuário quer jogar contra a Engine e nenhuma foi encontrada.
//!
//! A lógica (`Wizard`) não depende do egui: detecção, verificação e download rodam em
//! threads e devolvem mensagens por canal, que `poll` consome a cada quadro; assim a
//! interface nunca trava e o comportamento é testável. Sem Engine o app segue com a
//! Partida a dois e os Exercícios (UCI-04); a janela explica isso.

use crate::engine::EngineUi;
use crate::i18n::I18n;
use bevy::prelude::*;
use bevy_egui::{EguiContexts, EguiPrimaryContextPass, egui};
use chess_uci::{
    Asset, DetectedEngine, InstallError, InstallSpec, Installed, PROBE_TIMEOUT, Progress,
};
use std::path::Path;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{self, Receiver, TryRecvError};
use std::sync::{Mutex, PoisonError};
use std::thread;

pub struct EngineWizardPlugin;

impl Plugin for EngineWizardPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<EngineWizard>()
            .add_systems(EguiPrimaryContextPass, wizard_window);
    }
}

/// `Receiver` que pode viver num `Resource` (exige `Sync`): só `poll` o lê, no sistema do egui.
struct Inbox<T>(Mutex<Receiver<T>>);

impl<T> Inbox<T> {
    fn new(rx: Receiver<T>) -> Self {
        Self(Mutex::new(rx))
    }

    fn try_recv(&self) -> Result<T, TryRecvError> {
        self.0
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .try_recv()
    }
}

/// Mensagens da thread de instalação.
enum InstallMsg {
    Step(Progress),
    Done(Result<Installed, InstallError>),
}

/// O que o download mostra no momento (`None` em `EngineWizard::download`: nada pedido).
#[derive(Debug)]
enum Download {
    /// Licença, tamanho e botão de consentimento na tela.
    Confirming,
    Running(Progress),
    Failed(Failure),
}

/// Falha pronta para a tela: chave Fluent + detalhe técnico.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Failure {
    key: &'static str,
    code: Option<u16>,
    detail: String,
}

/// Chave Fluent do erro de instalação; `None` quando o usuário cancelou.
fn failure_of(error: &InstallError) -> Option<Failure> {
    let (key, code) = match error {
        InstallError::Network(_) => ("wizard-error-network", None),
        InstallError::Http(code) => ("wizard-error-http", Some(*code)),
        InstallError::HashMismatch { .. } => ("wizard-error-hash", None),
        InstallError::Io { .. } => ("wizard-error-disk", None),
        InstallError::Archive(_) | InstallError::Handshake(_) => ("wizard-error-invalid", None),
        InstallError::Cancelled => return None,
    };
    Some(Failure {
        key,
        code,
        detail: error.to_string(),
    })
}

/// "77,6 MB" (decimal, vírgula do pt-BR).
pub fn format_megabytes(bytes: u64) -> String {
    let tenths = (bytes + 50_000) / 100_000;
    format!("{},{} MB", tenths / 10, tenths % 10)
}

/// Estado do assistente. É um `Resource`; a janela só aparece com `open`.
#[derive(Resource, Default)]
pub struct EngineWizard {
    open: bool,
    detecting: bool,
    detected: Vec<DetectedEngine>,
    detect_rx: Option<Inbox<Vec<DetectedEngine>>>,
    path_text: String,
    probing: bool,
    probe_error: Option<String>,
    probe_rx: Option<Inbox<Result<DetectedEngine, String>>>,
    download: Option<Download>,
    install_rx: Option<Inbox<InstallMsg>>,
    cancel: Arc<AtomicBool>,
    /// Engine escolhida/instalada, à espera de ser entregue ao painel.
    chosen: Option<DetectedEngine>,
    /// Última escolha concluída, para a mensagem "Engine pronta".
    ready: Option<DetectedEngine>,
}

impl std::fmt::Debug for EngineWizard {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("EngineWizard")
            .field("open", &self.open)
            .field("detecting", &self.detecting)
            .field("detected", &self.detected.len())
            .field("download", &self.download)
            .finish_non_exhaustive()
    }
}

impl EngineWizard {
    pub fn is_open(&self) -> bool {
        self.open
    }

    /// Abre a janela e procura Engines numa thread (sem travar a interface).
    pub fn open(&mut self) {
        self.open_with(chess_uci::detect_host);
    }

    fn open_with(&mut self, detector: impl FnOnce() -> Vec<DetectedEngine> + Send + 'static) {
        if self.open {
            return;
        }
        self.open = true;
        self.ready = None;
        self.probe_error = None;
        self.detecting = true;
        self.detected.clear();
        let (tx, rx) = mpsc::channel();
        self.detect_rx = Some(Inbox::new(rx));
        thread::spawn(move || {
            let _ = tx.send(detector());
        });
    }

    /// Fecha a janela; um download em andamento é cancelado.
    pub fn close(&mut self) {
        self.cancel.store(true, Ordering::Relaxed);
        self.open = false;
        self.detect_rx = None;
        self.probe_rx = None;
        self.install_rx = None;
        self.probing = false;
        self.detecting = false;
        self.download = None;
    }

    /// Escolha concluída (entregue uma única vez).
    pub fn take_chosen(&mut self) -> Option<DetectedEngine> {
        self.chosen.take()
    }

    /// Escolhe uma Engine já validada.
    fn choose(&mut self, engine: DetectedEngine) {
        self.ready = Some(engine.clone());
        self.chosen = Some(engine);
        self.download = None;
    }

    /// Valida o executável digitado, numa thread.
    fn check_typed_path(&mut self) {
        let text = self.path_text.trim().to_owned();
        if text.is_empty() || self.probing {
            return;
        }
        self.probing = true;
        self.probe_error = None;
        let (tx, rx) = mpsc::channel();
        self.probe_rx = Some(Inbox::new(rx));
        thread::spawn(move || {
            let result = chess_uci::probe_engine(Path::new(&text), PROBE_TIMEOUT)
                .map_err(|err| err.to_string());
            let _ = tx.send(result);
        });
    }

    /// Pede o consentimento do download.
    fn request_download(&mut self) {
        self.download = Some(Download::Confirming);
    }

    /// Consentimento dado: baixa o asset para `<dados>/engines/` numa thread.
    fn start_download(&mut self, asset: &'static Asset) {
        let Some(root) = chess_uci::engines_dir() else {
            self.download = Some(Download::Failed(Failure {
                key: "wizard-error-nodir",
                code: None,
                detail: String::new(),
            }));
            return;
        };
        let spec = InstallSpec::from(asset);
        let dir_name = chess_uci::install_dir_name();
        self.start_job(
            Progress::Downloading {
                done: 0,
                total: Some(asset.size),
            },
            move |cancel, step| chess_uci::install(&spec, &root, &dir_name, cancel, step),
        );
    }

    fn start_job(
        &mut self,
        first: Progress,
        job: impl FnOnce(&AtomicBool, &mut dyn FnMut(Progress)) -> Result<Installed, InstallError>
        + Send
        + 'static,
    ) {
        self.cancel = Arc::new(AtomicBool::new(false));
        let cancel = Arc::clone(&self.cancel);
        let (tx, rx) = mpsc::channel();
        self.install_rx = Some(Inbox::new(rx));
        self.download = Some(Download::Running(first));
        thread::spawn(move || {
            let step_tx = tx.clone();
            let result = job(&cancel, &mut |p| {
                let _ = step_tx.send(InstallMsg::Step(p));
            });
            let _ = tx.send(InstallMsg::Done(result));
        });
    }

    /// Consome as mensagens das threads; chamado a cada quadro.
    pub fn poll(&mut self) {
        if let Some(rx) = &self.detect_rx {
            match rx.try_recv() {
                Ok(found) => {
                    self.detected = found;
                    self.detecting = false;
                    self.detect_rx = None;
                }
                Err(TryRecvError::Empty) => {}
                Err(TryRecvError::Disconnected) => {
                    self.detecting = false;
                    self.detect_rx = None;
                }
            }
        }
        if let Some(rx) = &self.probe_rx {
            match rx.try_recv() {
                Ok(Ok(engine)) => {
                    self.probing = false;
                    self.probe_rx = None;
                    self.choose(engine);
                }
                Ok(Err(detail)) => {
                    self.probing = false;
                    self.probe_rx = None;
                    self.probe_error = Some(detail);
                }
                Err(TryRecvError::Empty) => {}
                Err(TryRecvError::Disconnected) => {
                    self.probing = false;
                    self.probe_rx = None;
                }
            }
        }
        while let Some(rx) = &self.install_rx {
            match rx.try_recv() {
                Ok(InstallMsg::Step(step)) => {
                    if matches!(self.download, Some(Download::Running(_))) {
                        self.download = Some(Download::Running(step));
                    }
                }
                Ok(InstallMsg::Done(Ok(installed))) => {
                    self.install_rx = None;
                    self.choose(installed.engine);
                }
                Ok(InstallMsg::Done(Err(error))) => {
                    self.install_rx = None;
                    self.download = failure_of(&error).map(Download::Failed);
                }
                Err(TryRecvError::Empty) => break,
                Err(TryRecvError::Disconnected) => {
                    self.install_rx = None;
                    self.download = None;
                }
            }
        }
    }
}

/// Entrega a escolha ao painel e guarda a preferência.
fn deliver(wizard: &mut EngineWizard, engine_ui: &mut EngineUi) {
    if let Some(engine) = wizard.take_chosen() {
        if let Some(dir) = chess_uci::engines_dir()
            && let Err(err) = chess_uci::save_preferred(&dir, &engine.path)
        {
            warn!("não foi possível gravar a Engine preferida: {err}");
        }
        engine_ui.set_path(&engine.path);
    }
}

fn wizard_window(
    mut contexts: EguiContexts,
    i18n: Res<I18n>,
    mut wizard: ResMut<EngineWizard>,
    mut engine_ui: ResMut<EngineUi>,
) -> Result {
    let wizard = &mut *wizard;
    if engine_ui.take_wizard_request() {
        wizard.open();
    }
    if !wizard.is_open() {
        return Ok(());
    }
    wizard.poll();
    deliver(wizard, &mut engine_ui);
    let ctx = contexts.ctx_mut()?;
    let mut keep_open = true;
    egui::Window::new(i18n.t("wizard-title"))
        .collapsible(false)
        .resizable(false)
        .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
        .default_width(520.0)
        .open(&mut keep_open)
        .show(ctx, |ui| wizard_contents(ui, &i18n, wizard));
    if !keep_open {
        wizard.close();
    }
    Ok(())
}

fn path_label(path: &Path) -> String {
    path.display().to_string()
}

fn wizard_contents(ui: &mut egui::Ui, i18n: &I18n, wizard: &mut EngineWizard) {
    ui.label(i18n.t("wizard-intro"));
    ui.small(i18n.t("wizard-without-engine"));

    if let Some(ready) = wizard.ready.clone() {
        ui.separator();
        ui.colored_label(
            egui::Color32::LIGHT_GREEN,
            i18n.t_with("wizard-ready", &[("name", &ready.display_name())]),
        );
        ui.small(path_label(&ready.path));
        ui.label(i18n.t("wizard-ready-hint"));
        if ui.button(i18n.t("wizard-close")).clicked() {
            wizard.close();
        }
        return;
    }

    detected_section(ui, i18n, wizard);
    choose_section(ui, i18n, wizard);
    download_section(ui, i18n, wizard);

    ui.separator();
    if ui.button(i18n.t("wizard-continue-without")).clicked() {
        wizard.close();
    }
}

fn detected_section(ui: &mut egui::Ui, i18n: &I18n, wizard: &mut EngineWizard) {
    ui.separator();
    ui.strong(i18n.t("wizard-detected-heading"));
    if wizard.detecting {
        ui.horizontal(|ui| {
            ui.spinner();
            ui.label(i18n.t("wizard-detecting"));
        });
        return;
    }
    if wizard.detected.is_empty() {
        ui.label(i18n.t("wizard-none-detected"));
        return;
    }
    let mut picked = None;
    for engine in &wizard.detected {
        ui.horizontal(|ui| {
            ui.vertical(|ui| {
                ui.label(engine.display_name());
                if let Some(author) = &engine.author {
                    ui.small(i18n.t_with("wizard-engine-author", &[("author", author)]));
                }
                ui.small(path_label(&engine.path));
            });
            if ui.button(i18n.t("wizard-use")).clicked() {
                picked = Some(engine.clone());
            }
        });
    }
    if let Some(engine) = picked {
        wizard.choose(engine);
    }
}

fn choose_section(ui: &mut egui::Ui, i18n: &I18n, wizard: &mut EngineWizard) {
    ui.separator();
    ui.strong(i18n.t("wizard-choose-heading"));
    ui.horizontal(|ui| {
        ui.add(
            egui::TextEdit::singleline(&mut wizard.path_text)
                .hint_text(i18n.t("wizard-choose-hint"))
                .desired_width(300.0),
        );
        let can_check = !wizard.probing && !wizard.path_text.trim().is_empty();
        if ui
            .add_enabled(can_check, egui::Button::new(i18n.t("wizard-choose-button")))
            .clicked()
        {
            wizard.check_typed_path();
        }
    });
    if wizard.probing {
        ui.horizontal(|ui| {
            ui.spinner();
            ui.label(i18n.t("wizard-checking"));
        });
    }
    if let Some(detail) = &wizard.probe_error {
        ui.colored_label(
            egui::Color32::LIGHT_RED,
            i18n.t_with("wizard-check-failed", &[("detail", detail)]),
        );
    }
}

fn step_text(i18n: &I18n, step: Progress) -> (String, f32) {
    match step {
        Progress::Downloading {
            done,
            total: Some(total),
        } if total > 0 => (
            i18n.t_with(
                "wizard-step-downloading",
                &[
                    ("done", &format_megabytes(done)),
                    ("total", &format_megabytes(total)),
                ],
            ),
            (done as f32 / total as f32).clamp(0.0, 1.0),
        ),
        Progress::Downloading { done, .. } => (
            i18n.t_with(
                "wizard-step-downloading-unknown",
                &[("done", &format_megabytes(done))],
            ),
            0.0,
        ),
        Progress::Verifying => (i18n.t("wizard-step-verifying"), 1.0),
        Progress::Extracting => (i18n.t("wizard-step-extracting"), 1.0),
        Progress::Validating => (i18n.t("wizard-step-validating"), 1.0),
    }
}

fn download_section(ui: &mut egui::Ui, i18n: &I18n, wizard: &mut EngineWizard) {
    ui.separator();
    let version = chess_uci::STOCKFISH_VERSION;
    ui.strong(i18n.t_with("wizard-download-heading", &[("version", version)]));
    let Some(asset) = chess_uci::asset_for_host() else {
        ui.label(i18n.t_with(
            "wizard-download-unavailable",
            &[
                ("os", std::env::consts::OS),
                ("arch", std::env::consts::ARCH),
            ],
        ));
        return;
    };
    match wizard.download.take() {
        None => {
            if ui
                .button(i18n.t_with("wizard-download-button", &[("version", version)]))
                .clicked()
            {
                wizard.request_download();
            }
        }
        Some(Download::Confirming) => {
            wizard.download = Some(Download::Confirming);
            ui.label(i18n.t_with(
                "wizard-license",
                &[("license", chess_uci::STOCKFISH_LICENSE_NAME)],
            ));
            ui.horizontal(|ui| {
                ui.hyperlink_to(
                    i18n.t("wizard-license-link"),
                    chess_uci::STOCKFISH_LICENSE_URL,
                );
                ui.hyperlink_to(
                    i18n.t_with("wizard-source-link", &[("version", version)]),
                    chess_uci::STOCKFISH_SOURCE_URL,
                );
            });
            ui.label(i18n.t_with(
                "wizard-download-size",
                &[
                    ("file", asset.file_name),
                    ("size", &format_megabytes(asset.size)),
                ],
            ));
            ui.small(i18n.t("wizard-download-network-notice"));
            ui.horizontal(|ui| {
                if ui.button(i18n.t("wizard-download-confirm")).clicked() {
                    wizard.start_download(asset);
                }
                if ui.button(i18n.t("wizard-download-cancel")).clicked() {
                    wizard.download = None;
                }
            });
        }
        Some(Download::Running(step)) => {
            wizard.download = Some(Download::Running(step));
            let (text, fraction) = step_text(i18n, step);
            ui.add(egui::ProgressBar::new(fraction).text(text));
            if ui.button(i18n.t("wizard-download-cancel")).clicked() {
                wizard.cancel.store(true, Ordering::Relaxed);
            }
        }
        Some(Download::Failed(failure)) => {
            ui.colored_label(egui::Color32::LIGHT_RED, i18n.t("wizard-error-title"));
            let message = match failure.code {
                Some(code) => i18n.t_with(failure.key, &[("code", &code.to_string())]),
                None => i18n.t(failure.key),
            };
            ui.label(message);
            if !failure.detail.is_empty() {
                ui.small(i18n.t_with("wizard-error-detail", &[("detail", &failure.detail)]));
            }
            if ui.button(i18n.t("wizard-retry")).clicked() {
                wizard.request_download();
            } else {
                wizard.download = Some(Download::Failed(failure));
            }
        }
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;
    use std::path::PathBuf;
    use std::time::{Duration, Instant};

    fn has(i18n: &I18n, key: &str) -> bool {
        i18n.t(key) != key
    }

    fn fake_engine(name: &str) -> DetectedEngine {
        DetectedEngine {
            path: PathBuf::from(format!("/engines/{name}")),
            name: Some(name.to_owned()),
            author: None,
        }
    }

    /// Chama `poll` até a condição valer (as threads são rápidas; o prazo evita travar).
    fn poll_until(wizard: &mut EngineWizard, done: impl Fn(&EngineWizard) -> bool) {
        let deadline = Instant::now() + Duration::from_secs(5);
        while !done(wizard) {
            assert!(Instant::now() < deadline, "tempo esgotado: {wizard:?}");
            wizard.poll();
            thread::sleep(Duration::from_millis(5));
        }
    }

    #[test]
    fn detection_runs_off_thread_and_lists_results() {
        let mut wizard = EngineWizard::default();
        wizard.open_with(|| vec![fake_engine("A"), fake_engine("B")]);
        assert!(wizard.is_open());
        poll_until(&mut wizard, |w| !w.detecting);
        assert_eq!(wizard.detected.len(), 2);
    }

    #[test]
    fn using_a_detected_engine_hands_it_to_the_panel_once() {
        let mut wizard = EngineWizard::default();
        wizard.open_with(Vec::new);
        wizard.choose(fake_engine("A"));
        assert_eq!(
            wizard.take_chosen().unwrap().path,
            PathBuf::from("/engines/A")
        );
        assert!(wizard.take_chosen().is_none());
        assert!(wizard.ready.is_some());
    }

    #[test]
    fn download_progress_then_success_chooses_the_engine() {
        let mut wizard = EngineWizard::default();
        wizard.start_job(Progress::Verifying, |_, step| {
            step(Progress::Extracting);
            Ok(Installed {
                dir: PathBuf::from("/engines"),
                engine: fake_engine("Stockfish 19"),
                options: Vec::new(),
                sha256: String::new(),
            })
        });
        poll_until(&mut wizard, |w| w.chosen.is_some());
        assert_eq!(wizard.take_chosen().unwrap().display_name(), "Stockfish 19");
        assert!(wizard.download.is_none());
    }

    #[test]
    fn download_errors_become_clear_failures() {
        let mut wizard = EngineWizard::default();
        wizard.start_job(Progress::Verifying, |_, _| {
            Err(InstallError::HashMismatch {
                expected: "a".into(),
                actual: "b".into(),
            })
        });
        poll_until(&mut wizard, |w| {
            matches!(w.download, Some(Download::Failed(_)))
        });
        let Some(Download::Failed(failure)) = &wizard.download else {
            panic!("esperava falha");
        };
        assert_eq!(failure.key, "wizard-error-hash");
        assert!(wizard.chosen.is_none());
    }

    #[test]
    fn cancelling_returns_to_idle_without_error() {
        let mut wizard = EngineWizard::default();
        wizard.start_job(Progress::Verifying, |_, _| Err(InstallError::Cancelled));
        poll_until(&mut wizard, |w| w.install_rx.is_none());
        assert!(wizard.download.is_none());
    }

    #[test]
    fn every_error_kind_maps_to_an_existing_message() {
        let i18n = I18n::pt_br();
        let errors = [
            InstallError::Network("x".into()),
            InstallError::Http(503),
            InstallError::HashMismatch {
                expected: "a".into(),
                actual: "b".into(),
            },
            InstallError::Io {
                path: PathBuf::from("/x"),
                source: std::io::Error::other("cheio"),
            },
            InstallError::Archive("x".into()),
            InstallError::Handshake(chess_uci::EngineError::Terminated),
        ];
        for error in &errors {
            let failure = failure_of(error).unwrap();
            assert!(has(&i18n, failure.key), "falta {}", failure.key);
        }
        assert!(has(&i18n, "wizard-error-nodir"));
        assert!(failure_of(&InstallError::Cancelled).is_none());
    }

    #[test]
    fn every_wizard_message_used_in_the_source_exists() {
        let i18n = I18n::pt_br();
        let text = include_str!("engine_wizard.rs");
        let mut checked = 0;
        for literal in text.split('"').skip(1).step_by(2) {
            let is_id = literal.starts_with("wizard-")
                && !literal.ends_with('-')
                && literal
                    .chars()
                    .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-');
            if is_id {
                checked += 1;
                assert!(has(&i18n, literal), "falta `{literal}`");
            }
        }
        assert!(checked > 20, "poucas chaves verificadas: {checked}");
    }

    #[test]
    fn size_is_shown_in_decimal_megabytes_with_comma() {
        assert_eq!(format_megabytes(81_388_977), "81,4 MB");
        assert_eq!(format_megabytes(0), "0,0 MB");
    }
}
