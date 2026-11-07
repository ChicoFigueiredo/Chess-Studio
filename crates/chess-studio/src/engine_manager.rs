//! Gerenciador de Engines (C9, UCI-05, PEND-08): lista as Engines instaladas, detectadas e
//! escolhidas à mão; instala do catálogo, atualiza, remove com segurança e configura
//! threads, hash, nível e demais opções UCI anunciadas. Design de segurança em
//! `docs/decisoes/seguranca-engines.md`.
//!
//! A lógica (`EngineManager`) não depende do egui: instalação, detecção e sondagem rodam em
//! threads e devolvem mensagens por canal, que `poll` consome a cada quadro. Nada é baixado
//! nem executado sem o usuário confirmar a ação (o painel de confirmação mostra licença,
//! fonte e tamanho). Os registros ficam no `chess-store` (`engine_installations`).

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{self, Receiver, Sender, TryRecvError};
use std::sync::{Arc, Mutex, PoisonError};
use std::thread;

use bevy::prelude::*;
use bevy_egui::egui;
use chess_store::{EngineId, EngineOrigin, EngineRecord, Store};
use chess_uci::{
    Control, DetectedEngine, EngineOption, InstallError, InstallPlan, Installed, InstalledNetwork,
    Limits, OptionGroup, OptionSpec, PROBE_TIMEOUT, Progress, RemoveError,
};

use crate::engine::EngineUi;
use crate::engine_wizard::format_megabytes;
use crate::i18n::I18n;

/// Caminho do banco, para o gerenciador abrir a própria conexão (a gravação é por ação).
#[derive(Resource, Debug, Clone)]
pub struct DbLocation(pub PathBuf);

pub struct EngineManagerPlugin;

impl Plugin for EngineManagerPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<EngineManager>();
        crate::dock::register_panel(
            app,
            crate::dock::PanelDef {
                id: "Engines",
                label_key: "dock-panel-engines",
                access: crate::dock::PanelAccess::Always,
                ui: engines_panel,
            },
        );
    }
}

/// `Receiver` que pode viver num `Resource`.
struct Inbox<T>(Mutex<Receiver<T>>);

impl<T> Inbox<T> {
    fn try_recv(&self) -> Result<T, TryRecvError> {
        self.0
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .try_recv()
    }
}

/// Resultado de um trabalho de instalação/atualização.
pub struct JobDone {
    pub installed: Installed,
    pub network: Option<InstalledNetwork>,
    /// Atualização: a pasta da versão anterior foi apagada.
    pub old_removed: bool,
}

type Runner =
    Box<dyn FnOnce(&AtomicBool, &mut dyn FnMut(Progress)) -> Result<JobDone, InstallError> + Send>;

enum Msg {
    Step(Progress),
    Done(Result<JobDone, InstallError>),
    Detected(Vec<DetectedEngine>),
    Added(Result<DetectedEngine, String>),
    Options(EngineId, Result<Vec<EngineOption>, String>),
}

/// O que está sendo instalado/atualizado.
#[derive(Clone)]
struct JobKind {
    plan: InstallPlan,
    /// Atualização: o registro que será substituído.
    old: Option<EngineId>,
}

struct Running {
    kind: JobKind,
    progress: Progress,
}

/// Ação à espera de confirmação do usuário.
#[derive(Clone)]
pub enum Pending {
    Install { plan: InstallPlan, network: bool },
    Update { id: EngineId, plan: InstallPlan },
    Remove(EngineId),
}

/// Mensagem para o usuário: chave Fluent + detalhe técnico.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Notice {
    pub key: &'static str,
    pub detail: String,
    /// Código HTTP, quando o servidor recusou.
    pub code: String,
    pub error: bool,
}

impl Notice {
    fn info(key: &'static str) -> Self {
        Self {
            key,
            detail: String::new(),
            code: String::new(),
            error: false,
        }
    }

    fn error(key: &'static str, detail: impl ToString) -> Self {
        Self {
            key,
            detail: detail.to_string(),
            code: String::new(),
            error: true,
        }
    }
}

/// Linha da lista.
#[derive(Debug, Clone)]
pub struct Row {
    pub id: EngineId,
    pub record: EngineRecord,
    /// O executável ainda existe no disco.
    pub exists: bool,
    /// Há versão mais nova no catálogo embutido.
    pub update: Option<InstallPlan>,
}

/// Formulário de configuração gerado das opções anunciadas.
#[derive(Debug)]
pub struct ConfigForm {
    pub id: EngineId,
    pub name: String,
    pub specs: Vec<OptionSpec>,
    pub values: BTreeMap<String, String>,
    pub networks: Vec<PathBuf>,
    pub error: Option<String>,
}

/// Engine escolhida para jogar, à espera de ser entregue ao painel.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Chosen {
    pub path: PathBuf,
    pub options: Vec<(String, String)>,
}

/// Estado do gerenciador. É um `Resource`; a janela só aparece com `open`.
#[derive(Resource)]
pub struct EngineManager {
    open: bool,
    db_path: Option<PathBuf>,
    engines_dir: Option<PathBuf>,
    rows: Vec<Row>,
    tx: Sender<Msg>,
    rx: Inbox<Msg>,
    cancel: Arc<AtomicBool>,
    job: Option<Running>,
    pending: Option<Pending>,
    notice: Option<Notice>,
    detecting: bool,
    adding: bool,
    path_text: String,
    form: Option<ConfigForm>,
    chosen: Option<Chosen>,
    /// Instalar também a rede recomendada (Lc0) ao confirmar.
    with_network: bool,
}

impl std::fmt::Debug for EngineManager {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("EngineManager")
            .field("open", &self.open)
            .field("rows", &self.rows.len())
            .field("busy", &self.job.is_some())
            .finish_non_exhaustive()
    }
}

impl Default for EngineManager {
    fn default() -> Self {
        let (tx, rx) = mpsc::channel();
        Self {
            open: false,
            db_path: None,
            engines_dir: chess_uci::engines_dir(),
            rows: Vec::new(),
            tx,
            rx: Inbox(Mutex::new(rx)),
            cancel: Arc::new(AtomicBool::new(false)),
            job: None,
            pending: None,
            notice: None,
            detecting: false,
            adding: false,
            path_text: String::new(),
            form: None,
            chosen: None,
            with_network: true,
        }
    }
}

impl EngineManager {
    /// Gerenciador sobre um banco e um diretório de Engines explícitos (testes).
    #[cfg(test)]
    pub fn with_paths(db_path: Option<PathBuf>, engines_dir: Option<PathBuf>) -> Self {
        Self {
            db_path,
            engines_dir,
            ..Self::default()
        }
    }

    pub fn is_open(&self) -> bool {
        self.open
    }

    pub fn rows(&self) -> &[Row] {
        &self.rows
    }

    pub fn notice(&self) -> Option<&Notice> {
        self.notice.as_ref()
    }

    pub fn pending(&self) -> Option<&Pending> {
        self.pending.as_ref()
    }

    pub fn is_busy(&self) -> bool {
        self.job.is_some() || self.detecting || self.adding
    }

    pub fn form(&self) -> Option<&ConfigForm> {
        self.form.as_ref()
    }

    /// Garante a lista de Engines carregada (painéis que só escolhem uma Engine, como a
    /// Análise): aponta o banco, se ainda não apontado, e abre uma única vez.
    pub fn ensure_loaded(&mut self, db: Option<PathBuf>) {
        if self.db_path.is_none() {
            self.db_path = db;
        }
        if !self.open {
            self.open();
        }
    }

    /// Marca como aberto e atualiza a lista.
    pub fn open(&mut self) {
        self.open = true;
        self.refresh();
    }

    /// Escolha para jogar (entregue uma única vez).
    pub fn take_chosen(&mut self) -> Option<Chosen> {
        self.chosen.take()
    }

    fn store(&self) -> Option<Store> {
        Store::open(self.db_path.as_ref()?).ok()
    }

    fn no_db(&mut self) {
        self.notice = Some(Notice::error("manager-error-nodb", ""));
    }

    /// Relê o banco, adota pastas que o app instalou e ainda não estão registradas (por
    /// exemplo pelo assistente do P0) e confere se os executáveis existem.
    pub fn refresh(&mut self) {
        let Some(store) = self.store() else {
            self.rows.clear();
            return;
        };
        if let Some(dir) = self.engines_dir.clone() {
            adopt_installed_dirs(&store, &dir);
        }
        let mut rows = Vec::new();
        for (id, record) in store.list_engines().unwrap_or_default() {
            let exists = Path::new(&record.path).is_file();
            let wanted = if exists { "ready" } else { "missing" };
            if record.status != wanted {
                let _ = store.set_engine_status(id, wanted);
            }
            let update = (record.origin == EngineOrigin::Catalog)
                .then(|| {
                    chess_uci::update_available(
                        record.catalog_id.as_deref()?,
                        record.version.as_deref()?,
                        record.variant.as_deref().unwrap_or_default(),
                        std::env::consts::OS,
                        std::env::consts::ARCH,
                    )
                })
                .flatten();
            rows.push(Row {
                id,
                record: EngineRecord {
                    status: wanted.to_owned(),
                    ..record
                },
                exists,
                update,
            });
        }
        self.rows = rows;
    }

    // --- instalar / atualizar -------------------------------------------------------------

    /// Pede a confirmação de uma instalação (nada é baixado ainda).
    pub fn request_install(&mut self, plan: InstallPlan) {
        self.notice = None;
        self.with_network = plan.entry.networks.iter().any(|n| n.recommended);
        self.pending = Some(Pending::Install {
            plan,
            network: self.with_network,
        });
    }

    pub fn request_update(&mut self, id: EngineId) {
        self.notice = None;
        if let Some(plan) = self.rows.iter().find(|r| r.id == id).and_then(|r| r.update) {
            self.pending = Some(Pending::Update { id, plan });
        }
    }

    pub fn request_remove(&mut self, id: EngineId) {
        self.notice = None;
        self.pending = Some(Pending::Remove(id));
    }

    pub fn cancel_pending(&mut self) {
        self.pending = None;
    }

    /// O usuário confirmou: executa a ação pendente.
    pub fn confirm(&mut self) {
        let Some(pending) = self.pending.take() else {
            return;
        };
        match pending {
            Pending::Install { plan, network } => {
                let plan = InstallPlan {
                    network: network
                        .then(|| plan.entry.networks.iter().find(|n| n.recommended))
                        .flatten(),
                    ..plan
                };
                self.start_catalog_job(JobKind { plan, old: None });
            }
            Pending::Update { id, plan } => {
                self.start_catalog_job(JobKind {
                    plan,
                    old: Some(id),
                });
            }
            Pending::Remove(id) => self.remove(id),
        }
    }

    fn start_catalog_job(&mut self, kind: JobKind) {
        let Some(root) = self.engines_dir.clone() else {
            self.notice = Some(Notice::error("manager-error-nodir", ""));
            return;
        };
        let old_dir = kind.old.and_then(|id| {
            self.rows
                .iter()
                .find(|r| r.id == id)
                .and_then(|r| r.record.install_dir.clone())
                .map(PathBuf::from)
        });
        let plan = kind.plan;
        let is_update = kind.old.is_some();
        self.start_job(
            kind,
            Box::new(move |cancel, step| {
                // A rede neural vem antes: se falhar, nada da Engine foi tocado.
                let network = match plan.network {
                    Some(net) => Some(chess_uci::install_network_from_catalog(
                        net, &root, cancel, step,
                    )?),
                    None => None,
                };
                if is_update {
                    let done = chess_uci::update_from_catalog(
                        &plan,
                        &root,
                        old_dir.as_deref(),
                        cancel,
                        step,
                    )?;
                    Ok(JobDone {
                        installed: done.installed,
                        network,
                        old_removed: done.old_removed,
                    })
                } else {
                    let installed = chess_uci::install_from_catalog(&plan, &root, cancel, step)?;
                    Ok(JobDone {
                        installed,
                        network,
                        old_removed: false,
                    })
                }
            }),
        );
    }

    fn start_job(&mut self, kind: JobKind, runner: Runner) {
        if self.job.is_some() {
            return;
        }
        self.cancel = Arc::new(AtomicBool::new(false));
        let cancel = Arc::clone(&self.cancel);
        let tx = self.tx.clone();
        self.job = Some(Running {
            kind,
            progress: Progress::Verifying,
        });
        thread::spawn(move || {
            let step_tx = tx.clone();
            let result = runner(&cancel, &mut |p| {
                let _ = step_tx.send(Msg::Step(p));
            });
            let _ = tx.send(Msg::Done(result));
        });
    }

    /// Cancela o download em andamento.
    pub fn cancel_job(&self) {
        self.cancel.store(true, Ordering::Relaxed);
    }

    // --- remover ---------------------------------------------------------------------------

    /// Remove o registro e, só se o app instalou a pasta, a pasta (com as verificações de
    /// `chess_uci::remove_installed`). Engines detectadas ou escolhidas à mão permanecem
    /// intactas no disco.
    fn remove(&mut self, id: EngineId) {
        let Some(store) = self.store() else {
            return self.no_db();
        };
        let Some(row) = self.rows.iter().find(|r| r.id == id).cloned() else {
            return;
        };
        let app_dir = (row.record.origin == EngineOrigin::Catalog)
            .then(|| row.record.install_dir.clone())
            .flatten();
        if let Some(dir) = app_dir {
            let Some(root) = self.engines_dir.clone() else {
                self.notice = Some(Notice::error("manager-error-nodir", ""));
                return;
            };
            if let Err(err) = chess_uci::remove_installed(&root, Path::new(&dir)) {
                // Registro adulterado, link ou pasta que não é do app: nada é apagado.
                let key = match err {
                    RemoveError::Io(_) => "manager-error-remove-io",
                    _ => "manager-error-remove-refused",
                };
                self.notice = Some(Notice::error(key, err));
                return;
            }
        }
        match store.remove_engine(id) {
            Ok(()) => {
                self.notice = Some(Notice::info("manager-removed"));
                self.refresh();
            }
            Err(err) => self.notice = Some(Notice::error("manager-error-store", err)),
        }
    }

    // --- detectar / adicionar -------------------------------------------------------------------

    /// Procura Engines no computador (thread) e as registra como "detectadas".
    pub fn detect(&mut self) {
        if self.detecting {
            return;
        }
        self.detecting = true;
        self.notice = None;
        let tx = self.tx.clone();
        thread::spawn(move || {
            let _ = tx.send(Msg::Detected(chess_uci::detect_host()));
        });
    }

    /// Valida (handshake, em thread) e registra o executável digitado, como "manual".
    pub fn add_manual(&mut self) {
        let text = self.path_text.trim().to_owned();
        if text.is_empty() || self.adding {
            return;
        }
        self.adding = true;
        self.notice = None;
        let tx = self.tx.clone();
        thread::spawn(move || {
            let result =
                chess_uci::probe_engine(Path::new(&text), PROBE_TIMEOUT).map_err(|e| e.to_string());
            let _ = tx.send(Msg::Added(result));
        });
    }

    fn register(&mut self, engine: &DetectedEngine, origin: EngineOrigin) {
        let Some(store) = self.store() else {
            return self.no_db();
        };
        let path = engine.path.to_string_lossy().into_owned();
        // Quem já está registrado (inclusive instalada pelo app) não é rebaixado.
        if matches!(store.find_engine_by_path(&path), Ok(Some(_))) {
            return;
        }
        let record = EngineRecord {
            name: engine.display_name(),
            version: None,
            path,
            protocol: "uci".to_owned(),
            checksum: None,
            status: "ready".to_owned(),
            origin,
            catalog_id: None,
            variant: None,
            install_dir: None,
            settings: BTreeMap::new(),
            installed_at_ms: 0,
        };
        if let Err(err) = store.upsert_engine(&record) {
            self.notice = Some(Notice::error("manager-error-store", err));
        }
    }

    // --- configurar ------------------------------------------------------------------------------

    /// Abre o formulário: sonda a Engine (thread) para ler as opções anunciadas.
    pub fn open_config(&mut self, id: EngineId) {
        let Some(row) = self.rows.iter().find(|r| r.id == id) else {
            return;
        };
        self.notice = None;
        let path = PathBuf::from(&row.record.path);
        let networks = self
            .engines_dir
            .as_deref()
            .map(chess_uci::installed_networks)
            .unwrap_or_default();
        self.form = Some(ConfigForm {
            id,
            name: row.record.name.clone(),
            specs: Vec::new(),
            values: row.record.settings.clone(),
            networks,
            error: None,
        });
        self.adding = true;
        let tx = self.tx.clone();
        thread::spawn(move || {
            let result = chess_uci::probe_full(&path, PROBE_TIMEOUT)
                .map(|(_, options)| options)
                .map_err(|e| e.to_string());
            let _ = tx.send(Msg::Options(id, result));
        });
    }

    pub fn close_config(&mut self) {
        self.form = None;
    }

    /// Altera um valor do formulário (a validação ocorre ao salvar).
    pub fn set_value(&mut self, name: &str, value: String) {
        if let Some(form) = &mut self.form {
            form.values.insert(name.to_owned(), value);
            form.error = None;
        }
    }

    /// Volta uma opção ao padrão da Engine (remove a escolha).
    pub fn reset_value(&mut self, name: &str) {
        if let Some(form) = &mut self.form {
            form.values.remove(name);
        }
    }

    /// Valida tudo contra as opções anunciadas e os limites do app, e grava. Erro fica no
    /// formulário; nada é gravado se algo for inválido.
    pub fn save_config(&mut self) -> bool {
        let Some(form) = &mut self.form else {
            return false;
        };
        let weights = form.values.get("WeightsFile").cloned();
        let mut checked = form.values.clone();
        checked.remove("WeightsFile");
        let limits = Limits::for_host();
        if let Err(err) = chess_uci::resolve_settings(&form.specs, &limits, &checked) {
            form.error = Some(err.to_string());
            return false;
        }
        if let Some(path) = &weights {
            let ok = self
                .engines_dir
                .as_deref()
                .is_some_and(|dir| chess_uci::weights_option(dir, Path::new(path)).is_some());
            if !ok {
                form.error = Some("a rede neural precisa ser uma das baixadas pelo app".to_owned());
                return false;
            }
            checked.insert("WeightsFile".to_owned(), path.clone());
        }
        let id = form.id;
        let Some(store) = self.store() else {
            self.no_db();
            return false;
        };
        match store.set_engine_settings(id, &checked) {
            Ok(()) => {
                self.form = None;
                self.notice = Some(Notice::info("manager-config-saved"));
                self.refresh();
                true
            }
            Err(err) => {
                if let Some(form) = &mut self.form {
                    form.error = Some(err.to_string());
                }
                false
            }
        }
    }

    // --- usar ---------------------------------------------------------------------------------------

    /// Escolhe a Engine para jogar, com a configuração gravada (revalidada ao iniciar).
    pub fn use_engine(&mut self, id: EngineId) {
        let Some(row) = self.rows.iter().find(|r| r.id == id) else {
            return;
        };
        let options = chess_uci::launch_options(
            &row.record.settings,
            &Limits::for_host(),
            self.engines_dir.as_deref(),
        );
        self.chosen = Some(Chosen {
            path: PathBuf::from(&row.record.path),
            options,
        });
        self.notice = Some(Notice::info("manager-in-use"));
    }

    // --- mensagens das threads -------------------------------------------------------------------

    /// Consome as mensagens das threads; chamado a cada quadro.
    pub fn poll(&mut self) {
        while let Ok(msg) = self.rx.try_recv() {
            match msg {
                Msg::Step(step) => {
                    if let Some(job) = &mut self.job {
                        job.progress = step;
                    }
                }
                Msg::Done(result) => self.finish_job(result),
                Msg::Detected(found) => {
                    self.detecting = false;
                    for engine in &found {
                        self.register(engine, EngineOrigin::Detected);
                    }
                    self.notice = Some(Notice {
                        detail: found.len().to_string(),
                        ..Notice::info("manager-detected")
                    });
                    self.refresh();
                }
                Msg::Added(Ok(engine)) => {
                    self.adding = false;
                    self.register(&engine, EngineOrigin::Manual);
                    self.path_text.clear();
                    self.refresh();
                }
                Msg::Added(Err(detail)) => {
                    self.adding = false;
                    self.notice = Some(Notice::error("manager-error-not-uci", detail));
                }
                Msg::Options(id, result) => {
                    self.adding = false;
                    if let Some(form) = self.form.as_mut().filter(|f| f.id == id) {
                        match result {
                            Ok(options) => form.specs = chess_uci::parse_options(&options),
                            Err(detail) => form.error = Some(detail),
                        }
                    }
                }
            }
        }
    }

    fn finish_job(&mut self, result: Result<JobDone, InstallError>) {
        let Some(job) = self.job.take() else { return };
        let done = match result {
            Ok(done) => done,
            Err(InstallError::Cancelled) => {
                self.notice = Some(Notice::info("manager-cancelled"));
                return;
            }
            Err(err) => {
                // Qualquer falha deixa banco e disco como estavam (a versão anterior fica).
                let code = match &err {
                    InstallError::Http(code) => code.to_string(),
                    _ => String::new(),
                };
                self.notice = Some(Notice {
                    code,
                    ..Notice::error(install_error_key(&err), err)
                });
                return;
            }
        };
        let Some(store) = self.store() else {
            return self.no_db();
        };
        let plan = job.kind.plan;
        let mut settings = BTreeMap::new();
        // Atualização: a configuração do usuário segue para a nova versão.
        if let Some(old) = job.kind.old
            && let Ok(Some(previous)) = store.get_engine(old)
        {
            settings = previous.settings;
        }
        if let Some(net) = &done.network {
            settings.insert(
                "WeightsFile".to_owned(),
                net.path.to_string_lossy().into_owned(),
            );
        }
        let record = EngineRecord {
            name: if plan.asset.variant.is_empty() {
                format!("{} {}", plan.entry.name, plan.entry.version)
            } else {
                format!(
                    "{} {} — {}",
                    plan.entry.name, plan.entry.version, plan.asset.label
                )
            },
            version: Some(plan.entry.version.to_owned()),
            path: done.installed.engine.path.to_string_lossy().into_owned(),
            protocol: "uci".to_owned(),
            checksum: Some(done.installed.sha256.clone()),
            status: "ready".to_owned(),
            origin: EngineOrigin::Catalog,
            catalog_id: Some(plan.entry.id.to_owned()),
            variant: Some(plan.asset.variant.to_owned()),
            install_dir: Some(done.installed.dir.to_string_lossy().into_owned()),
            settings,
            installed_at_ms: 0,
        };
        if let Err(err) = store.upsert_engine(&record) {
            self.notice = Some(Notice::error("manager-error-store", err));
            return;
        }
        if let Some(old) = job.kind.old
            && done.old_removed
        {
            let _ = store.remove_engine(old);
        }
        self.notice = Some(Notice::info(if job.kind.old.is_some() {
            "manager-updated"
        } else {
            "manager-installed"
        }));
        self.refresh();
    }
}

fn install_error_key(error: &InstallError) -> &'static str {
    match error {
        InstallError::Network(_) => "wizard-error-network",
        InstallError::Http(_) => "wizard-error-http",
        InstallError::HashMismatch { .. } => "wizard-error-hash",
        InstallError::Io { .. } => "wizard-error-disk",
        InstallError::Archive(_) | InstallError::Handshake(_) => "wizard-error-invalid",
        InstallError::Cancelled => "manager-cancelled",
    }
}

/// Registra pastas com o marcador do app que o banco ainda não conhece.
fn adopt_installed_dirs(store: &Store, engines_dir: &Path) {
    let Ok(read) = std::fs::read_dir(engines_dir) else {
        return;
    };
    let known: Vec<String> = store
        .list_engines()
        .unwrap_or_default()
        .into_iter()
        .filter_map(|(_, r)| r.install_dir)
        .collect();
    for entry in read.flatten() {
        let dir = entry.path();
        let dir_text = dir.to_string_lossy().into_owned();
        let is_real_dir = std::fs::symlink_metadata(&dir)
            .map(|m| m.is_dir())
            .unwrap_or(false);
        if !is_real_dir
            || known.contains(&dir_text)
            || !dir.join(chess_uci::INSTALL_MARKER).is_file()
        {
            continue;
        }
        let name = entry.file_name().to_string_lossy().into_owned();
        for catalog in chess_uci::CATALOG {
            for asset in catalog.assets {
                if catalog.dir_name(asset) != name {
                    continue;
                }
                let exe = asset.executable.rsplit('/').next().unwrap_or_default();
                let path = dir.join(exe);
                if !path.is_file() {
                    continue;
                }
                let record = EngineRecord {
                    name: if asset.variant.is_empty() {
                        format!("{} {}", catalog.name, catalog.version)
                    } else {
                        format!("{} {} — {}", catalog.name, catalog.version, asset.label)
                    },
                    version: Some(catalog.version.to_owned()),
                    path: path.to_string_lossy().into_owned(),
                    protocol: "uci".to_owned(),
                    checksum: None,
                    status: "ready".to_owned(),
                    origin: EngineOrigin::Catalog,
                    catalog_id: Some(catalog.id.to_owned()),
                    variant: Some(asset.variant.to_owned()),
                    install_dir: Some(dir_text.clone()),
                    settings: BTreeMap::new(),
                    installed_at_ms: 0,
                };
                let _ = store.upsert_engine(&record);
                break;
            }
        }
    }
}

// --- interface -------------------------------------------------------------------------------------

/// Desenha o gerenciador dentro do painel "Engines" do dock.
fn engines_panel(ui: &mut egui::Ui, i18n: &I18n, world: &mut World) {
    let location = world.get_resource::<DbLocation>().cloned();
    world.resource_scope(|world, mut manager: Mut<EngineManager>| {
        let manager = &mut *manager;
        if manager.db_path.is_none()
            && let Some(location) = location
        {
            manager.db_path = Some(location.0.clone());
        }
        if !manager.is_open() {
            manager.open();
        }
        manager.poll();
        if let Some(chosen) = manager.take_chosen() {
            if let Some(dir) = chess_uci::engines_dir()
                && let Err(err) = chess_uci::save_preferred(&dir, &chosen.path)
            {
                warn!("não foi possível gravar a Engine preferida: {err}");
            }
            if let Some(mut engine_ui) = world.get_resource_mut::<EngineUi>() {
                engine_ui.set_path(&chosen.path);
                engine_ui.set_options(chosen.options);
            }
        }
        egui::ScrollArea::vertical().show(ui, |ui| manager_contents(ui, i18n, manager));
        if manager.is_busy() {
            ui.ctx()
                .request_repaint_after(std::time::Duration::from_millis(100));
        }
    });
}

fn origin_key(origin: EngineOrigin) -> &'static str {
    match origin {
        EngineOrigin::Catalog => "manager-origin-catalog",
        EngineOrigin::Detected => "manager-origin-detected",
        EngineOrigin::Manual => "manager-origin-manual",
    }
}

fn manager_contents(ui: &mut egui::Ui, i18n: &I18n, manager: &mut EngineManager) {
    ui.label(i18n.t("manager-intro"));
    if let Some(notice) = manager.notice().cloned() {
        let color = if notice.error {
            egui::Color32::LIGHT_RED
        } else {
            egui::Color32::LIGHT_GREEN
        };
        ui.colored_label(
            color,
            i18n.t_with(
                notice.key,
                &[("detail", &notice.detail), ("code", &notice.code)],
            ),
        );
        if notice.error && !notice.detail.is_empty() {
            ui.small(i18n.t_with("wizard-error-detail", &[("detail", &notice.detail)]));
        }
    }

    if manager.form().is_some() {
        config_form(ui, i18n, manager);
        return;
    }
    if let Some(progress) = manager.job.as_ref().map(|j| j.progress) {
        job_progress(ui, i18n, manager, progress);
        return;
    }
    if let Some(pending) = manager.pending().cloned() {
        confirmation(ui, i18n, manager, &pending);
        return;
    }
    installed_list(ui, i18n, manager);
    catalog_section(ui, i18n, manager);
    add_section(ui, i18n, manager);
}

fn installed_list(ui: &mut egui::Ui, i18n: &I18n, manager: &mut EngineManager) {
    ui.separator();
    ui.strong(i18n.t("manager-list-heading"));
    if manager.rows().is_empty() {
        ui.label(i18n.t("manager-list-empty"));
    }
    let mut action: Option<(EngineId, &'static str)> = None;
    for row in manager.rows() {
        ui.group(|ui| {
            ui.horizontal_wrapped(|ui| {
                ui.strong(&row.record.name);
                if let Some(v) = &row.record.version {
                    ui.label(i18n.t_with("manager-version", &[("version", v)]));
                }
                ui.label(i18n.t_with(
                    "manager-protocol",
                    &[("protocol", &row.record.protocol.to_uppercase())],
                ));
                ui.label(i18n.t(origin_key(row.record.origin)));
                let (state_key, color) = if row.exists {
                    ("manager-state-ready", egui::Color32::LIGHT_GREEN)
                } else {
                    ("manager-state-missing", egui::Color32::LIGHT_RED)
                };
                ui.colored_label(color, i18n.t(state_key));
            });
            ui.small(&row.record.path);
            if let Some(hash) = &row.record.checksum {
                ui.small(i18n.t_with("manager-checksum", &[("sha256", hash)]));
            }
            ui.horizontal(|ui| {
                if ui
                    .add_enabled(row.exists, egui::Button::new(i18n.t("manager-use")))
                    .clicked()
                {
                    action = Some((row.id, "use"));
                }
                if ui
                    .add_enabled(row.exists, egui::Button::new(i18n.t("manager-configure")))
                    .clicked()
                {
                    action = Some((row.id, "configure"));
                }
                if let Some(plan) = &row.update
                    && ui
                        .button(i18n.t_with("manager-update", &[("version", plan.entry.version)]))
                        .clicked()
                {
                    action = Some((row.id, "update"));
                }
                if ui.button(i18n.t("manager-remove")).clicked() {
                    action = Some((row.id, "remove"));
                }
            });
        });
    }
    match action {
        Some((id, "use")) => manager.use_engine(id),
        Some((id, "configure")) => manager.open_config(id),
        Some((id, "update")) => manager.request_update(id),
        Some((id, "remove")) => manager.request_remove(id),
        _ => {}
    }
}

fn catalog_section(ui: &mut egui::Ui, i18n: &I18n, manager: &mut EngineManager) {
    ui.separator();
    ui.strong(i18n.t("manager-catalog-heading"));
    let plans = chess_uci::plans_for(std::env::consts::OS, std::env::consts::ARCH);
    let mut chosen = None;
    for entry in chess_uci::CATALOG {
        let mine: Vec<_> = plans.iter().filter(|p| p.entry.id == entry.id).collect();
        ui.label(i18n.t_with(
            "manager-catalog-entry",
            &[("name", entry.name), ("version", entry.version)],
        ));
        if mine.is_empty() {
            ui.small(i18n.t_with(
                "manager-catalog-unavailable",
                &[
                    ("name", entry.name),
                    ("os", std::env::consts::OS),
                    ("arch", std::env::consts::ARCH),
                ],
            ));
            continue;
        }
        for plan in mine {
            let installed = manager.rows().iter().any(|r| {
                r.record.catalog_id.as_deref() == Some(entry.id)
                    && r.record.version.as_deref() == Some(entry.version)
                    && r.record.variant.as_deref().unwrap_or_default() == plan.asset.variant
            });
            ui.horizontal(|ui| {
                ui.label(plan.asset.label);
                ui.small(format_megabytes(plan.asset.size));
                if ui
                    .add_enabled(
                        !installed,
                        egui::Button::new(i18n.t(if installed {
                            "manager-installed-already"
                        } else {
                            "manager-install"
                        })),
                    )
                    .clicked()
                {
                    chosen = Some(*plan);
                }
            });
        }
    }
    if let Some(plan) = chosen {
        manager.request_install(plan);
    }
}

fn add_section(ui: &mut egui::Ui, i18n: &I18n, manager: &mut EngineManager) {
    ui.separator();
    ui.strong(i18n.t("manager-add-heading"));
    ui.horizontal(|ui| {
        if ui
            .add_enabled(
                !manager.is_busy(),
                egui::Button::new(i18n.t("manager-detect")),
            )
            .clicked()
        {
            manager.detect();
        }
        if manager.detecting {
            ui.spinner();
            ui.label(i18n.t("wizard-detecting"));
        }
    });
    ui.horizontal(|ui| {
        ui.add(
            egui::TextEdit::singleline(&mut manager.path_text)
                .hint_text(i18n.t("wizard-choose-hint"))
                .desired_width(320.0),
        );
        let can = !manager.is_busy() && !manager.path_text.trim().is_empty();
        if ui
            .add_enabled(can, egui::Button::new(i18n.t("manager-add-path")))
            .clicked()
        {
            manager.add_manual();
        }
    });
    ui.small(i18n.t("manager-add-warning"));
}

fn confirmation(ui: &mut egui::Ui, i18n: &I18n, manager: &mut EngineManager, pending: &Pending) {
    ui.separator();
    match pending {
        Pending::Install { plan, .. } | Pending::Update { plan, .. } => {
            let entry = plan.entry;
            ui.strong(i18n.t_with(
                "manager-confirm-heading",
                &[("name", entry.name), ("version", entry.version)],
            ));
            ui.label(i18n.t_with("manager-confirm-variant", &[("label", plan.asset.label)]));
            ui.label(i18n.t_with(
                "manager-confirm-license",
                &[("license", entry.license_name)],
            ));
            ui.horizontal(|ui| {
                ui.hyperlink_to(i18n.t("wizard-license-link"), entry.license_url);
                ui.hyperlink_to(
                    i18n.t_with("manager-source-link", &[("name", entry.name)]),
                    entry.source_url,
                );
            });
            ui.small(plan.asset.url());
            ui.label(i18n.t_with(
                "wizard-download-size",
                &[
                    ("file", plan.asset.file_name),
                    ("size", &format_megabytes(plan.asset.size)),
                ],
            ));
            if !plan.asset.note.is_empty() {
                ui.colored_label(egui::Color32::YELLOW, plan.asset.note);
            }
            if let Some(net) = entry.networks.iter().find(|n| n.recommended) {
                ui.checkbox(
                    &mut manager.with_network,
                    i18n.t_with(
                        "manager-confirm-network",
                        &[("name", net.name), ("size", &format_megabytes(net.size))],
                    ),
                );
                ui.small(net.url());
            }
            ui.small(i18n.t("wizard-download-network-notice"));
            let want_network = manager.with_network;
            ui.horizontal(|ui| {
                if ui.button(i18n.t("manager-confirm-accept")).clicked() {
                    manager.pending = Some(match pending {
                        Pending::Install { plan, .. } => Pending::Install {
                            plan: *plan,
                            network: want_network,
                        },
                        other => other.clone(),
                    });
                    manager.confirm();
                }
                if ui.button(i18n.t("wizard-download-cancel")).clicked() {
                    manager.cancel_pending();
                }
            });
        }
        Pending::Remove(id) => {
            let Some(row) = manager.rows().iter().find(|r| r.id == *id).cloned() else {
                manager.cancel_pending();
                return;
            };
            ui.strong(i18n.t_with("manager-remove-heading", &[("name", &row.record.name)]));
            let app_dir =
                row.record.origin == EngineOrigin::Catalog && row.record.install_dir.is_some();
            if app_dir {
                ui.label(i18n.t_with(
                    "manager-remove-app",
                    &[("dir", row.record.install_dir.as_deref().unwrap_or_default())],
                ));
            } else {
                ui.label(i18n.t_with("manager-remove-unregister", &[("path", &row.record.path)]));
            }
            ui.horizontal(|ui| {
                if ui.button(i18n.t("manager-remove-confirm")).clicked() {
                    manager.pending = Some(pending.clone());
                    manager.confirm();
                }
                if ui.button(i18n.t("wizard-download-cancel")).clicked() {
                    manager.cancel_pending();
                }
            });
        }
    }
}

fn job_progress(ui: &mut egui::Ui, i18n: &I18n, manager: &EngineManager, step: Progress) {
    ui.separator();
    let (text, fraction) = match step {
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
    };
    ui.add(egui::ProgressBar::new(fraction).text(text));
    if ui.button(i18n.t("wizard-download-cancel")).clicked() {
        manager.cancel_job();
    }
}

/// Rótulo do grupo do formulário.
fn group_key(group: OptionGroup) -> &'static str {
    match group {
        OptionGroup::Resources => "manager-group-resources",
        OptionGroup::Level => "manager-group-level",
        OptionGroup::Advanced => "manager-group-advanced",
    }
}

fn config_form(ui: &mut egui::Ui, i18n: &I18n, manager: &mut EngineManager) {
    ui.separator();
    let limits = Limits::for_host();
    let Some(form) = manager.form() else { return };
    ui.strong(i18n.t_with("manager-config-heading", &[("name", &form.name)]));
    if manager.adding {
        ui.horizontal(|ui| {
            ui.spinner();
            ui.label(i18n.t("wizard-checking"));
        });
    }
    let specs = form.specs.clone();
    let values = form.values.clone();
    let networks = form.networks.clone();
    let error = form.error.clone();
    let mut edits: Vec<(String, Option<String>)> = Vec::new();
    for group in [
        OptionGroup::Resources,
        OptionGroup::Level,
        OptionGroup::Advanced,
    ] {
        let members: Vec<&OptionSpec> = specs
            .iter()
            .filter(|s| chess_uci::group_of(&s.name) == group)
            .filter(|s| {
                // Arquivos arbitrários não são editáveis; botões não são configuração.
                !matches!(s.control, Control::Button)
                    && !(matches!(s.control, Control::Text { .. })
                        && chess_uci::is_path_like(&s.name))
            })
            .collect();
        if members.is_empty() {
            continue;
        }
        ui.add_space(6.0);
        ui.strong(i18n.t(group_key(group)));
        for spec in members {
            option_row(ui, i18n, spec, &limits, &values, &mut edits);
        }
    }
    // Rede neural do Lc0: só entre as baixadas pelo app.
    if specs.iter().any(|s| s.name == "WeightsFile") {
        ui.add_space(6.0);
        ui.strong(i18n.t("manager-group-network"));
        if networks.is_empty() {
            ui.small(i18n.t("manager-network-none"));
        }
        let current = values.get("WeightsFile").cloned().unwrap_or_default();
        for net in &networks {
            let path = net.to_string_lossy().into_owned();
            let label = net
                .file_name()
                .map(|n| n.to_string_lossy().into_owned())
                .unwrap_or_default();
            if ui.radio(current == path, label).clicked() {
                edits.push(("WeightsFile".to_owned(), Some(path)));
            }
        }
    }
    if let Some(error) = &error {
        ui.colored_label(
            egui::Color32::LIGHT_RED,
            i18n.t_with("manager-config-invalid", &[("detail", error)]),
        );
    }
    for (name, value) in edits {
        match value {
            Some(v) => manager.set_value(&name, v),
            None => manager.reset_value(&name),
        }
    }
    ui.add_space(8.0);
    ui.horizontal(|ui| {
        if ui.button(i18n.t("manager-config-save")).clicked() {
            manager.save_config();
        }
        if ui.button(i18n.t("wizard-download-cancel")).clicked() {
            manager.close_config();
        }
    });
}

fn option_row(
    ui: &mut egui::Ui,
    i18n: &I18n,
    spec: &OptionSpec,
    limits: &Limits,
    values: &BTreeMap<String, String>,
    edits: &mut Vec<(String, Option<String>)>,
) {
    let current = values.get(&spec.name);
    ui.horizontal(|ui| {
        ui.label(&spec.name);
        match &spec.control {
            Control::Spin { default, .. } => {
                let (min, max) = chess_uci::effective_bounds(spec, limits).unwrap_or((0, 0));
                let mut value: i64 = current
                    .and_then(|v| v.parse().ok())
                    .unwrap_or((*default).clamp(min, max));
                if ui.add(egui::Slider::new(&mut value, min..=max)).changed() {
                    edits.push((spec.name.clone(), Some(value.to_string())));
                }
            }
            Control::Check { default } => {
                let mut value = current.map_or(*default, |v| v == "true");
                if ui.checkbox(&mut value, "").changed() {
                    edits.push((spec.name.clone(), Some(value.to_string())));
                }
            }
            Control::Combo { default, vars } => {
                let selected = current.cloned().unwrap_or_else(|| default.clone());
                egui::ComboBox::from_id_salt(&spec.name)
                    .selected_text(&selected)
                    .show_ui(ui, |ui| {
                        for var in vars {
                            if ui.selectable_label(*var == selected, var).clicked() {
                                edits.push((spec.name.clone(), Some(var.clone())));
                            }
                        }
                    });
            }
            Control::Text { default } => {
                let mut value = current.cloned().unwrap_or_else(|| default.clone());
                if ui
                    .add(egui::TextEdit::singleline(&mut value).desired_width(200.0))
                    .changed()
                {
                    edits.push((spec.name.clone(), Some(value)));
                }
            }
            Control::Button => {}
        }
        if current.is_some() && ui.small_button(i18n.t("manager-config-default")).clicked() {
            edits.push((spec.name.clone(), None));
        }
    });
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;
    use std::time::{Duration, Instant};

    fn scratch(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("manager-ui-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    fn manager_in(name: &str) -> (EngineManager, PathBuf) {
        let base = scratch(name);
        let engines = base.join("engines");
        std::fs::create_dir_all(&engines).unwrap();
        (
            EngineManager::with_paths(Some(base.join("db.sqlite")), Some(engines)),
            base,
        )
    }

    fn poll_until(m: &mut EngineManager, done: impl Fn(&EngineManager) -> bool) {
        let deadline = Instant::now() + Duration::from_secs(10);
        while !done(m) {
            assert!(Instant::now() < deadline, "tempo esgotado: {m:?}");
            m.poll();
            thread::sleep(Duration::from_millis(5));
        }
    }

    /// Engine UCI mínima em shell, só para o handshake.
    #[cfg(unix)]
    fn script_engine(dir: &Path, name: &str) -> PathBuf {
        use std::os::unix::fs::PermissionsExt;
        let path = dir.join(name);
        std::fs::write(
            &path,
            "#!/bin/sh\nwhile read l; do case \"$l\" in\n\
             uci) echo 'id name Falsa 1'; \
             echo 'option name Threads type spin default 1 min 1 max 1024'; \
             echo 'option name Hash type spin default 16 min 1 max 33554432'; \
             echo 'option name Skill Level type spin default 20 min 0 max 20'; \
             echo uciok;;\n\
             isready) echo readyok;;\n quit) exit 0;;\nesac; done\n",
        )
        .unwrap();
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o755)).unwrap();
        path
    }

    fn catalog_record(dir: &Path, exe: &Path) -> EngineRecord {
        EngineRecord {
            name: "Stockfish 19".into(),
            version: Some("19".into()),
            path: exe.to_string_lossy().into_owned(),
            protocol: "uci".into(),
            checksum: Some("ab".repeat(32)),
            status: "ready".into(),
            origin: EngineOrigin::Catalog,
            catalog_id: Some("stockfish".into()),
            variant: Some(String::new()),
            install_dir: Some(dir.to_string_lossy().into_owned()),
            settings: BTreeMap::new(),
            installed_at_ms: 0,
        }
    }

    #[test]
    #[cfg(unix)]
    fn pastas_instaladas_pelo_app_sao_adotadas_e_o_resto_nao() {
        let (mut m, base) = manager_in("adopt");
        let engines = base.join("engines");
        // Pasta do app (com marcador), como o assistente do P0 deixa.
        let sf = engines.join("stockfish-19");
        std::fs::create_dir_all(&sf).unwrap();
        script_engine(&sf, "stockfish-linux-x86-64-universal");
        std::fs::write(sf.join(chess_uci::INSTALL_MARKER), "x").unwrap();
        // Pasta qualquer sem marcador: ignorada.
        let other = engines.join("stockfish-20");
        std::fs::create_dir_all(&other).unwrap();
        script_engine(&other, "stockfish-linux-x86-64-universal");

        m.refresh();
        assert_eq!(m.rows().len(), 1);
        let row = &m.rows()[0];
        assert_eq!(row.record.origin, EngineOrigin::Catalog);
        assert_eq!(row.record.catalog_id.as_deref(), Some("stockfish"));
        assert!(row.exists);
        assert!(row.update.is_none(), "19 é a versão do catálogo");
    }

    #[test]
    #[cfg(unix)]
    fn engine_manual_e_registrada_mas_remover_nao_apaga_o_arquivo() {
        let (mut m, base) = manager_in("manual");
        let exe = script_engine(&base, "stockfish-meu");
        m.path_text = exe.to_string_lossy().into_owned();
        m.add_manual();
        poll_until(&mut m, |m| !m.adding);
        assert_eq!(m.rows().len(), 1);
        assert_eq!(m.rows()[0].record.origin, EngineOrigin::Manual);
        let id = m.rows()[0].id;

        m.request_remove(id);
        m.confirm();
        assert!(m.rows().is_empty(), "registro removido");
        assert!(exe.is_file(), "o arquivo do usuário permanece");
    }

    #[test]
    #[cfg(unix)]
    fn arquivo_que_nao_e_engine_nao_e_registrado() {
        use std::os::unix::fs::PermissionsExt;
        let (mut m, base) = manager_in("not-uci");
        let exe = base.join("stockfish-falso");
        std::fs::write(&exe, "#!/bin/sh\nexit 0\n").unwrap();
        std::fs::set_permissions(&exe, std::fs::Permissions::from_mode(0o755)).unwrap();
        m.path_text = exe.to_string_lossy().into_owned();
        m.add_manual();
        poll_until(&mut m, |m| !m.adding);
        assert!(m.rows().is_empty());
        assert_eq!(m.notice().unwrap().key, "manager-error-not-uci");
    }

    #[test]
    #[cfg(unix)]
    fn remover_engine_do_app_apaga_so_a_pasta_dentro_de_engines() {
        let (mut m, base) = manager_in("remove-app");
        let dir = base.join("engines").join("stockfish-19");
        std::fs::create_dir_all(&dir).unwrap();
        let exe = script_engine(&dir, "stockfish-linux-x86-64-universal");
        std::fs::write(dir.join(chess_uci::INSTALL_MARKER), "x").unwrap();
        m.refresh();
        let id = m.rows()[0].id;
        assert_eq!(m.rows()[0].record.path, exe.to_string_lossy());

        m.request_remove(id);
        m.confirm();
        assert!(m.rows().is_empty());
        assert!(!dir.exists());
        assert!(base.join("engines").exists());
    }

    #[test]
    #[cfg(unix)]
    fn registro_adulterado_apontando_para_fora_nao_apaga_nada() {
        let (mut m, base) = manager_in("tampered");
        let precious = base.join("documentos");
        std::fs::create_dir_all(&precious).unwrap();
        std::fs::write(precious.join("tese.txt"), "importante").unwrap();
        // Mesmo com marcador, fora de `engines/` nada é apagado.
        std::fs::write(precious.join(chess_uci::INSTALL_MARKER), "x").unwrap();
        let store = Store::open(base.join("db.sqlite")).unwrap();
        let id = store
            .upsert_engine(&catalog_record(&precious, &precious.join("stockfish")))
            .unwrap();
        drop(store);
        m.refresh();

        m.request_remove(id);
        m.confirm();
        assert!(precious.join("tese.txt").exists());
        assert_eq!(m.notice().unwrap().key, "manager-error-remove-refused");
        assert_eq!(m.rows().len(), 1, "o registro permanece");
    }

    #[test]
    #[cfg(unix)]
    fn configuracao_valida_e_persistida_e_a_invalida_e_recusada() {
        let (mut m, base) = manager_in("config");
        let exe = script_engine(&base, "stockfish-meu");
        m.path_text = exe.to_string_lossy().into_owned();
        m.add_manual();
        poll_until(&mut m, |m| !m.adding);
        let id = m.rows()[0].id;

        m.open_config(id);
        poll_until(&mut m, |m| !m.adding);
        assert!(m.form().unwrap().specs.iter().any(|s| s.name == "Threads"));

        m.set_value("Threads", "9999".into());
        assert!(!m.save_config(), "acima do limite do app");
        assert!(m.form().unwrap().error.is_some());
        m.set_value("Threads", "1".into());
        m.set_value("Nao Anunciada", "1".into());
        assert!(!m.save_config(), "opção que a Engine não anunciou");
        m.reset_value("Nao Anunciada");
        m.set_value("Hash", "128".into());
        m.set_value("Skill Level", "5".into());
        assert!(m.save_config());
        assert!(m.form().is_none());

        // Reabre o gerenciador do zero: a configuração veio do banco.
        let mut again = EngineManager::with_paths(m.db_path.clone(), m.engines_dir.clone());
        again.refresh();
        let settings = &again.rows()[0].record.settings;
        assert_eq!(settings.get("Hash").map(String::as_str), Some("128"));
        assert_eq!(settings.get("Skill Level").map(String::as_str), Some("5"));

        again.use_engine(again.rows()[0].id);
        let chosen = again.take_chosen().unwrap();
        assert_eq!(chosen.path, exe);
        assert!(
            chosen
                .options
                .contains(&("Hash".to_owned(), "128".to_owned()))
        );
    }

    fn fake_done(base: &Path, dir_name: &str) -> JobDone {
        let dir = base.join("engines").join(dir_name);
        std::fs::create_dir_all(&dir).unwrap();
        #[cfg(unix)]
        let exe = script_engine(&dir, "lc0");
        #[cfg(not(unix))]
        let exe = dir.join("lc0");
        std::fs::write(dir.join(chess_uci::INSTALL_MARKER), "x").unwrap();
        JobDone {
            installed: Installed {
                dir,
                engine: DetectedEngine {
                    path: exe,
                    name: Some("Lc0".into()),
                    author: None,
                },
                options: Vec::new(),
                sha256: "cd".repeat(32),
            },
            network: None,
            old_removed: false,
        }
    }

    fn lc0_plan() -> InstallPlan {
        let entry = chess_uci::catalog_entry("lc0").unwrap();
        InstallPlan {
            entry,
            asset: &entry.assets[0],
            network: None,
        }
    }

    #[test]
    #[cfg(unix)]
    fn instalacao_concluida_registra_a_engine_e_falha_nao_muda_nada() {
        let (mut m, base) = manager_in("job");
        let plan = lc0_plan();
        let done = fake_done(&base, "lc0-0.32.1-cpu-openblas");
        m.start_job(
            JobKind { plan, old: None },
            Box::new(move |_, step| {
                step(Progress::Extracting);
                Ok(done)
            }),
        );
        poll_until(&mut m, |m| m.job.is_none());
        assert_eq!(m.rows().len(), 1);
        let rec = &m.rows()[0].record;
        assert_eq!(rec.origin, EngineOrigin::Catalog);
        assert_eq!(rec.catalog_id.as_deref(), Some("lc0"));
        assert_eq!(rec.variant.as_deref(), Some("cpu-openblas"));
        assert_eq!(rec.checksum.as_deref(), Some("cd".repeat(32).as_str()));
        assert_eq!(m.notice().unwrap().key, "manager-installed");

        m.start_job(
            JobKind { plan, old: None },
            Box::new(|_, _| {
                Err(InstallError::HashMismatch {
                    expected: "a".into(),
                    actual: "b".into(),
                })
            }),
        );
        poll_until(&mut m, |m| m.job.is_none());
        assert_eq!(m.rows().len(), 1, "nada novo foi registrado");
        assert_eq!(m.notice().unwrap().key, "wizard-error-hash");
        assert!(m.notice().unwrap().error);
    }

    #[test]
    #[cfg(unix)]
    fn atualizacao_leva_a_configuracao_e_falha_mantem_a_versao_anterior() {
        let (mut m, base) = manager_in("update");
        let plan = lc0_plan();
        // Versão antiga já registrada, com configuração.
        let old_done = fake_done(&base, "lc0-0.31.0-cpu-openblas");
        let mut old = catalog_record(&old_done.installed.dir, &old_done.installed.engine.path);
        old.name = "Lc0 0.31.0".into();
        old.version = Some("0.31.0".into());
        old.catalog_id = Some("lc0".into());
        old.variant = Some("cpu-openblas".into());
        old.settings.insert("Hash".into(), "64".into());
        let store = Store::open(base.join("db.sqlite")).unwrap();
        let old_id = store.upsert_engine(&old).unwrap();
        drop(store);
        m.refresh();

        // Falha: a antiga e sua configuração permanecem.
        m.start_job(
            JobKind {
                plan,
                old: Some(old_id),
            },
            Box::new(|_, _| Err(InstallError::Http(500))),
        );
        poll_until(&mut m, |m| m.job.is_none());
        assert_eq!(m.rows().len(), 1);
        assert_eq!(m.rows()[0].record.version.as_deref(), Some("0.31.0"));
        assert!(old_done.installed.dir.exists());

        // Sucesso: nova registrada com a configuração, antiga sai do banco.
        let mut done = fake_done(&base, "lc0-0.32.1-cpu-openblas");
        done.old_removed = true;
        m.start_job(
            JobKind {
                plan,
                old: Some(old_id),
            },
            Box::new(move |_, _| Ok(done)),
        );
        poll_until(&mut m, |m| m.job.is_none());
        assert_eq!(m.rows().len(), 1);
        let rec = &m.rows()[0].record;
        assert_eq!(rec.version.as_deref(), Some("0.32.1"));
        assert_eq!(rec.settings.get("Hash").map(String::as_str), Some("64"));
        assert_eq!(m.notice().unwrap().key, "manager-updated");
    }

    #[test]
    fn instalar_so_pede_confirmacao_e_nao_baixa_nada_antes() {
        let (mut m, _base) = manager_in("confirm");
        m.request_install(lc0_plan());
        assert!(matches!(m.pending(), Some(Pending::Install { .. })));
        assert!(!m.is_busy(), "nenhum trabalho começou sem confirmar");
        m.cancel_pending();
        assert!(m.pending().is_none());
        assert!(!m.is_busy());
    }

    #[test]
    fn todas_as_chaves_fluent_do_gerenciador_existem() {
        let i18n = I18n::pt_br();
        for key in [
            "manager-title",
            "manager-intro",
            "manager-list-heading",
            "manager-list-empty",
            "manager-version",
            "manager-protocol",
            "manager-checksum",
            "manager-origin-catalog",
            "manager-origin-detected",
            "manager-origin-manual",
            "manager-state-ready",
            "manager-state-missing",
            "manager-use",
            "manager-configure",
            "manager-update",
            "manager-remove",
            "manager-catalog-heading",
            "manager-catalog-entry",
            "manager-catalog-unavailable",
            "manager-install",
            "manager-installed-already",
            "manager-add-heading",
            "manager-detect",
            "manager-add-path",
            "manager-add-warning",
            "manager-confirm-heading",
            "manager-confirm-variant",
            "manager-confirm-license",
            "manager-confirm-network",
            "manager-confirm-accept",
            "manager-source-link",
            "manager-remove-heading",
            "manager-remove-app",
            "manager-remove-unregister",
            "manager-remove-confirm",
            "manager-group-resources",
            "manager-group-level",
            "manager-group-advanced",
            "manager-group-network",
            "manager-network-none",
            "manager-config-heading",
            "manager-config-invalid",
            "manager-config-save",
            "manager-config-default",
            "manager-config-saved",
            "manager-removed",
            "manager-installed",
            "manager-updated",
            "manager-cancelled",
            "manager-detected",
            "manager-in-use",
            "manager-error-nodb",
            "manager-error-nodir",
            "manager-error-store",
            "manager-error-not-uci",
            "manager-error-remove-refused",
            "manager-error-remove-io",
        ] {
            assert_ne!(i18n.t(key), key, "chave ausente: {key}");
        }
    }
}
