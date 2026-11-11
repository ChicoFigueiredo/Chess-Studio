//! Seletor de arquivo do sistema (CX-20, #243): "Escolher arquivo…" e "Salvar como…".
//!
//! O diálogo nativo (portal do desktop no Linux, Windows, macOS) vem do crate `rfd` e é
//! **bloqueante**; por isso roda numa thread de fundo e entrega o resultado por quadro:
//! a interface chama [`FilePickButton::show_open`]/[`FilePickButton::show_save`] a cada quadro
//! e recebe o caminho escolhido quando ele chega. O trait [`FilePicker`] separa o diálogo real
//! de um seletor falso nos testes. Sem diálogo disponível (sem portal nem `zenity`, por
//! exemplo), o botão some e o campo de caminho digitável continua valendo.

use crate::i18n::I18n;
use crate::ui_theme::{ButtonVariant, cs_button};
use bevy_egui::egui;
use std::fmt;
use std::path::{Path, PathBuf};
use std::sync::mpsc::{self, Receiver, TryRecvError};
use std::sync::{Arc, Mutex};

/// Um filtro de extensões do diálogo (sem ponto: `["pgn", "gz"]`).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Filter {
    pub name: String,
    pub extensions: Vec<String>,
}

impl Filter {
    pub fn new(name: &str, extensions: &[&str]) -> Self {
        Self {
            name: name.to_owned(),
            extensions: extensions.iter().map(|e| (*e).to_owned()).collect(),
        }
    }
}

/// O que o diálogo deve fazer.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum PickKind {
    /// Escolher um arquivo existente.
    Open,
    /// Escolher onde gravar; o nome inicial sugerido vai em `file_name`.
    Save { file_name: String },
}

/// Pedido ao seletor.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PickRequest {
    pub kind: PickKind,
    pub title: String,
    pub filters: Vec<Filter>,
    /// Pasta em que o diálogo abre, se existir.
    pub start_dir: Option<PathBuf>,
}

/// Resultado entregue à interface.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum PickOutcome {
    Chosen(PathBuf),
    Cancelled,
    /// Não há diálogo neste sistema (ou ele falhou): a interface cai no campo de caminho.
    Unavailable,
}

/// O seletor em si. `pick` pode bloquear: quem chama o faz fora da thread da interface.
pub trait FilePicker: Send + Sync {
    /// Se há como abrir um diálogo neste sistema (verificação barata, sem abri-lo).
    fn available(&self) -> bool;
    fn pick(&self, request: PickRequest) -> PickOutcome;
}

/// Diálogo nativo via `rfd`.
#[derive(Debug, Default)]
pub struct SystemPicker;

impl FilePicker for SystemPicker {
    fn available(&self) -> bool {
        system_available()
    }

    fn pick(&self, request: PickRequest) -> PickOutcome {
        let mut dialog = rfd::FileDialog::new().set_title(request.title.as_str());
        for filter in &request.filters {
            dialog = dialog.add_filter(filter.name.as_str(), &filter.extensions);
        }
        if let Some(dir) = request.start_dir.as_deref().filter(|d| d.is_dir()) {
            dialog = dialog.set_directory(dir);
        }
        let chosen = match &request.kind {
            PickKind::Open => dialog.pick_file(),
            PickKind::Save { file_name } => dialog
                .set_file_name(file_name.as_str())
                .save_file()
                .map(|p| ensure_extension(p, request.filters.first())),
        };
        chosen.map_or(PickOutcome::Cancelled, PickOutcome::Chosen)
    }
}

/// "Salvar como" sem extensão ganha a do primeiro filtro (o GTK/portal não acrescenta).
fn ensure_extension(path: PathBuf, filter: Option<&Filter>) -> PathBuf {
    let Some(ext) = filter.and_then(|f| f.extensions.first()) else {
        return path;
    };
    if path.extension().is_some() {
        return path;
    }
    let mut name = path.into_os_string();
    name.push(".");
    name.push(ext);
    PathBuf::from(name)
}

#[cfg(any(target_os = "windows", target_os = "macos"))]
fn system_available() -> bool {
    true
}

#[cfg(not(any(target_os = "windows", target_os = "macos")))]
fn system_available() -> bool {
    let env = |k: &str| std::env::var_os(k);
    let path = env("PATH").unwrap_or_default();
    let has_zenity = std::env::split_paths(&path).any(|d| d.join("zenity").is_file());
    linux_available(&env, &|p| Path::new(p).exists(), has_zenity)
}

/// Linux: há diálogo se existe um barramento de sessão (portal) ou o `zenity` (reserva do `rfd`).
/// O `rfd` carrega a `libdbus` em tempo de execução, sem GTK nem dependência de compilação.
#[cfg_attr(any(target_os = "windows", target_os = "macos"), allow(dead_code))]
fn linux_available(
    env: &dyn Fn(&str) -> Option<std::ffi::OsString>,
    exists: &dyn Fn(&str) -> bool,
    has_zenity: bool,
) -> bool {
    if has_zenity || env("DBUS_SESSION_BUS_ADDRESS").is_some_and(|v| !v.is_empty()) {
        return true;
    }
    env("XDG_RUNTIME_DIR")
        .and_then(|d| d.to_str().map(|d| format!("{d}/bus")))
        .is_some_and(|p| exists(&p))
}

/// Botão com o seu próprio pedido em andamento: uma instância por campo de caminho.
pub struct FilePickButton {
    picker: Arc<dyn FilePicker>,
    pending: Option<Mutex<Receiver<PickOutcome>>>,
    unavailable: bool,
}

impl fmt::Debug for FilePickButton {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("FilePickButton")
            .field("busy", &self.pending.is_some())
            .field("unavailable", &self.unavailable)
            .finish()
    }
}

impl Default for FilePickButton {
    fn default() -> Self {
        Self::with_picker(Arc::new(SystemPicker))
    }
}

impl FilePickButton {
    pub fn with_picker(picker: Arc<dyn FilePicker>) -> Self {
        let unavailable = !picker.available();
        Self {
            picker,
            pending: None,
            unavailable,
        }
    }

    /// Há um diálogo aberto esperando o usuário.
    pub fn is_busy(&self) -> bool {
        self.pending.is_some()
    }

    /// Sem diálogo neste sistema: a interface mostra só o campo de caminho.
    pub fn is_unavailable(&self) -> bool {
        self.unavailable
    }

    /// Abre o diálogo numa thread de fundo; ignora o pedido se já há um aberto.
    pub fn request(&mut self, request: PickRequest) {
        if self.pending.is_some() || self.unavailable {
            return;
        }
        let (tx, rx) = mpsc::channel();
        let picker = Arc::clone(&self.picker);
        let spawned = std::thread::Builder::new()
            .name("file-picker".into())
            .spawn(move || {
                let _ = tx.send(picker.pick(request));
            });
        match spawned {
            Ok(_) => self.pending = Some(Mutex::new(rx)),
            Err(_) => self.unavailable = true,
        }
    }

    /// Consulta por quadro: `Some` uma única vez, quando o diálogo termina. Uma thread que
    /// morreu sem responder (falha do backend) conta como indisponível.
    pub fn poll(&mut self) -> Option<PickOutcome> {
        let rx = self.pending.as_ref()?;
        let received = rx
            .lock()
            .map_or(Err(TryRecvError::Disconnected), |rx| rx.try_recv());
        let outcome = match received {
            Ok(outcome) => outcome,
            Err(TryRecvError::Empty) => return None,
            Err(TryRecvError::Disconnected) => PickOutcome::Unavailable,
        };
        self.pending = None;
        if outcome == PickOutcome::Unavailable {
            self.unavailable = true;
        }
        Some(outcome)
    }

    /// "Escolher arquivo…": devolve o caminho no quadro em que o usuário confirma.
    pub fn show_open(
        &mut self,
        ui: &mut egui::Ui,
        i18n: &I18n,
        make: impl FnOnce() -> PickRequest,
    ) -> Option<PathBuf> {
        self.show(ui, i18n, "file-picker-open", make)
    }

    /// "Salvar como…": idem, para um destino a gravar.
    pub fn show_save(
        &mut self,
        ui: &mut egui::Ui,
        i18n: &I18n,
        make: impl FnOnce() -> PickRequest,
    ) -> Option<PathBuf> {
        self.show(ui, i18n, "file-picker-save", make)
    }

    fn show(
        &mut self,
        ui: &mut egui::Ui,
        i18n: &I18n,
        label_key: &str,
        make: impl FnOnce() -> PickRequest,
    ) -> Option<PathBuf> {
        let chosen = match self.poll() {
            Some(PickOutcome::Chosen(path)) => Some(path),
            _ => None,
        };
        if self.is_unavailable() {
            ui.small(i18n.t("file-picker-unavailable"));
            return chosen;
        }
        let busy = self.is_busy();
        if busy {
            ui.ctx()
                .request_repaint_after(std::time::Duration::from_millis(100));
        }
        let clicked = ui
            .add_enabled_ui(!busy, |ui| {
                cs_button(ui, &i18n.t(label_key), ButtonVariant::Secondary).clicked()
            })
            .inner;
        if busy {
            ui.small(i18n.t("file-picker-waiting"));
        } else if clicked {
            self.request(make());
        }
        chosen
    }
}

/// Pasta inicial do diálogo: a do caminho digitado, se existir; senão a de Documentos.
pub fn start_dir_for(typed: &str) -> Option<PathBuf> {
    let typed = typed.trim();
    if !typed.is_empty() {
        let parent = Path::new(typed).parent().filter(|p| p.is_dir());
        if let Some(parent) = parent {
            return Some(parent.to_path_buf());
        }
    }
    let docs = crate::paths::documents_dir(
        crate::paths::Platform::current(),
        &crate::paths::process_env,
    );
    docs.is_dir().then_some(docs)
}

/// Pedido de "Escolher arquivo…" para um PGN (`.pgn`, `.pgn.gz`).
pub fn pgn_open_request(i18n: &I18n, typed: &str) -> PickRequest {
    PickRequest {
        kind: PickKind::Open,
        title: i18n.t("file-picker-title-pgn-open"),
        filters: vec![Filter::new(
            &i18n.t("file-picker-filter-pgn"),
            &["pgn", "gz"],
        )],
        start_dir: start_dir_for(typed),
    }
}

/// Pedido de "Escolher arquivo…" para um CSV de problemas (`.csv`, `.csv.zst`).
pub fn puzzles_open_request(i18n: &I18n, typed: &str) -> PickRequest {
    PickRequest {
        kind: PickKind::Open,
        title: i18n.t("file-picker-title-puzzles"),
        filters: vec![Filter::new(
            &i18n.t("file-picker-filter-puzzles"),
            &["csv", "zst"],
        )],
        start_dir: start_dir_for(typed),
    }
}

/// Pedido de "Salvar como…" para um PGN, sugerindo `file_name`.
pub fn pgn_save_request(i18n: &I18n, file_name: String) -> PickRequest {
    PickRequest {
        kind: PickKind::Save { file_name },
        title: i18n.t("file-picker-title-pgn-save"),
        filters: vec![Filter::new(&i18n.t("file-picker-filter-pgn"), &["pgn"])],
        start_dir: start_dir_for(""),
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;
    use std::time::{Duration, Instant};

    struct Fake {
        available: bool,
        outcome: PickOutcome,
        seen: Mutex<Vec<PickRequest>>,
        gate: Mutex<Option<Receiver<()>>>,
    }

    impl Fake {
        fn new(available: bool, outcome: PickOutcome) -> Arc<Self> {
            Arc::new(Self {
                available,
                outcome,
                seen: Mutex::new(Vec::new()),
                gate: Mutex::new(None),
            })
        }
    }

    impl FilePicker for Fake {
        fn available(&self) -> bool {
            self.available
        }
        fn pick(&self, request: PickRequest) -> PickOutcome {
            self.seen.lock().unwrap().push(request);
            let gate = self.gate.lock().unwrap().take();
            if let Some(gate) = gate {
                let _ = gate.recv();
            }
            self.outcome.clone()
        }
    }

    fn open_request() -> PickRequest {
        PickRequest {
            kind: PickKind::Open,
            title: "Abrir".into(),
            filters: vec![Filter::new("PGN", &["pgn", "gz"])],
            start_dir: None,
        }
    }

    fn wait(button: &mut FilePickButton) -> PickOutcome {
        let limit = Instant::now() + Duration::from_secs(5);
        loop {
            if let Some(outcome) = button.poll() {
                return outcome;
            }
            assert!(Instant::now() < limit, "o seletor não respondeu");
            std::thread::sleep(Duration::from_millis(2));
        }
    }

    /// Manual: abre o diálogo real do sistema (`cargo test -- --ignored real_system_dialog`).
    #[test]
    #[ignore = "abre uma janela do sistema e espera o usuário"]
    fn real_system_dialog() {
        let picker = SystemPicker;
        assert!(picker.available());
        let outcome = picker.pick(PickRequest {
            kind: PickKind::Open,
            title: "Chess Studio: teste do seletor".into(),
            filters: vec![Filter::new("PGN", &["pgn", "gz"])],
            start_dir: start_dir_for(""),
        });
        eprintln!("RESULTADO DO DIALOGO: {outcome:?}");
    }

    #[test]
    fn delivers_the_chosen_path_once() {
        let fake = Fake::new(true, PickOutcome::Chosen(PathBuf::from("/x/a.pgn")));
        let mut button = FilePickButton::with_picker(fake.clone());
        button.request(open_request());
        assert_eq!(
            wait(&mut button),
            PickOutcome::Chosen(PathBuf::from("/x/a.pgn"))
        );
        assert_eq!(button.poll(), None);
        assert!(!button.is_busy());
        assert_eq!(fake.seen.lock().unwrap()[0], open_request());
    }

    #[test]
    fn cancelling_leaves_the_button_usable() {
        let fake = Fake::new(true, PickOutcome::Cancelled);
        let mut button = FilePickButton::with_picker(fake.clone());
        button.request(open_request());
        assert_eq!(wait(&mut button), PickOutcome::Cancelled);
        assert!(!button.is_unavailable());
        button.request(open_request());
        assert_eq!(wait(&mut button), PickOutcome::Cancelled);
        assert_eq!(fake.seen.lock().unwrap().len(), 2);
    }

    #[test]
    fn unavailable_picker_never_opens_and_flags_the_fallback() {
        let fake = Fake::new(false, PickOutcome::Cancelled);
        let mut button = FilePickButton::with_picker(fake.clone());
        assert!(button.is_unavailable());
        button.request(open_request());
        assert!(!button.is_busy());
        assert!(fake.seen.lock().unwrap().is_empty());
    }

    #[test]
    fn unavailable_outcome_switches_to_the_typed_path() {
        let fake = Fake::new(true, PickOutcome::Unavailable);
        let mut button = FilePickButton::with_picker(fake);
        button.request(open_request());
        assert_eq!(wait(&mut button), PickOutcome::Unavailable);
        assert!(button.is_unavailable());
    }

    #[test]
    fn a_second_request_is_ignored_while_the_dialog_is_open() {
        let fake = Fake::new(true, PickOutcome::Chosen(PathBuf::from("/x/a.pgn")));
        let (open, gate) = mpsc::channel();
        *fake.gate.lock().unwrap() = Some(gate);
        let mut button = FilePickButton::with_picker(fake.clone());
        button.request(open_request());
        assert!(button.is_busy());
        assert_eq!(button.poll(), None, "o diálogo ainda está aberto");
        button.request(open_request());
        open.send(()).unwrap();
        wait(&mut button);
        assert_eq!(fake.seen.lock().unwrap().len(), 1);
    }

    #[test]
    fn a_picker_thread_that_dies_counts_as_unavailable() {
        struct Panics;
        impl FilePicker for Panics {
            fn available(&self) -> bool {
                true
            }
            #[allow(clippy::panic)]
            fn pick(&self, _: PickRequest) -> PickOutcome {
                panic!("backend quebrado");
            }
        }
        let mut button = FilePickButton::with_picker(Arc::new(Panics));
        button.request(open_request());
        assert_eq!(wait(&mut button), PickOutcome::Unavailable);
        assert!(button.is_unavailable());
    }

    #[test]
    fn save_without_extension_gets_the_first_filter_extension() {
        let pgn = Filter::new("PGN", &["pgn"]);
        assert_eq!(
            ensure_extension(PathBuf::from("/x/partida"), Some(&pgn)),
            PathBuf::from("/x/partida.pgn")
        );
        assert_eq!(
            ensure_extension(PathBuf::from("/x/partida.txt"), Some(&pgn)),
            PathBuf::from("/x/partida.txt")
        );
        assert_eq!(
            ensure_extension(PathBuf::from("/x/partida"), None),
            PathBuf::from("/x/partida")
        );
    }

    #[test]
    fn linux_availability_follows_session_bus_or_zenity() {
        let none = |_: &str| None;
        let no_file = |_: &str| false;
        assert!(!linux_available(&none, &no_file, false));
        assert!(linux_available(&none, &no_file, true));
        let bus = |k: &str| (k == "DBUS_SESSION_BUS_ADDRESS").then(|| "unix:path=/b".into());
        assert!(linux_available(&bus, &no_file, false));
        let runtime = |k: &str| (k == "XDG_RUNTIME_DIR").then(|| "/run/user/1".into());
        assert!(linux_available(
            &runtime,
            &|p| p == "/run/user/1/bus",
            false
        ));
        assert!(!linux_available(&runtime, &no_file, false));
    }
}
