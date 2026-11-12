//! Pacotes de Exercícios na tela de Treino (CX-5, #217): instalar de um arquivo de
//! problemas do Lichess (CSV, CC0), listar, remover e filtrar a lista por pacote, tema e
//! dificuldade. A leitura roda numa thread (arquivo com milhões de linhas), com progresso e
//! cancelamento; nenhuma chamada de rede (o usuário escolhe um arquivo que já tem).

use super::download::{self, DownloadUi};
use super::themes::theme_name;
use super::{Action, TrainingUi, View, with_store};
use crate::i18n::I18n;
use crate::persistence::PersistenceHandle;
use crate::ui_theme::{ButtonVariant, cs_button, cs_dropdown, cs_text_field};
use bevy::prelude::*;
use bevy_egui::egui;
use chess_training::{
    Catalog, ExerciseFilter, ImportProgress, ImportReport, PackInfo, PuzzleFilter,
    import_lichess_csv, install_pack, list_packs, load_packs, open_puzzle_file, remove_pack,
};
use std::collections::BTreeSet;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::Mutex;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{self, Receiver};

/// Quantidade máxima padrão ao importar.
const DEFAULT_MAX_COUNT: usize = 500;

/// Linhas de Exercício mostradas por seção (um pacote pode ter milhares).
pub(super) const ROWS_PER_SECTION: usize = 100;

pub(super) enum Cmd {
    StartImport,
    CancelImport,
    StartDownload,
    CancelDownload,
    AskRemove(String),
    ConfirmRemove(String),
    CancelRemove,
}

/// Filtro de pacote da lista.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Default)]
pub(super) enum PackChoice {
    #[default]
    All,
    Factory,
    Pack(String),
}

#[derive(Debug)]
enum JobMsg {
    Progress(ImportProgress),
    Done(Result<Outcome, String>),
}

#[derive(Debug)]
struct Outcome {
    info: Option<PackInfo>,
    report: ImportReport,
}

#[derive(Debug)]
struct Job {
    cancel: Arc<AtomicBool>,
    /// `Mutex` só para o recurso do Bevy ser `Sync`; quem lê é um sistema por vez.
    rx: Mutex<Receiver<JobMsg>>,
    progress: ImportProgress,
    /// Tamanho do `.csv` (para a barra); `None` em `.zst`, cujos bytes lidos são descomprimidos.
    total_bytes: Option<u64>,
}

/// Resultado da última instalação, mostrado até a próxima.
#[derive(Debug)]
enum Message {
    Done { name: String, report: ImportReport },
    Cancelled,
    Empty,
    Failed(String),
}

#[derive(Debug)]
pub(super) struct PacksUi {
    pub(super) dir: Option<PathBuf>,
    pub(super) installed: Vec<PackInfo>,
    load_errors: Vec<(String, String)>,
    path: String,
    pick: crate::file_picker::FilePickButton,
    /// Arquivos `.csv`/`.csv.zst` sugeridos (pasta de documentos), lidos uma vez.
    suggestions: Option<Vec<PathBuf>>,
    themes: String,
    min_rating: String,
    max_rating: String,
    max_count: String,
    job: Option<Job>,
    message: Option<Message>,
    confirm_remove: Option<String>,
    /// Baixar pacotes do Lichess pela rede (#238).
    pub(super) download: DownloadUi,
    /// Temas que aparecem nos Exercícios carregados (para o filtro da lista).
    pub(super) theme_ids: Vec<String>,
    // Filtro da lista de Exercícios.
    pub(super) filter_pack: PackChoice,
    filter_theme: String,
    filter_min: String,
    filter_max: String,
}

impl Default for PacksUi {
    fn default() -> Self {
        Self {
            dir: None,
            installed: Vec::new(),
            load_errors: Vec::new(),
            path: String::new(),
            pick: crate::file_picker::FilePickButton::default(),
            suggestions: None,
            themes: String::new(),
            min_rating: String::new(),
            max_rating: String::new(),
            max_count: DEFAULT_MAX_COUNT.to_string(),
            job: None,
            message: None,
            confirm_remove: None,
            download: DownloadUi::default(),
            theme_ids: Vec::new(),
            filter_pack: PackChoice::All,
            filter_theme: String::new(),
            filter_min: String::new(),
            filter_max: String::new(),
        }
    }
}

/// `.csv` e `.csv.zst` de uma pasta, em ordem de nome (no máximo 8).
fn list_puzzle_files(dir: &Path) -> Vec<PathBuf> {
    let Ok(read) = std::fs::read_dir(dir) else {
        return Vec::new();
    };
    let mut files: Vec<PathBuf> = read
        .filter_map(|e| e.ok().map(|e| e.path()))
        .filter(|p| {
            let name = p.to_string_lossy().to_lowercase();
            p.is_file() && (name.ends_with(".csv") || name.ends_with(".csv.zst"))
        })
        .collect();
    files.sort();
    files.truncate(8);
    files
}

fn parse_rating(text: &str) -> Option<u32> {
    text.trim().parse().ok()
}

impl PacksUi {
    /// Filtro atual da lista de Exercícios do Treino.
    pub(super) fn filter(&self) -> ExerciseFilter {
        ExerciseFilter {
            pack: match &self.filter_pack {
                PackChoice::All => None,
                PackChoice::Factory => Some(None),
                PackChoice::Pack(id) => Some(Some(id.clone())),
            },
            theme: self.filter_theme.clone(),
            min_rating: parse_rating(&self.filter_min),
            max_rating: parse_rating(&self.filter_max),
        }
    }

    pub(super) fn available(&self) -> bool {
        self.dir.is_some()
    }

    fn selection(&self) -> PuzzleFilter {
        PuzzleFilter {
            themes: self.themes.split_whitespace().map(str::to_owned).collect(),
            min_rating: parse_rating(&self.min_rating),
            max_rating: parse_rating(&self.max_rating),
            max_count: self
                .max_count
                .trim()
                .parse()
                .ok()
                .filter(|n| *n > 0)
                .unwrap_or(DEFAULT_MAX_COUNT),
        }
    }
}

/// Nome de arquivo sem `.zst` e `.csv`, só com `a-z0-9-` (id do pacote).
fn slug(path: &Path) -> String {
    let name = path
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("problemas")
        .to_lowercase();
    let name = name.strip_suffix(".zst").unwrap_or(&name);
    let name = name.strip_suffix(".csv").unwrap_or(name);
    let mut out = String::new();
    for c in name.chars() {
        let c = if c.is_ascii_alphanumeric() { c } else { '-' };
        if c != '-' || !out.ends_with('-') {
            out.push(c);
        }
    }
    let out = out.trim_matches('-');
    let out = if out.is_empty() { "problemas" } else { out };
    format!("lichess-{}", out.chars().take(48).collect::<String>())
}

/// Id livre entre os pacotes instalados (`-2`, `-3`… se o nome já existe).
pub(super) fn unique_id(base: String, installed: &[PackInfo]) -> String {
    if !installed.iter().any(|p| p.id == base) {
        return base;
    }
    (2..)
        .map(|n| format!("{base}-{n}"))
        .find(|id| !installed.iter().any(|p| &p.id == id))
        .unwrap_or(base)
}

impl TrainingUi {
    /// Relê os pacotes do disco e refaz o catálogo (fábrica + pacotes). Sem banco em disco
    /// não há pasta de pacotes: fica só o catálogo de fábrica.
    pub(super) fn reload_packs(&mut self, handle: Option<&PersistenceHandle>) {
        let dir = handle
            .and_then(|h| h.0.lock().ok())
            .and_then(|p| p.exercise_packs_dir());
        self.reload_packs_from(dir);
    }

    /// Como [`Self::reload_packs`], mas com a pasta já conhecida.
    pub(super) fn reload_packs_from(&mut self, dir: Option<PathBuf>) {
        let bundled = Catalog::bundled().unwrap_or_default();
        let mut catalog = bundled.clone();
        self.packs.installed.clear();
        self.packs.load_errors.clear();
        if let Some(dir) = &dir {
            self.packs.installed = list_packs(dir);
            let (loaded, errors) = load_packs(dir, &bundled);
            self.packs.load_errors = errors
                .into_iter()
                .map(|(id, err)| (id, err.to_string()))
                .collect();
            if let Err(err) = catalog.extend(loaded) {
                warn!("Treino: pacotes ignorados: {err}");
            }
        }
        self.packs.dir = dir;
        if self.packs.filter_pack != PackChoice::All
            && let PackChoice::Pack(id) = &self.packs.filter_pack
            && !self.packs.installed.iter().any(|p| &p.id == id)
        {
            self.packs.filter_pack = PackChoice::All;
        }
        let themes: BTreeSet<&str> = catalog
            .iter()
            .flat_map(|e| e.themes.iter().map(String::as_str))
            .collect();
        self.packs.theme_ids = themes.into_iter().map(str::to_owned).collect();
        self.catalog = catalog;
        // O banco precisa conhecer os Exercícios novos antes da próxima Resolução.
        self.synced = false;
    }

    fn start_import(&mut self) {
        let Some(dir) = self.packs.dir.clone() else {
            return;
        };
        let path = PathBuf::from(self.packs.path.trim());
        if self.packs.job.is_some() || path.as_os_str().is_empty() {
            return;
        }
        let filter = self.packs.selection();
        let id = unique_id(slug(&path), &self.packs.installed);
        let name = path
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("problemas")
            .to_owned();
        let total_bytes = (path.extension().is_none_or(|x| x != "zst"))
            .then(|| std::fs::metadata(&path).ok().map(|m| m.len()))
            .flatten();
        let cancel = Arc::new(AtomicBool::new(false));
        let (tx, rx) = mpsc::channel();
        let flag = Arc::clone(&cancel);
        std::thread::spawn(move || {
            let result = (|| {
                let reader = open_puzzle_file(&path).map_err(|e| e.to_string())?;
                let (exercises, report) = import_lichess_csv(reader, &filter, &flag, &mut |p| {
                    let _ = tx.send(JobMsg::Progress(*p));
                })
                .map_err(|e| e.to_string())?;
                if report.cancelled || exercises.is_empty() {
                    return Ok(Outcome { info: None, report });
                }
                let info = install_pack(&dir, &id, &format!("Lichess: {name}"), &exercises)
                    .map_err(|e| e.to_string())?;
                Ok(Outcome {
                    info: Some(info),
                    report,
                })
            })();
            let _ = tx.send(JobMsg::Done(result));
        });
        self.packs.message = None;
        self.packs.job = Some(Job {
            cancel,
            rx: Mutex::new(rx),
            progress: ImportProgress::default(),
            total_bytes,
        });
    }

    /// Consome o que a thread de leitura mandou; ao terminar, recarrega o catálogo.
    pub(super) fn poll_import(&mut self, handle: Option<&PersistenceHandle>) {
        let Some(job) = self.packs.job.as_mut() else {
            return;
        };
        let mut finished = None;
        let Ok(rx) = job.rx.get_mut() else {
            self.packs.job = None;
            return;
        };
        loop {
            match rx.try_recv() {
                Ok(JobMsg::Progress(p)) => job.progress = p,
                Ok(JobMsg::Done(result)) => finished = Some(result),
                Err(mpsc::TryRecvError::Empty) => break,
                // Thread que sumiu sem avisar (pânico): trata como falha.
                Err(mpsc::TryRecvError::Disconnected) => {
                    finished.get_or_insert_with(|| Err(String::from("leitura interrompida")));
                    break;
                }
            }
        }
        let Some(result) = finished else {
            return;
        };
        self.packs.job = None;
        self.packs.message = Some(match result {
            Err(reason) => Message::Failed(reason),
            Ok(Outcome { report, .. }) if report.cancelled => Message::Cancelled,
            Ok(Outcome { info: None, .. }) => Message::Empty,
            Ok(Outcome {
                info: Some(info),
                report,
            }) => {
                self.reload_packs_from(self.packs.dir.clone());
                self.sync_catalog(handle);
                Message::Done {
                    name: info.name,
                    report,
                }
            }
        });
    }

    fn remove_installed(&mut self, id: &str, handle: Option<&PersistenceHandle>) {
        if let Some(dir) = self.packs.dir.clone()
            && let Err(err) = remove_pack(&dir, id)
        {
            self.packs.message = Some(Message::Failed(err.to_string()));
            return;
        }
        let pack = id.to_owned();
        let done = with_store(&mut self.local, handle, |store, _| store.remove_pack(&pack));
        if let Some(Err(err)) = done {
            warn!("Treino: não foi possível remover o pacote do banco: {err}");
        }
        self.reload_packs_from(self.packs.dir.clone());
        self.sync_catalog(handle);
    }

    pub(super) fn apply_packs(&mut self, cmd: Cmd, handle: Option<&PersistenceHandle>) {
        match cmd {
            Cmd::StartImport => self.start_import(),
            Cmd::StartDownload => {}
            Cmd::CancelDownload => self.cancel_download(),
            Cmd::CancelImport => {
                if let Some(job) = &self.packs.job {
                    job.cancel.store(true, Ordering::Relaxed);
                }
            }
            Cmd::AskRemove(id) => self.packs.confirm_remove = Some(id),
            Cmd::CancelRemove => self.packs.confirm_remove = None,
            Cmd::ConfirmRemove(id) => {
                self.packs.confirm_remove = None;
                self.remove_installed(&id, handle);
            }
        }
    }
}

/// Sistema: acompanha a leitura em andamento.
pub(super) fn poll(mut state: ResMut<TrainingUi>, handle: Option<Res<PersistenceHandle>>) {
    if state.packs.job.is_some() {
        state.poll_import(handle.as_deref());
    }
}

/// Nome do Provedor para a pessoa ("lichess" no arquivo do pacote, "Lichess" na tela).
fn provider_display(provider: &str) -> String {
    let mut chars = provider.chars();
    chars.next().map_or_else(String::new, |first| {
        first.to_uppercase().chain(chars).collect()
    })
}

fn origin_line(i18n: &I18n, info: &PackInfo) -> Option<String> {
    let provider = provider_display(info.provider.as_deref()?);
    let license = info.license.as_deref().unwrap_or("?");
    Some(match info.downloaded_on.as_deref() {
        Some(day) => i18n.t_with(
            "training-packs-origin-dated",
            &[
                ("provider", &provider),
                ("license", license),
                ("date", &download::br_date(day)),
            ],
        ),
        None => i18n.t_with(
            "training-packs-origin",
            &[("provider", &provider), ("license", license)],
        ),
    })
}

/// Texto "Origem: lichess, licença CC0-1.0" de um pacote instalado.
pub(super) fn pack_origin(i18n: &I18n, packs: &PacksUi, id: &str) -> Option<String> {
    packs
        .installed
        .iter()
        .find(|p| p.id == id)
        .and_then(|p| origin_line(i18n, p))
}

pub(super) fn pack_name<'a>(packs: &'a PacksUi, id: &'a str) -> &'a str {
    packs
        .installed
        .iter()
        .find(|p| p.id == id)
        .map_or(id, |p| p.name.as_str())
}

/// Filtros da lista: pacote, tema e faixa de dificuldade.
/// Reserva um espaço de largura e altura fixas para um campo, para os campos da mesma linha
/// ficarem alinhados (sem "escada" quando a linha quebra).
fn field_slot(ui: &mut egui::Ui, width: f32, add: impl FnOnce(&mut egui::Ui)) {
    let size = egui::vec2(width, ui.spacing().interact_size.y);
    ui.allocate_ui_with_layout(size, egui::Layout::left_to_right(egui::Align::Center), add);
}

pub(super) fn filter_bar(ui: &mut egui::Ui, i18n: &I18n, packs: &mut PacksUi) {
    let mut options = vec![
        (PackChoice::All, i18n.t("training-filter-pack-all")),
        (PackChoice::Factory, i18n.t("training-filter-pack-factory")),
    ];
    options.extend(
        packs
            .installed
            .iter()
            .map(|p| (PackChoice::Pack(p.id.clone()), p.name.clone())),
    );
    cs_dropdown(ui, "training-filter-pack", &mut packs.filter_pack, &options);
    ui.horizontal_wrapped(|ui| {
        ui.set_max_width(ui.available_width());
        if !packs.theme_ids.is_empty() || !packs.filter_theme.is_empty() {
            let mut named: Vec<(String, String)> = packs
                .theme_ids
                .iter()
                .map(|id| (id.clone(), theme_name(i18n, id)))
                .collect();
            named.sort_by_key(|(_, name)| name.to_lowercase());
            let mut options = vec![(String::new(), i18n.t("training-filter-theme-any"))];
            options.extend(named);
            if !options.iter().any(|(id, _)| *id == packs.filter_theme) {
                packs.filter_theme.clear();
            }
            cs_dropdown(
                ui,
                "training-filter-theme",
                &mut packs.filter_theme,
                &options,
            );
        }
        field_slot(ui, 170.0, |ui| {
            cs_text_field(
                ui,
                &mut packs.filter_min,
                &i18n.t("training-filter-min-hint"),
                None,
            );
        });
        field_slot(ui, 170.0, |ui| {
            cs_text_field(
                ui,
                &mut packs.filter_max,
                &i18n.t("training-filter-max-hint"),
                None,
            );
        });
    });
}

fn message_line(ui: &mut egui::Ui, i18n: &I18n, message: &Message) {
    match message {
        Message::Done { name, report } => {
            ui.label(i18n.t_with(
                "training-packs-done",
                &[
                    ("name", name),
                    ("imported", &report.imported.to_string()),
                    ("invalid", &report.invalid.to_string()),
                    ("filtered", &report.filtered_out.to_string()),
                ],
            ));
        }
        Message::Cancelled => {
            ui.label(i18n.t("training-packs-cancelled"));
        }
        Message::Empty => {
            ui.label(i18n.t("training-packs-empty"));
        }
        Message::Failed(reason) => {
            ui.label(i18n.t_with("training-packs-error", &[("reason", reason)]));
        }
    }
}

/// Tela Treino > Pacotes.
pub(super) fn view(ui: &mut egui::Ui, i18n: &I18n, packs: &mut PacksUi) -> Option<Action> {
    let mut action = None;
    ui.strong(i18n.t("training-packs-heading"));
    if !packs.available() {
        ui.label(i18n.t("training-packs-unavailable"));
        if cs_button(ui, &i18n.t("training-back"), ButtonVariant::Secondary).clicked() {
            action = Some(Action::Show(View::List));
        }
        return action;
    }

    let mut cmd = None;
    if let Some(chosen) = download::card(ui, i18n, &mut packs.download) {
        cmd = Some(chosen);
    }
    ui.add_space(8.0);

    ui.label(egui::RichText::new(i18n.t("training-packs-installed")).strong());
    if packs.installed.is_empty() {
        ui.label(i18n.t("training-packs-none"));
    }
    for info in &packs.installed {
        ui.separator();
        ui.label(i18n.t_with(
            "training-packs-entry",
            &[("name", &info.name), ("count", &info.count.to_string())],
        ));
        if let Some(origin) = origin_line(i18n, info) {
            ui.small(origin);
        }
        if packs.confirm_remove.as_deref() == Some(info.id.as_str()) {
            ui.label(i18n.t_with("training-packs-remove-confirm", &[("name", &info.name)]));
            ui.horizontal(|ui| {
                if cs_button(
                    ui,
                    &i18n.t("training-packs-remove-yes"),
                    ButtonVariant::Danger,
                )
                .clicked()
                {
                    cmd = Some(Cmd::ConfirmRemove(info.id.clone()));
                }
                if cs_button(
                    ui,
                    &i18n.t("training-packs-remove-no"),
                    ButtonVariant::Secondary,
                )
                .clicked()
                {
                    cmd = Some(Cmd::CancelRemove);
                }
            });
        } else if cs_button(ui, &i18n.t("training-packs-remove"), ButtonVariant::Danger).clicked() {
            cmd = Some(Cmd::AskRemove(info.id.clone()));
        }
    }
    for (id, reason) in &packs.load_errors {
        ui.small(i18n.t_with(
            "training-packs-load-error",
            &[("id", id), ("reason", reason)],
        ));
    }

    ui.separator();
    ui.strong(i18n.t("training-packs-import-heading"));
    ui.small(i18n.t("training-packs-import-note"));
    cs_text_field(
        ui,
        &mut packs.path,
        &i18n.t("training-packs-path-hint"),
        None,
    );
    let typed = packs.path.clone();
    if let Some(file) = packs.pick.show_open(ui, i18n, || {
        crate::file_picker::puzzles_open_request(i18n, &typed)
    }) {
        packs.path = file.display().to_string();
    }
    let files = packs.suggestions.get_or_insert_with(|| {
        let dir = crate::paths::documents_dir(
            crate::paths::Platform::current(),
            &crate::paths::process_env,
        );
        list_puzzle_files(&dir)
    });
    if !files.is_empty() {
        ui.small(i18n.t("training-packs-suggestions"));
        for file in files.clone() {
            let name = file
                .file_name()
                .map_or_else(String::new, |n| n.to_string_lossy().into_owned());
            if ui.small_button(name).clicked() {
                packs.path = file.display().to_string();
            }
        }
    }
    cs_text_field(
        ui,
        &mut packs.themes,
        &i18n.t("training-packs-theme-hint"),
        None,
    );
    ui.horizontal_wrapped(|ui| {
        for (text, key) in [
            (&mut packs.min_rating, "training-filter-min-hint"),
            (&mut packs.max_rating, "training-filter-max-hint"),
            (&mut packs.max_count, "training-packs-count-hint"),
        ] {
            field_slot(ui, 160.0, |ui| {
                cs_text_field(ui, text, &i18n.t(key), None);
            });
        }
    });
    if let Some(job) = &packs.job {
        ui.ctx().request_repaint();
        ui.label(i18n.t_with(
            "training-packs-progress",
            &[
                ("lines", &job.progress.lines.to_string()),
                ("imported", &job.progress.imported.to_string()),
            ],
        ));
        if let Some(total) = job.total_bytes.filter(|t| *t > 0) {
            ui.add(
                egui::ProgressBar::new((job.progress.bytes as f32 / total as f32).min(1.0))
                    .desired_width(f32::INFINITY),
            );
        }
        if cs_button(
            ui,
            &i18n.t("training-packs-cancel"),
            ButtonVariant::Secondary,
        )
        .clicked()
        {
            cmd = Some(Cmd::CancelImport);
        }
    } else if ui
        .add_enabled_ui(!packs.path.trim().is_empty(), |ui| {
            cs_button(
                ui,
                &i18n.t("training-packs-install"),
                ButtonVariant::Primary,
            )
        })
        .inner
        .clicked()
    {
        cmd = Some(Cmd::StartImport);
    }
    if let Some(message) = &packs.message {
        message_line(ui, i18n, message);
    }

    ui.separator();
    if cs_button(ui, &i18n.t("training-back"), ButtonVariant::Secondary).clicked() {
        action = Some(Action::Show(View::List));
    }
    cmd.map(Action::Packs).or(action)
}

#[cfg(test)]
mod tests {
    use super::*;
    use chess_training::ExerciseKind;

    const BEFORE: &str = "6k1/1p3ppp/8/8/8/8/5PPP/R5K1 b - - 0 1";

    fn temp(name: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!("cs-ui-packs-{name}-{}", std::process::id()));
        std::fs::remove_dir_all(&d).ok();
        std::fs::create_dir_all(&d).unwrap();
        d
    }

    fn wait(state: &mut TrainingUi) {
        for _ in 0..2000 {
            state.poll_import(None);
            if state.packs.job.is_none() {
                return;
            }
            std::thread::sleep(std::time::Duration::from_millis(5));
        }
        panic!("a leitura não terminou");
    }

    #[test]
    fn install_filter_and_remove_a_lichess_pack_end_to_end() {
        let root = temp("e2e");
        let csv = root.join("Meus Problemas.csv");
        let mut text = String::from(
            "PuzzleId,FEN,Moves,Rating,RatingDeviation,Popularity,NbPlays,Themes,GameUrl,OpeningTags\n",
        );
        for i in 0..20 {
            let themes = if i % 2 == 0 { "fork" } else { "pin" };
            text += &format!(
                "x{i},{BEFORE},b7b6 a1a8,{},75,90,1,{themes},https://lichess.org/x{i},\n",
                1000 + i * 10
            );
        }
        text += "ruim,fen,b7b6 a1a8,1500,1,1,1,fork,u,\n";
        std::fs::write(&csv, text).unwrap();

        let mut state = TrainingUi::default();
        state.packs.dir = Some(root.join("packs"));
        state.packs.path = csv.display().to_string();
        state.packs.themes = "fork".into();
        state.packs.min_rating = "1050".into();
        state.packs.max_count = "5".into();
        let factory = state.catalog.len();
        state.start_import();
        assert!(state.packs.job.is_some());
        wait(&mut state);

        assert!(matches!(
            state.packs.message,
            Some(Message::Done { ref name, report }) if name.contains("Meus Problemas") && report.imported == 5
        ));
        assert_eq!(state.catalog.len(), factory + 5);
        assert_eq!(state.packs.installed.len(), 1);
        assert_eq!(state.packs.installed[0].id, "lichess-meus-problemas");
        let tactic: Vec<_> = state
            .catalog
            .iter()
            .filter(|e| e.kind == ExerciseKind::Tactic)
            .collect();
        assert_eq!(tactic.len(), 5);
        assert!(
            tactic
                .iter()
                .all(|e| e.themes.contains(&"fork".to_owned()) && e.rating >= Some(1050))
        );

        // Filtro da lista por pacote.
        state.packs.filter_pack = PackChoice::Pack("lichess-meus-problemas".into());
        let f = state.packs.filter();
        assert_eq!(state.catalog.iter().filter(|e| f.matches(e)).count(), 5);
        state.packs.filter_pack = PackChoice::Factory;
        let f = state.packs.filter();
        assert_eq!(
            state.catalog.iter().filter(|e| f.matches(e)).count(),
            factory
        );

        // Um segundo pacote com o mesmo nome de arquivo ganha outro id.
        state.packs.themes.clear();
        state.start_import();
        wait(&mut state);
        assert_eq!(state.packs.installed.len(), 2);

        // Remover um pacote tira só os Exercícios dele.
        state.remove_installed("lichess-meus-problemas", None);
        assert_eq!(state.packs.installed.len(), 1);
        assert!(
            state
                .catalog
                .iter()
                .all(|e| e.pack.as_deref() != Some("lichess-meus-problemas"))
        );
        assert!(state.catalog.len() > factory);
        std::fs::remove_dir_all(root).ok();
    }

    #[test]
    fn a_cancelled_or_empty_selection_installs_nothing() {
        let root = temp("empty");
        let csv = root.join("p.csv");
        std::fs::write(&csv, format!("a1,{BEFORE},b7b6 a1a8,1500,1,1,1,fork,u,\n")).unwrap();
        let mut state = TrainingUi::default();
        state.packs.dir = Some(root.join("packs"));
        state.packs.path = csv.display().to_string();
        state.packs.themes = "pin".into();
        state.start_import();
        wait(&mut state);
        assert!(matches!(state.packs.message, Some(Message::Empty)));
        assert!(state.packs.installed.is_empty());
        std::fs::remove_dir_all(root).ok();
    }

    #[test]
    fn ids_are_slugs_and_stay_unique() {
        assert_eq!(
            slug(Path::new("/x/lichess_db_puzzle.csv.zst")),
            "lichess-lichess-db-puzzle"
        );
        assert_eq!(slug(Path::new("")), "lichess-problemas");
        let taken = vec![PackInfo {
            id: "lichess-a".into(),
            name: String::new(),
            count: 0,
            provider: None,
            license: None,
            downloaded_on: None,
        }];
        assert_eq!(unique_id("lichess-a".into(), &taken), "lichess-a-2");
    }
}
