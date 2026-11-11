//! Painel Biblioteca (C7, #49): pesquisa Partidas jogadas e importadas (texto, jogador,
//! resultado, Política, data e Posição), abre uma Partida (jogada: revisão; importada: árvore
//! de variações navegável), importa PGN do disco, marca Posições favoritas e acha Exercícios
//! por tipo.
//!
//! A Biblioteca é só leitura sobre a Partida (UI-01): mostrar/ocultar o painel nunca emite
//! Comando. Abrir algo **troca** a Partida de forma explícita, pela mesma via de Nova
//! Partida/FEN (época nova): uma Partida jogada volta do banco (pausada, retomável), uma
//! Posição favorita ou de Exercício vira uma Partida Assistida de estudo. O painel fica
//! inacessível numa Partida competitiva (UI-08), como Dicas e Anotações.

use crate::dock::{PanelAccess, PanelDef, ids};
use crate::i18n::I18n;
use crate::persistence::PersistenceHandle;
use crate::session::Session;
use crate::study::StudyNotes;
use crate::training_ui::{TrainingUi, with_store};
use crate::ui_theme::{ButtonVariant, cs_button, cs_dropdown, palette};
use bevy::prelude::*;
use bevy_egui::egui;
use chess_core::{CanonicalPosition, Color as Side};
use chess_game::{GameResult, Outcome, PolicyKind};
use chess_pgn::{MoveNode, PgnGame};
use chess_store::{
    Favorite, FavoriteId, FavoriteOrigin, FavoriteQuery, GameId, ImportMode, ImportOptions,
    ImportProgress, ImportSource, ImportedId, LibraryEntry, LibraryPage, LibraryQuery,
    LibrarySource, NewFavorite, Origin, Pace, ProfileId, Provenance, Store,
};
use chess_training::{Exercise, ExerciseKind};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

/// Partidas por página.
pub const PAGE_SIZE: u32 = 12;
/// A lista se atualiza sozinha (Partida em andamento ganha Lances) neste intervalo.
const AUTO_REFRESH: Duration = Duration::from_secs(2);
/// Relatório de importação: no máximo estes erros são listados.
const MAX_LISTED_ERRORS: usize = 20;
/// Arquivos `.pgn` sugeridos na pasta de documentos.
const MAX_SUGGESTED_FILES: usize = 12;

pub struct LibraryPlugin;

impl Plugin for LibraryPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<LibraryUi>()
            .add_systems(Update, library_demo);
        crate::dock::register_panel(
            app,
            PanelDef {
                id: ids::LIBRARY,
                label_key: "dock-panel-library",
                access: PanelAccess::OutsideCompetitive,
                ui: library_panel,
            },
        );
    }
}

// --- Lógica pura (testável sem Bevy) ----------------------------------------------

/// Símbolo de um NAG de lance (`$1` = `!` ...); os demais glifos não são mostrados.
pub fn nag_symbol(nag: u8) -> Option<&'static str> {
    match nag {
        1 => Some("!"),
        2 => Some("?"),
        3 => Some("!!"),
        4 => Some("??"),
        5 => Some("!?"),
        6 => Some("?!"),
        _ => None,
    }
}

/// Um Lance da árvore já com a Posição resultante.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TreeItem {
    /// Caminho estável na árvore (índices alternando Lance e variação).
    pub id: Vec<u16>,
    /// `1. e4`, `1... c5` (ou só o SAN no meio da linha), com `!`/`?`.
    pub label: String,
    /// FEN depois do Lance.
    pub fen: String,
    /// Meio-lance na linha principal (`None` numa variação).
    pub main_ply: Option<usize>,
    pub comment: String,
}

/// Uma linha da árvore: a principal ou uma variação.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TreeLine {
    /// `None` na linha principal; nas variações, o Lance que elas substituem.
    pub replaces: Option<String>,
    pub depth: usize,
    pub items: Vec<TreeItem>,
}

/// Achata a árvore em linhas navegáveis: a principal primeiro (com a Posição inicial como
/// primeiro item) e depois cada variação, em profundidade. Um Lance ilegal (árvore
/// adulterada) encerra a linha ali.
pub fn flatten_tree(game: &PgnGame) -> Vec<TreeLine> {
    let Ok(start) = CanonicalPosition::from_fen(&game.start_fen) else {
        return Vec::new();
    };
    let mut out = Vec::new();
    walk_line(&mut out, &start, &game.moves, &[], 0, true, 0, None);
    if let Some(first) = out.first_mut() {
        first.items.insert(
            0,
            TreeItem {
                id: Vec::new(),
                label: "Início".into(),
                fen: start.to_fen(),
                main_ply: Some(0),
                comment: game.comments.join(" "),
            },
        );
    }
    out
}

/// Variação pendente: caminho, Posição antes do Lance, linha, meios-lances antes e o Lance trocado.
type Branch<'a> = (
    Vec<u16>,
    CanonicalPosition,
    &'a Vec<MoveNode>,
    usize,
    String,
);

#[allow(clippy::too_many_arguments)]
fn walk_line(
    out: &mut Vec<TreeLine>,
    start: &CanonicalPosition,
    nodes: &[MoveNode],
    prefix: &[u16],
    depth: usize,
    main: bool,
    before_plies: usize,
    replaces: Option<String>,
) {
    let slot = out.len();
    out.push(TreeLine {
        replaces,
        depth,
        items: Vec::new(),
    });
    let mut items = Vec::new();
    let mut branches: Vec<Branch<'_>> = Vec::new();
    let mut position = start.clone();
    for (i, node) in nodes.iter().enumerate() {
        let Ok(mv) = position.parse_san(&node.san) else {
            break;
        };
        let Ok(next) = position.play(mv) else { break };
        let number = position.fullmove_number();
        let white = position.side_to_move() == Side::White;
        let marks: String = node.nags.iter().filter_map(|n| nag_symbol(*n)).collect();
        let text = format!("{}{marks}", node.san);
        let label = if white {
            format!("{number}. {text}")
        } else if i == 0 {
            format!("{number}... {text}")
        } else {
            text
        };
        let plain = if white {
            format!("{number}. {}", node.san)
        } else {
            format!("{number}... {}", node.san)
        };
        let mut id = prefix.to_vec();
        id.push(i as u16);
        let comment = node
            .pre_comments
            .iter()
            .chain(&node.comments)
            .cloned()
            .collect::<Vec<_>>()
            .join(" ");
        items.push(TreeItem {
            id: id.clone(),
            label,
            fen: next.to_fen(),
            main_ply: main.then_some(before_plies + i + 1),
            comment,
        });
        for (v, variation) in node.variations.iter().enumerate() {
            let mut branch_prefix = id.clone();
            branch_prefix.push(v as u16);
            branches.push((
                branch_prefix,
                position.clone(),
                variation,
                before_plies + i,
                plain.clone(),
            ));
        }
        position = next;
    }
    out[slot].items = items;
    for (branch_prefix, before, variation, plies, plain) in branches {
        walk_line(
            out,
            &before,
            variation,
            &branch_prefix,
            depth + 1,
            false,
            plies,
            Some(plain),
        );
    }
}

/// Arquivos `.pgn` de `dir` (ordem alfabética, no máximo [`MAX_SUGGESTED_FILES`]).
pub fn list_pgn_files(dir: &Path) -> Vec<PathBuf> {
    let Ok(read) = std::fs::read_dir(dir) else {
        return Vec::new();
    };
    let mut files: Vec<PathBuf> = read
        .filter_map(|entry| entry.ok().map(|e| e.path()))
        .filter(|p| {
            p.is_file()
                && p.extension()
                    .is_some_and(|ext| ext.eq_ignore_ascii_case("pgn"))
        })
        .collect();
    files.sort();
    files.truncate(MAX_SUGGESTED_FILES);
    files
}

/// Resultado de importar um arquivo PGN (resumo: novas, repetidas e com erro).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ImportOutcome {
    pub file: String,
    /// Partidas novas gravadas.
    pub imported: usize,
    /// Partidas que já estavam na Biblioteca.
    pub duplicates: usize,
    /// Total de Partidas puladas por erro (`errors` lista só as primeiras).
    pub error_count: usize,
    /// Um texto por partida pulada: partida, linha, coluna e o Lance problemático.
    pub errors: Vec<String>,
    /// Partidas convertidas de Windows-1252/Latin-1 (aviso: conferir os acentos).
    pub converted: usize,
    pub cancelled: bool,
}

/// Importa o arquivo (`.pgn` ou `.pgn.gz`) em fluxo, gravando a Origem escolhida
/// (importação manual guiada: sem rede, sem login). `progress` é chamado a cada lote.
pub fn import_file(
    store: &mut Store,
    profile: ProfileId,
    path: &Path,
    source: ImportSource,
    cancel: &AtomicBool,
    progress: &mut dyn FnMut(&ImportProgress),
) -> Result<ImportOutcome, String> {
    let file = path.file_name().map_or_else(
        || path.display().to_string(),
        |n| n.to_string_lossy().into_owned(),
    );
    let mut options = ImportOptions::new(
        Some(&file),
        Provenance::from_source(source, ImportMode::Manual, unix_ms()),
    );
    options.batch_size = IMPORT_BATCH;
    let summary = store
        .import_pgn_file(profile, path, &options, cancel, progress)
        .map_err(|e| format!("{}: {e}", path.display()))?;
    Ok(ImportOutcome {
        file,
        imported: summary.new,
        duplicates: summary.duplicates,
        error_count: summary.errors,
        errors: summary
            .error_samples
            .iter()
            .map(ToString::to_string)
            .collect(),
        converted: summary.converted_games,
        cancelled: summary.cancelled,
    })
}

/// Instante da importação (borda da interface: o domínio recebe o instante pronto).
fn unix_ms() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| i64::try_from(d.as_millis()).unwrap_or(i64::MAX))
}

/// Partidas por transação na importação pela interface.
const IMPORT_BATCH: usize = 500;

/// Importação rodando numa thread própria, com o andamento e o cancelamento
/// compartilhados com a interface.
struct ImportJob {
    file: String,
    progress: Arc<Mutex<ImportProgress>>,
    cancel: Arc<AtomicBool>,
    result: Arc<Mutex<Option<Result<ImportOutcome, String>>>>,
}

impl ImportJob {
    /// Abre uma conexão própria com o banco em `db` e importa fora da thread da interface.
    fn spawn(db: PathBuf, profile: ProfileId, path: PathBuf, source: ImportSource) -> Self {
        let file = path.file_name().map_or_else(
            || path.display().to_string(),
            |n| n.to_string_lossy().into_owned(),
        );
        let job = Self {
            file,
            progress: Arc::new(Mutex::new(ImportProgress::default())),
            cancel: Arc::new(AtomicBool::new(false)),
            result: Arc::new(Mutex::new(None)),
        };
        let (progress, cancel, result) = (
            Arc::clone(&job.progress),
            Arc::clone(&job.cancel),
            Arc::clone(&job.result),
        );
        std::thread::spawn(move || {
            let outcome = Store::open(&db)
                .map_err(|e| e.to_string())
                .and_then(|mut store| {
                    import_file(&mut store, profile, &path, source, &cancel, &mut |p| {
                        if let Ok(mut shared) = progress.lock() {
                            *shared = p.clone();
                        }
                    })
                });
            if let Ok(mut slot) = result.lock() {
                *slot = Some(outcome);
            }
        });
        job
    }

    fn take_result(&self) -> Option<Result<ImportOutcome, String>> {
        self.result.lock().ok()?.take()
    }

    fn snapshot(&self) -> ImportProgress {
        self.progress.lock().map(|p| p.clone()).unwrap_or_default()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
enum Tab {
    #[default]
    Games,
    Favorites,
    Exercises,
    /// Apagar em lote as Partidas importadas por Origem (#242).
    Imports,
}

/// De onde vêm as Partidas que a pessoa vai importar.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
enum ImportMethod {
    /// Um arquivo `.pgn`/`.pgn.gz` no computador (sem rede).
    #[default]
    File,
    /// As Partidas públicas de um jogador, direto do Provedor.
    Online(crate::online_import::Site),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
enum ResultFilter {
    #[default]
    Any,
    WhiteWins,
    BlackWins,
    Draw,
    Unfinished,
}

impl ResultFilter {
    const ALL: [Self; 5] = [
        Self::Any,
        Self::WhiteWins,
        Self::BlackWins,
        Self::Draw,
        Self::Unfinished,
    ];

    fn token(self) -> Option<&'static str> {
        match self {
            Self::Any => None,
            Self::WhiteWins => Some("1-0"),
            Self::BlackWins => Some("0-1"),
            Self::Draw => Some("1/2-1/2"),
            Self::Unfinished => Some("*"),
        }
    }

    fn label_key(self) -> &'static str {
        match self {
            Self::Any => "library-any",
            Self::WhiteWins => "library-result-white",
            Self::BlackWins => "library-result-black",
            Self::Draw => "library-result-draw",
            Self::Unfinished => "library-result-unfinished",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
enum SourceFilter {
    #[default]
    Any,
    Played,
    Imported,
}

fn policy_key(policy: PolicyKind) -> &'static str {
    match policy {
        PolicyKind::FideSimulation => "newgame-policy-fide",
        PolicyKind::SuddenElimination => "newgame-policy-elimination",
        _ => "newgame-policy-assisted",
    }
}

const POLICIES: [PolicyKind; 3] = [
    PolicyKind::Assisted,
    PolicyKind::FideSimulation,
    PolicyKind::SuddenElimination,
];

/// A Partida importada aberta no painel e o Lance selecionado na árvore.
struct Detail {
    id: ImportedId,
    title: String,
    tags: Vec<(String, String)>,
    origins: Vec<Origin>,
    result: String,
    game: PgnGame,
    lines: Vec<TreeLine>,
    selected: Vec<u16>,
}

impl Detail {
    fn selected_item(&self) -> Option<&TreeItem> {
        self.lines
            .iter()
            .flat_map(|l| &l.items)
            .find(|item| item.id == self.selected)
    }
}

/// Estado do painel Biblioteca.
#[derive(Resource)]
pub struct LibraryUi {
    tab: Tab,
    text: String,
    player: String,
    result: ResultFilter,
    policy: Option<PolicyKind>,
    source: SourceFilter,
    provider: Option<ImportSource>,
    eco: String,
    pace: Option<Pace>,
    date_from: String,
    date_to: String,
    position: String,
    page: u32,
    results: Option<LibraryPage>,
    favorites: Vec<Favorite>,
    favorite_text: String,
    exercises: Vec<Exercise>,
    exercise_kind: Option<ExerciseKind>,
    exercise_text: String,
    dirty: bool,
    refreshed: Option<Instant>,
    error: Option<String>,
    status: Option<String>,
    detail: Option<Detail>,
    import_path: String,
    import_source: ImportSource,
    import_job: Option<ImportJob>,
    import_result: Option<Result<ImportOutcome, String>>,
    import_method: ImportMethod,
    /// Abre a seção de importar na próxima vez que desenhar (Ação "Importar online").
    import_open: bool,
    pub(crate) online: crate::online_import_ui::OnlineState,
    pgn_files: Vec<PathBuf>,
    favorite_name: String,
    bulk: crate::library_bulk::BulkUi,
    /// Banco em memória quando o app roda sem persistência (capturas).
    local: Option<Mutex<Store>>,
}

impl Default for LibraryUi {
    fn default() -> Self {
        Self {
            tab: Tab::default(),
            text: String::new(),
            player: String::new(),
            result: ResultFilter::default(),
            policy: None,
            source: SourceFilter::default(),
            provider: None,
            eco: String::new(),
            pace: None,
            date_from: String::new(),
            date_to: String::new(),
            position: String::new(),
            page: 0,
            results: None,
            favorites: Vec::new(),
            favorite_text: String::new(),
            exercises: Vec::new(),
            exercise_kind: None,
            exercise_text: String::new(),
            dirty: true,
            refreshed: None,
            error: None,
            status: None,
            detail: None,
            import_path: String::new(),
            import_source: ImportSource::Other,
            import_job: None,
            import_result: None,
            import_method: ImportMethod::default(),
            import_open: false,
            online: crate::online_import_ui::OnlineState::default(),
            pgn_files: Vec::new(),
            favorite_name: String::new(),
            bulk: crate::library_bulk::BulkUi::default(),
            local: None,
        }
    }
}

impl LibraryUi {
    /// A consulta que os filtros atuais descrevem.
    fn query(&self) -> LibraryQuery {
        let some = |s: &str| (!s.trim().is_empty()).then(|| s.trim().to_owned());
        LibraryQuery {
            text: some(&self.text),
            player: some(&self.player),
            result: self.result.token().map(str::to_owned),
            policy: self.policy,
            date_from: some(&self.date_from),
            date_to: some(&self.date_to),
            position: some(&self.position),
            source: match self.source {
                SourceFilter::Any => None,
                SourceFilter::Played => Some(LibrarySource::Played),
                SourceFilter::Imported => Some(LibrarySource::Imported),
            },
            provider: self.provider.map(|p| p.provider_id().to_owned()),
            eco: some(&self.eco),
            pace: self.pace,
            profile: None, // o chamador fixa o perfil ativo
            limit: Some(PAGE_SIZE),
            offset: self.page * PAGE_SIZE,
        }
    }

    fn touch(&mut self) {
        self.dirty = true;
    }

    /// Procura as Partidas de um jogador (vinda do Explorador: "Partidas de referência").
    pub fn focus_player(&mut self, name: &str) {
        self.tab = Tab::Games;
        self.detail = None;
        self.text.clear();
        self.player = name.trim().to_owned();
        self.page = 0;
        self.touch();
    }

    /// Cena de evidência (`--explorer-demo import`): importação online em andamento ou concluída.
    pub(crate) fn demo_online(&mut self, running: bool) {
        use crate::online_import::{Progress, Site, Summary};
        // Partidas já importadas deste nome: habilita o "Atualizar".
        seed_online_demo(&mut self.local);
        self.show_online_import();
        self.import_method = ImportMethod::Online(Site::Lichess);
        if running {
            self.online.demo_running(
                Site::Lichess,
                "ana_alvares",
                Progress {
                    received: 184,
                    new: 151,
                    duplicates: 33,
                    errors: 0,
                },
            );
        } else {
            self.online.demo_finished(
                Site::Lichess,
                "ana_alvares",
                Summary {
                    received: 212,
                    new: 179,
                    duplicates: 33,
                    errors: 0,
                    cancelled: false,
                },
            );
        }
    }

    /// Mostra a importação online (Ação "Importar do Lichess ou Chess.com").
    pub fn show_online_import(&mut self) {
        self.tab = Tab::Games;
        self.detail = None;
        self.import_open = true;
        if self.import_method == ImportMethod::File {
            self.import_method = ImportMethod::Online(self.online.site);
        }
    }

    /// Atualiza o formulário da importação online com o estado do app e recolhe a thread.
    /// Aba Importações: relê a lista de importações quando ela está à vista.
    fn refresh_bulk(&mut self, handle: Option<&PersistenceHandle>) {
        if self.tab == Tab::Imports {
            self.bulk.refresh(&mut self.local, handle);
        }
    }

    /// Aba Importações: executa o que a tela pediu e manda reler a Biblioteca se apagou.
    fn apply_bulk(&mut self, handle: Option<&PersistenceHandle>) {
        if self.bulk.apply(&mut self.local, handle) {
            self.touch();
        }
    }

    fn sync_online(
        &mut self,
        net: Option<&crate::net_service::NetService>,
        session: &Session,
        handle: Option<&PersistenceHandle>,
    ) {
        self.online.sync(
            net,
            session.competitive_in_progress(),
            handle,
            &mut self.local,
        );
        if self.online.poll() {
            self.touch();
        }
    }

    /// Recolhe o resultado da importação em andamento, se terminou.
    fn poll_import(&mut self) {
        let finished = self.import_job.as_ref().and_then(ImportJob::take_result);
        if let Some(result) = finished {
            self.import_job = None;
            self.import_result = Some(result);
            self.touch();
        }
    }

    /// Releitura do que a aba mostra (a lista também envelhece sozinha).
    fn refresh(&mut self, handle: Option<&PersistenceHandle>) {
        let due = self.refreshed.is_none_or(|at| at.elapsed() >= AUTO_REFRESH);
        if !self.dirty && !due {
            return;
        }
        self.dirty = false;
        self.refreshed = Some(Instant::now());
        let tab = self.tab;
        let favorite_text = self.favorite_text.clone();
        let (kind, exercise_text) = (self.exercise_kind, self.exercise_text.clone());
        let filters = self.query();
        let data = with_store(&mut self.local, handle, |store, profile| match tab {
            Tab::Games => store
                .search_library(&LibraryQuery {
                    profile: Some(profile),
                    ..filters
                })
                .map(|p| (Some(p), None, None)),
            Tab::Favorites => store
                .list_favorites(&FavoriteQuery {
                    text: Some(favorite_text),
                    profile: Some(profile),
                    ..Default::default()
                })
                .map(|f| (None, Some(f), None)),
            Tab::Exercises => {
                exercises_for(store, kind, &exercise_text).map(|e| (None, None, Some(e)))
            }
            Tab::Imports => Ok((None, None, None)),
        });
        match data {
            Some(Ok((page, favorites, exercises))) => {
                self.error = None;
                if let Some(page) = page {
                    // Página além do fim (a lista encolheu): volta para a última.
                    if page.entries.is_empty() && page.total > 0 && self.page > 0 {
                        self.page -= 1;
                        self.dirty = true;
                    }
                    self.results = Some(page);
                }
                if let Some(favorites) = favorites {
                    self.favorites = favorites;
                }
                if let Some(exercises) = exercises {
                    self.exercises = exercises;
                }
            }
            Some(Err(err)) => self.error = Some(err.to_string()),
            None => {}
        }
    }
}

/// Exercícios do filtro; o catálogo embutido entra no banco na primeira vez (idempotente).
fn exercises_for(
    store: &mut Store,
    kind: Option<ExerciseKind>,
    text: &str,
) -> chess_store::Result<Vec<Exercise>> {
    let found = store.search_exercises(kind, Some(text))?;
    if found.is_empty()
        && kind.is_none()
        && text.trim().is_empty()
        && let Ok(catalog) = chess_training::Catalog::bundled()
    {
        let all: Vec<Exercise> = catalog.iter().cloned().collect();
        store.upsert_exercises(&all)?;
        return store.search_exercises(kind, Some(text));
    }
    Ok(found)
}

// --- Ações (aplicadas depois de desenhar) ---------------------------------------------

enum Action {
    OpenPlayed {
        id: GameId,
        ply: Option<u32>,
    },
    OpenImported(ImportedId),
    DeleteImported(ImportedId),
    StudyFen(String),
    StudyMainline {
        ply: usize,
    },
    MarkFavorite {
        name: String,
    },
    MarkTreeNode {
        fen: String,
        name: String,
        origin: Option<FavoriteOrigin>,
    },
    UpdateFavorite {
        id: FavoriteId,
        name: String,
        note: String,
    },
    DeleteFavorite(FavoriteId),
    Import(PathBuf),
    CancelImport,
    OpenFavoriteSource(FavoriteOrigin),
}

/// Posição que o tabuleiro mostra agora (revisão ou ao vivo) e o meio-lance dela.
fn current_position(session: &Session) -> (String, usize) {
    match session.review() {
        Some(view) => (view.position.to_fen(), view.ply),
        None => (
            session.snapshot().position.to_fen(),
            session.snapshot().history.len(),
        ),
    }
}

fn apply(
    action: Action,
    lib: &mut LibraryUi,
    session: &mut Session,
    handle: Option<&PersistenceHandle>,
    i18n: &I18n,
) -> bool {
    let mut opened_played = false;
    match action {
        Action::OpenPlayed { id, ply } => {
            let Some(handle) = handle else {
                lib.status = Some(i18n.t("library-needs-database"));
                return false;
            };
            let Ok(mut persistence) = handle.0.lock() else {
                return false;
            };
            match persistence.switch_to_game(id) {
                Ok(game) => {
                    session.open_game(game);
                    persistence.track(session);
                    session.set_review_ply(Some(ply.unwrap_or(0) as usize));
                    lib.status = Some(i18n.t("library-opened-played"));
                    lib.detail = None;
                    opened_played = true;
                }
                Err(err) => lib.status = Some(err.to_string()),
            }
        }
        Action::OpenImported(id) => {
            let loaded = with_store(&mut lib.local, handle, |store, profile| {
                let origins = store.origins(profile, id).unwrap_or_default();
                store.load_imported(profile, id).map(|game| (game, origins))
            });
            match loaded {
                Some(Ok((game, origins))) => {
                    let title = game_title(&game);
                    lib.detail = Some(Detail {
                        id,
                        title,
                        tags: game.tags.clone(),
                        origins,
                        result: game.result.token().to_owned(),
                        lines: flatten_tree(&game),
                        game,
                        selected: Vec::new(),
                    });
                }
                Some(Err(err)) => lib.status = Some(err.to_string()),
                None => {}
            }
        }
        Action::DeleteImported(id) => {
            let done = with_store(&mut lib.local, handle, |store, profile| {
                store.delete_imported(profile, id)
            });
            if let Some(Err(err)) = done {
                lib.status = Some(err.to_string());
            }
            lib.detail = None;
            lib.touch();
        }
        Action::StudyFen(fen) => match session.study_from_fen(&fen) {
            Ok(()) => lib.status = Some(i18n.t("library-opened-study")),
            Err(err) => lib.status = Some(err.to_string()),
        },
        Action::StudyMainline { ply } => {
            let Some(detail) = &lib.detail else {
                return false;
            };
            match detail.game.to_assisted_game() {
                Ok(game) => {
                    session.open_game(game);
                    session.set_review_ply(Some(ply));
                    lib.status = Some(i18n.t("library-opened-study"));
                }
                Err(err) => lib.status = Some(err.to_string()),
            }
        }
        Action::MarkFavorite { name } => {
            let (fen, ply) = current_position(session);
            let origin = handle
                .and_then(|h| h.0.lock().ok().and_then(|p| p.current_game()))
                .map(|id| FavoriteOrigin {
                    source: LibrarySource::Played,
                    ref_id: id.0,
                    ply: ply as u32,
                });
            let name = favorite_name(i18n, &name, ply);
            add_favorite(lib, handle, i18n, fen, name, origin);
        }
        Action::MarkTreeNode { fen, name, origin } => {
            let ply = origin.map_or(0, |o| o.ply as usize);
            let name = favorite_name(i18n, &name, ply);
            add_favorite(lib, handle, i18n, fen, name, origin);
        }
        Action::UpdateFavorite { id, name, note } => {
            let done = with_store(&mut lib.local, handle, |store, profile| {
                store.update_favorite(profile, id, &name, &note)
            });
            if let Some(Err(err)) = done {
                lib.status = Some(err.to_string());
            }
        }
        Action::DeleteFavorite(id) => {
            let done = with_store(&mut lib.local, handle, |store, profile| {
                store.delete_favorite(profile, id)
            });
            if let Some(Err(err)) = done {
                lib.status = Some(err.to_string());
            }
            lib.touch();
        }
        Action::Import(path) => {
            let source = lib.import_source;
            lib.import_result = None;
            // Banco em arquivo: importa numa thread com a sua conexão. Sem arquivo (capturas),
            // não há como compartilhar o banco em memória: importa aqui mesmo.
            let target = with_store(&mut lib.local, handle, |store, profile| {
                (store.database_path(), profile)
            });
            match target {
                Some((Some(db), profile)) => {
                    lib.import_job = Some(ImportJob::spawn(db, profile, path, source));
                }
                Some((None, _)) => {
                    lib.import_result = with_store(&mut lib.local, handle, |store, profile| {
                        import_file(
                            store,
                            profile,
                            &path,
                            source,
                            &AtomicBool::new(false),
                            &mut |_| {},
                        )
                    });
                    lib.touch();
                }
                None => {}
            }
        }
        Action::CancelImport => {
            if let Some(job) = &lib.import_job {
                job.cancel.store(true, Ordering::Relaxed);
            }
        }
        Action::OpenFavoriteSource(origin) => match (origin.source, handle) {
            (LibrarySource::Played, _) => {
                return apply(
                    Action::OpenPlayed {
                        id: GameId(origin.ref_id),
                        ply: Some(origin.ply),
                    },
                    lib,
                    session,
                    handle,
                    i18n,
                );
            }
            (LibrarySource::Imported, _) => {
                lib.tab = Tab::Games;
                return apply(
                    Action::OpenImported(ImportedId(origin.ref_id)),
                    lib,
                    session,
                    handle,
                    i18n,
                );
            }
        },
    }
    opened_played
}

fn favorite_name(i18n: &I18n, typed: &str, ply: usize) -> String {
    if typed.trim().is_empty() {
        i18n.t_with("library-fav-default-name", &[("ply", &ply.to_string())])
    } else {
        typed.trim().to_owned()
    }
}

fn add_favorite(
    lib: &mut LibraryUi,
    handle: Option<&PersistenceHandle>,
    i18n: &I18n,
    fen: String,
    name: String,
    origin: Option<FavoriteOrigin>,
) {
    let new = NewFavorite {
        fen,
        name,
        note: String::new(),
        origin,
    };
    match with_store(&mut lib.local, handle, |store, profile| {
        store.add_favorite_for(profile, &new)
    }) {
        Some(Ok(_)) => {
            lib.status = Some(i18n.t("library-fav-marked"));
            lib.favorite_name.clear();
            lib.touch();
        }
        Some(Err(err)) => lib.status = Some(err.to_string()),
        None => {}
    }
}

pub(crate) fn source_key(source: ImportSource) -> &'static str {
    match source {
        ImportSource::QueenAlice => "library-import-source-queen-alice",
        ImportSource::ChessBase => "library-import-source-chessbase",
        ImportSource::Twic => "library-import-source-twic",
        ImportSource::PgnMentor => "library-import-source-pgn-mentor",
        ImportSource::Lichess => "library-import-source-lichess",
        ImportSource::ChessCom => "library-import-source-chess-com",
        ImportSource::Other => "library-import-source-other",
    }
}

fn pace_key(pace: Pace) -> &'static str {
    match pace {
        Pace::Bullet => "library-pace-bullet",
        Pace::Blitz => "library-pace-blitz",
        Pace::Rapid => "library-pace-rapid",
        Pace::Classical => "library-pace-classical",
        Pace::Correspondence => "library-pace-correspondence",
    }
}

pub(crate) fn mode_key(mode: ImportMode) -> &'static str {
    match mode {
        ImportMode::File => "library-mode-file",
        ImportMode::Manual => "library-mode-manual",
        ImportMode::Api => "library-mode-api",
    }
}

/// Uma linha da Origem: Provedor, modo, data da importação e arquivo.
fn origin_line(i18n: &I18n, origin: &Origin) -> String {
    let provider = origin
        .provider
        .as_deref()
        .and_then(ImportSource::from_provider_id)
        .map_or_else(
            || i18n.t("library-origin-unknown"),
            |s| i18n.t(source_key(s)),
        );
    let when = origin.retrieved_at_ms / 1000 / 86_400;
    i18n.t_with(
        "library-origin-line",
        &[
            ("provider", &provider),
            ("mode", &i18n.t(mode_key(origin.mode))),
            ("day", &civil_date(when)),
            ("file", origin.source.as_deref().unwrap_or("-")),
        ],
    )
}

/// `AAAA-MM-DD` de um número de dias desde 1970-01-01 (calendário gregoriano proléptico).
fn civil_date(days: i64) -> String {
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1_460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let day = doy - (153 * mp + 2) / 5 + 1;
    let month = if mp < 10 { mp + 3 } else { mp - 9 };
    let year = yoe + era * 400 + i64::from(month <= 2);
    format!("{year:04}-{month:02}-{day:02}")
}

/// Importar PGN: de onde veio o arquivo (grava a Origem e mostra a licença daquela fonte),
/// caminho, andamento com cancelamento e o resumo (novas, repetidas, com erro).
fn draw_import(ui: &mut egui::Ui, i18n: &I18n, lib: &mut LibraryUi, actions: &mut Vec<Action>) {
    ui.label(i18n.t("library-import-method-label"));
    let mut methods = vec![(ImportMethod::File, i18n.t("library-import-method-file"))];
    methods.extend([
        (
            ImportMethod::Online(crate::online_import::Site::Lichess),
            i18n.t("library-import-method-lichess"),
        ),
        (
            ImportMethod::Online(crate::online_import::Site::ChessCom),
            i18n.t("library-import-method-chesscom"),
        ),
    ]);
    cs_dropdown(
        ui,
        "library-import-method",
        &mut lib.import_method,
        &methods,
    );
    ui.add_space(8.0);
    if let ImportMethod::Online(site) = lib.import_method {
        lib.online.site = site;
        crate::online_import_ui::draw(ui, i18n, &mut lib.online);
        return;
    }
    draw_file_import(ui, i18n, lib, actions);
}

/// Importar um arquivo PGN: de onde veio (grava a Origem), caminho, andamento e resumo.
fn draw_file_import(
    ui: &mut egui::Ui,
    i18n: &I18n,
    lib: &mut LibraryUi,
    actions: &mut Vec<Action>,
) {
    let p = palette(ui);
    ui.small(i18n.t("library-import-help"));
    ui.label(i18n.t("library-import-source-label"));
    let sources: Vec<(ImportSource, String)> = ImportSource::ALL
        .into_iter()
        .map(|s| (s, i18n.t(source_key(s))))
        .collect();
    cs_dropdown(
        ui,
        "library-import-source",
        &mut lib.import_source,
        &sources,
    );
    ui.small(i18n.t(&format!(
        "library-license-{}",
        lib.import_source.license_code()
    )));
    ui.add(
        egui::TextEdit::singleline(&mut lib.import_path)
            .hint_text(i18n.t("library-import-hint"))
            .desired_width(f32::INFINITY),
    );
    let busy = lib.import_job.is_some();
    let ready = !lib.import_path.trim().is_empty() && !busy;
    ui.add_enabled_ui(ready, |ui| {
        if cs_button(ui, &i18n.t("library-import-button"), ButtonVariant::Primary).clicked() {
            actions.push(Action::Import(PathBuf::from(lib.import_path.trim())));
        }
    });
    if let Some(job) = &lib.import_job {
        let progress = job.snapshot();
        ui.ctx()
            .request_repaint_after(std::time::Duration::from_millis(100));
        ui.small(i18n.t_with(
            "library-import-progress",
            &[
                ("file", &job.file),
                ("seen", &progress.games_seen.to_string()),
                ("new", &progress.new.to_string()),
                ("dup", &progress.duplicates.to_string()),
                ("errors", &progress.errors.to_string()),
            ],
        ));
        if let Some(total) = progress.bytes_total.filter(|t| *t > 0) {
            ui.add(
                egui::ProgressBar::new((progress.bytes_read as f32 / total as f32).min(1.0))
                    .desired_width(f32::INFINITY),
            );
        }
        if cs_button(
            ui,
            &i18n.t("library-import-cancel"),
            ButtonVariant::Secondary,
        )
        .clicked()
        {
            actions.push(Action::CancelImport);
        }
    }
    if lib.pgn_files.is_empty() {
        let dir = crate::paths::documents_dir(
            crate::paths::Platform::current(),
            &crate::paths::process_env,
        );
        lib.pgn_files = list_pgn_files(&dir);
    }
    if !lib.pgn_files.is_empty() {
        ui.small(i18n.t("library-import-suggestions"));
        for file in lib.pgn_files.clone() {
            let name = file
                .file_name()
                .map_or_else(String::new, |n| n.to_string_lossy().into_owned());
            if ui.small_button(name).clicked() {
                lib.import_path = file.display().to_string();
            }
        }
    }
    match &lib.import_result {
        Some(Ok(outcome)) => {
            let key = if outcome.cancelled {
                "library-import-cancelled"
            } else {
                "library-import-summary"
            };
            ui.colored_label(
                p.success,
                i18n.t_with(
                    key,
                    &[
                        ("file", &outcome.file),
                        ("new", &outcome.imported.to_string()),
                        ("dup", &outcome.duplicates.to_string()),
                        ("errors", &outcome.error_count.to_string()),
                    ],
                ),
            );
            if outcome.converted > 0 {
                ui.colored_label(
                    p.warning,
                    i18n.t_with(
                        "library-import-converted",
                        &[("count", &outcome.converted.to_string())],
                    ),
                );
            }
            if !outcome.errors.is_empty() {
                ui.colored_label(
                    p.danger,
                    i18n.t_with(
                        "library-import-errors",
                        &[("count", &outcome.error_count.to_string())],
                    ),
                );
                for error in outcome.errors.iter().take(MAX_LISTED_ERRORS) {
                    ui.small(error.clone());
                }
            }
        }
        Some(Err(message)) => {
            ui.colored_label(
                p.danger,
                i18n.t_with("library-import-failed", &[("detail", message)]),
            );
        }
        None => {}
    }
}

fn game_title(game: &PgnGame) -> String {
    let name = |tag: &str| game.tag(tag).filter(|v| *v != "?").map(str::to_owned);
    match (name("White"), name("Black")) {
        (Some(w), Some(b)) => format!("{w} × {b}"),
        _ => name("Event").unwrap_or_else(|| "PGN".to_owned()),
    }
}

// --- Interface ------------------------------------------------------------------------------

fn side_name(entry: &LibraryEntry, white: bool) -> String {
    let name = if white { &entry.white } else { &entry.black };
    name.clone().unwrap_or_else(|| "?".to_owned())
}

fn result_text(i18n: &I18n, entry: &LibraryEntry) -> String {
    let result = match entry.result.as_str() {
        "1-0" => Some(GameResult::WhiteWins),
        "0-1" => Some(GameResult::BlackWins),
        "1/2-1/2" => Some(GameResult::Draw),
        _ => None,
    };
    match (result, entry.reason) {
        (Some(result), Some(reason)) => crate::ui::outcome_text(i18n, Outcome { result, reason }),
        (Some(_), None) => entry.result.clone(),
        (None, _) => i18n.t("library-result-unfinished"),
    }
}

/// Conteúdo do painel (registrado no dock).
fn library_panel(ui: &mut egui::Ui, i18n: &I18n, world: &mut World) {
    // Abrir a própria Partida na Biblioteca mostraria a notação: oculta com a Súmula aberta.
    if world
        .get_resource::<crate::scoresheet::ScoresheetUi>()
        .is_some_and(crate::scoresheet::ScoresheetUi::hides_notation)
    {
        crate::scoresheet::hidden_notice(ui, i18n);
        return;
    }
    world.resource_scope(|world, mut lib: Mut<LibraryUi>| {
        world.resource_scope(|world, mut session: Mut<Session>| {
            let swapped = world
                .get_resource::<TrainingUi>()
                .is_some_and(|t| t.session_swapped());
            let mut actions = Vec::new();
            {
                let handle = world.get_resource::<PersistenceHandle>();
                lib.poll_import();
                lib.sync_online(
                    world.get_resource::<crate::net_service::NetService>(),
                    &session,
                    handle,
                );
                lib.refresh(handle);
                lib.refresh_bulk(handle);
                draw(ui, i18n, &mut lib, &session, swapped, &mut actions);
                lib.apply_bulk(handle);
            }
            let mut opened_played = false;
            for action in actions {
                let handle = world.get_resource::<PersistenceHandle>();
                opened_played |= apply(action, &mut lib, &mut session, handle, i18n);
            }
            if let Some(site) = lib.online.ask_to_turn_on.take() {
                crate::explorer::ask_to_turn_on(
                    world,
                    i18n,
                    site.provider(),
                    "library-online-feature",
                );
            }
            // Uma Partida reaberta do banco traz as Anotações de estudo dela.
            if opened_played {
                let loaded = world
                    .get_resource::<PersistenceHandle>()
                    .and_then(|h| h.0.lock().ok().map(|p| p.load_notes()));
                if let Some(loaded) = loaded
                    && let Some(mut notes) = world.get_resource_mut::<StudyNotes>()
                {
                    *notes = StudyNotes::loaded(session.epoch(), loaded);
                }
            }
        });
    });
}

fn draw(
    ui: &mut egui::Ui,
    i18n: &I18n,
    lib: &mut LibraryUi,
    session: &Session,
    swapped: bool,
    actions: &mut Vec<Action>,
) {
    if swapped {
        ui.weak(i18n.t("library-unavailable-training"));
    }
    let previous = lib.tab;
    ui.horizontal(|ui| {
        ui.selectable_value(&mut lib.tab, Tab::Games, i18n.t("library-tab-games"));
        ui.selectable_value(
            &mut lib.tab,
            Tab::Favorites,
            i18n.t_with(
                "library-tab-favorites",
                &[("count", &lib.favorites.len().to_string())],
            ),
        );
        ui.selectable_value(
            &mut lib.tab,
            Tab::Exercises,
            i18n.t("library-tab-exercises"),
        );
        ui.selectable_value(&mut lib.tab, Tab::Imports, i18n.t("library-tab-imports"));
    });
    if lib.tab != previous {
        lib.touch();
        lib.detail = None;
    }
    if let Some(error) = &lib.error {
        ui.colored_label(egui::Color32::LIGHT_RED, error.clone());
    }
    if let Some(status) = &lib.status {
        ui.small(status.clone());
    }
    ui.separator();
    match lib.tab {
        Tab::Games if lib.detail.is_some() => draw_detail(ui, i18n, lib, swapped, actions),
        Tab::Games => draw_games(ui, i18n, lib, session, swapped, actions),
        Tab::Favorites => draw_favorites(ui, i18n, lib, session, swapped, actions),
        Tab::Exercises => draw_exercises(ui, i18n, lib, swapped, actions),
        Tab::Imports => {
            let ctx = ui.ctx().clone();
            lib.bulk.draw(ui, &ctx, i18n);
        }
    }
}

fn mark_current_row(
    ui: &mut egui::Ui,
    i18n: &I18n,
    lib: &mut LibraryUi,
    session: &Session,
    actions: &mut Vec<Action>,
) {
    let (_, ply) = current_position(session);
    ui.horizontal(|ui| {
        ui.add(
            egui::TextEdit::singleline(&mut lib.favorite_name)
                .hint_text(i18n.t_with("library-fav-hint", &[("ply", &ply.to_string())]))
                .desired_width(150.0),
        );
        if ui
            .button(i18n.t("library-fav-mark"))
            .on_hover_text(i18n.t("library-fav-mark-hover"))
            .clicked()
        {
            actions.push(Action::MarkFavorite {
                name: lib.favorite_name.clone(),
            });
        }
    });
}

fn draw_games(
    ui: &mut egui::Ui,
    i18n: &I18n,
    lib: &mut LibraryUi,
    session: &Session,
    swapped: bool,
    actions: &mut Vec<Action>,
) {
    let force_open = std::mem::take(&mut lib.import_open).then_some(true);
    egui::CollapsingHeader::new(i18n.t("library-import-heading"))
        .open(force_open)
        .show(ui, |ui| {
            draw_import(ui, i18n, lib, actions);
        });

    let before = (
        lib.text.clone(),
        lib.player.clone(),
        lib.result,
        lib.policy,
        lib.source,
        (lib.provider, lib.eco.clone(), lib.pace),
        lib.date_from.clone(),
        lib.date_to.clone(),
        lib.position.clone(),
    );
    ui.add(
        egui::TextEdit::singleline(&mut lib.text)
            .hint_text(i18n.t("library-search-hint"))
            .desired_width(f32::INFINITY),
    );
    egui::CollapsingHeader::new(i18n.t("library-filters")).show(ui, |ui| {
        ui.add(
            egui::TextEdit::singleline(&mut lib.player)
                .hint_text(i18n.t("library-filter-player"))
                .desired_width(f32::INFINITY),
        );
        ui.horizontal_wrapped(|ui| {
            egui::ComboBox::from_id_salt("library-result")
                .selected_text(i18n.t(lib.result.label_key()))
                .show_ui(ui, |ui| {
                    for option in ResultFilter::ALL {
                        ui.selectable_value(&mut lib.result, option, i18n.t(option.label_key()));
                    }
                });
            egui::ComboBox::from_id_salt("library-policy")
                .selected_text(
                    lib.policy
                        .map_or_else(|| i18n.t("library-any-policy"), |p| i18n.t(policy_key(p))),
                )
                .show_ui(ui, |ui| {
                    ui.selectable_value(&mut lib.policy, None, i18n.t("library-any-policy"));
                    for policy in POLICIES {
                        ui.selectable_value(
                            &mut lib.policy,
                            Some(policy),
                            i18n.t(policy_key(policy)),
                        );
                    }
                });
            egui::ComboBox::from_id_salt("library-source")
                .selected_text(i18n.t(match lib.source {
                    SourceFilter::Any => "library-source-any",
                    SourceFilter::Played => "library-source-played",
                    SourceFilter::Imported => "library-source-imported",
                }))
                .show_ui(ui, |ui| {
                    ui.selectable_value(
                        &mut lib.source,
                        SourceFilter::Any,
                        i18n.t("library-source-any"),
                    );
                    ui.selectable_value(
                        &mut lib.source,
                        SourceFilter::Played,
                        i18n.t("library-source-played"),
                    );
                    ui.selectable_value(
                        &mut lib.source,
                        SourceFilter::Imported,
                        i18n.t("library-source-imported"),
                    );
                });
        });
        ui.horizontal(|ui| {
            ui.label(i18n.t("library-filter-date"));
            ui.add(
                egui::TextEdit::singleline(&mut lib.date_from)
                    .hint_text(i18n.t("library-filter-date-hint"))
                    .desired_width(86.0),
            );
            ui.label("–");
            ui.add(
                egui::TextEdit::singleline(&mut lib.date_to)
                    .hint_text(i18n.t("library-filter-date-hint"))
                    .desired_width(86.0),
            );
        });
        ui.horizontal_wrapped(|ui| {
            let providers: Vec<(Option<ImportSource>, String)> =
                std::iter::once((None, i18n.t("library-any-provider")))
                    .chain(
                        ImportSource::ALL
                            .into_iter()
                            .map(|s| (Some(s), i18n.t(source_key(s)))),
                    )
                    .collect();
            cs_dropdown(ui, "library-provider", &mut lib.provider, &providers);
            let paces: Vec<(Option<Pace>, String)> =
                std::iter::once((None, i18n.t("library-any-pace")))
                    .chain(
                        Pace::ALL
                            .into_iter()
                            .map(|p| (Some(p), i18n.t(pace_key(p)))),
                    )
                    .collect();
            cs_dropdown(ui, "library-pace", &mut lib.pace, &paces);
            ui.add(
                egui::TextEdit::singleline(&mut lib.eco)
                    .hint_text(i18n.t("library-filter-eco"))
                    .desired_width(64.0),
            );
        });
        ui.add(
            egui::TextEdit::singleline(&mut lib.position)
                .hint_text(i18n.t("library-filter-position"))
                .desired_width(f32::INFINITY),
        );
        ui.horizontal(|ui| {
            if ui.button(i18n.t("library-position-current")).clicked() {
                lib.position = current_position(session).0;
            }
            if ui.button(i18n.t("library-filters-clear")).clicked() {
                lib.text.clear();
                lib.player.clear();
                lib.result = ResultFilter::Any;
                lib.policy = None;
                lib.source = SourceFilter::Any;
                lib.provider = None;
                lib.eco.clear();
                lib.pace = None;
                lib.date_from.clear();
                lib.date_to.clear();
                lib.position.clear();
            }
        });
    });
    let after = (
        lib.text.clone(),
        lib.player.clone(),
        lib.result,
        lib.policy,
        lib.source,
        (lib.provider, lib.eco.clone(), lib.pace),
        lib.date_from.clone(),
        lib.date_to.clone(),
        lib.position.clone(),
    );
    if before != after {
        lib.page = 0;
        lib.touch();
    }
    mark_current_row(ui, i18n, lib, session, actions);
    ui.separator();

    let Some(page) = lib.results.clone() else {
        return;
    };
    let pages = page.total.div_ceil(u64::from(PAGE_SIZE)).max(1);
    ui.horizontal(|ui| {
        ui.strong(i18n.t_with("library-count", &[("count", &page.total.to_string())]));
        if ui
            .add_enabled(lib.page > 0, egui::Button::new("<"))
            .clicked()
        {
            lib.page -= 1;
            lib.touch();
        }
        ui.label(format!("{}/{pages}", lib.page + 1));
        if ui
            .add_enabled(u64::from(lib.page) + 1 < pages, egui::Button::new(">"))
            .clicked()
        {
            lib.page += 1;
            lib.touch();
        }
    });
    if page.entries.is_empty() {
        ui.weak(i18n.t("library-empty"));
        return;
    }
    egui::ScrollArea::vertical()
        .auto_shrink([false, false])
        .show(ui, |ui| {
            for entry in &page.entries {
                draw_entry(ui, i18n, entry, swapped, actions);
            }
        });
}

fn draw_entry(
    ui: &mut egui::Ui,
    i18n: &I18n,
    entry: &LibraryEntry,
    swapped: bool,
    actions: &mut Vec<Action>,
) {
    ui.group(|ui| {
        ui.set_width(ui.available_width());
        ui.horizontal_wrapped(|ui| {
            ui.strong(format!(
                "{} × {}",
                side_name(entry, true),
                side_name(entry, false)
            ));
            ui.weak(if entry.source == LibrarySource::Played {
                i18n.t("library-source-played")
            } else {
                i18n.t("library-source-imported")
            });
        });
        ui.small(result_text(i18n, entry));
        let mut meta =
            vec![i18n.t_with("library-plies", &[("count", &entry.ply_count.to_string())])];
        if let Some(date) = &entry.date {
            meta.push(date.clone());
        }
        if let Some(policy) = entry.policy {
            meta.push(i18n.t(policy_key(policy)));
        }
        if let Some(event) = &entry.event {
            meta.push(event.clone());
        }
        ui.small(meta.join(" · "));
        if let Some(opening) = &entry.opening {
            ui.small(opening.clone());
        }
        ui.horizontal(|ui| {
            let can_open = !swapped;
            if ui
                .add_enabled(can_open, egui::Button::new(i18n.t("library-open")))
                .clicked()
            {
                actions.push(match entry.source {
                    LibrarySource::Played => Action::OpenPlayed {
                        id: GameId(entry.ref_id),
                        ply: entry.matched_ply,
                    },
                    LibrarySource::Imported => Action::OpenImported(ImportedId(entry.ref_id)),
                });
            }
            if let Some(ply) = entry.matched_ply {
                ui.small(i18n.t_with("library-matched-ply", &[("ply", &ply.to_string())]));
            }
        });
    });
}

fn draw_detail(
    ui: &mut egui::Ui,
    i18n: &I18n,
    lib: &mut LibraryUi,
    swapped: bool,
    actions: &mut Vec<Action>,
) {
    let Some(detail) = lib.detail.as_mut() else {
        return;
    };
    let mut close = false;
    let mut delete = None;
    ui.horizontal(|ui| {
        if ui.button(i18n.t("library-back")).clicked() {
            close = true;
        }
        if ui.small_button(i18n.t("library-delete-imported")).clicked() {
            delete = Some(detail.id);
        }
    });
    ui.strong(detail.title.clone());
    ui.small(format!(
        "{} · {}",
        detail.result,
        i18n.t_with(
            "library-tree-summary",
            &[
                ("plies", &detail.game.total_plies().to_string()),
                ("variations", &detail.game.variation_count().to_string()),
            ]
        )
    ));
    egui::CollapsingHeader::new(i18n.t("library-origin-heading"))
        .default_open(true)
        .show(ui, |ui| {
            for origin in &detail.origins {
                ui.small(origin_line(i18n, origin));
                if let Some(license) = &origin.license {
                    ui.weak(i18n.t(&format!("library-license-{license}")));
                }
            }
        });
    egui::CollapsingHeader::new(i18n.t("library-tags")).show(ui, |ui| {
        for (name, value) in &detail.tags {
            ui.small(format!("{name}: {value}"));
        }
    });
    ui.separator();
    let mut selected = detail.selected.clone();
    egui::ScrollArea::vertical()
        .id_salt("library-tree")
        .max_height(ui.available_height() - 110.0)
        .auto_shrink([false, false])
        .show(ui, |ui| {
            for line in &detail.lines {
                ui.horizontal_wrapped(|ui| {
                    ui.add_space(line.depth as f32 * 10.0);
                    match &line.replaces {
                        Some(replaced) => {
                            ui.weak(i18n.t_with("library-variation", &[("move", replaced)]));
                        }
                        None => {
                            ui.weak(i18n.t("library-mainline"));
                        }
                    }
                });
                ui.horizontal_wrapped(|ui| {
                    ui.add_space(line.depth as f32 * 10.0);
                    for item in &line.items {
                        if ui
                            .selectable_label(item.id == selected, &item.label)
                            .clicked()
                        {
                            selected = item.id.clone();
                        }
                    }
                });
            }
        });
    detail.selected = selected;
    ui.separator();
    if let Some(item) = detail.selected_item() {
        if !item.comment.is_empty() {
            ui.small(item.comment.clone());
        }
        ui.small(item.fen.clone());
        let fen = item.fen.clone();
        let main_ply = item.main_ply;
        let imported_id = detail.id;
        ui.horizontal_wrapped(|ui| {
            if ui
                .add_enabled(
                    !swapped,
                    egui::Button::new(i18n.t("library-study-position")),
                )
                .on_hover_text(i18n.t("library-study-position-hover"))
                .clicked()
            {
                actions.push(Action::StudyFen(fen.clone()));
            }
            if let Some(ply) = main_ply
                && ui
                    .add_enabled(
                        !swapped,
                        egui::Button::new(i18n.t("library-study-mainline")),
                    )
                    .on_hover_text(i18n.t("library-study-mainline-hover"))
                    .clicked()
            {
                actions.push(Action::StudyMainline { ply });
            }
            if ui.button(i18n.t("library-fav-mark-this")).clicked() {
                actions.push(Action::MarkTreeNode {
                    fen: fen.clone(),
                    name: String::new(),
                    origin: main_ply.map(|ply| FavoriteOrigin {
                        source: LibrarySource::Imported,
                        ref_id: imported_id.0,
                        ply: ply as u32,
                    }),
                });
            }
        });
    }
    if close {
        lib.detail = None;
    }
    if let Some(id) = delete {
        actions.push(Action::DeleteImported(id));
    }
}

fn draw_favorites(
    ui: &mut egui::Ui,
    i18n: &I18n,
    lib: &mut LibraryUi,
    session: &Session,
    swapped: bool,
    actions: &mut Vec<Action>,
) {
    mark_current_row(ui, i18n, lib, session, actions);
    if ui
        .add(
            egui::TextEdit::singleline(&mut lib.favorite_text)
                .hint_text(i18n.t("library-fav-search"))
                .desired_width(f32::INFINITY),
        )
        .changed()
    {
        lib.touch();
    }
    ui.separator();
    if lib.favorites.is_empty() {
        ui.weak(i18n.t("library-fav-empty"));
        return;
    }
    egui::ScrollArea::vertical()
        .auto_shrink([false, false])
        .show(ui, |ui| {
            for favorite in &mut lib.favorites {
                ui.group(|ui| {
                    ui.set_width(ui.available_width());
                    let mut changed = false;
                    changed |= ui
                        .add(
                            egui::TextEdit::singleline(&mut favorite.name)
                                .desired_width(f32::INFINITY),
                        )
                        .changed();
                    changed |= ui
                        .add(
                            egui::TextEdit::multiline(&mut favorite.note)
                                .hint_text(i18n.t("library-fav-note-hint"))
                                .desired_rows(1)
                                .desired_width(f32::INFINITY),
                        )
                        .changed();
                    if changed {
                        actions.push(Action::UpdateFavorite {
                            id: favorite.id,
                            name: favorite.name.clone(),
                            note: favorite.note.clone(),
                        });
                    }
                    ui.small(favorite.fen.clone());
                    ui.horizontal_wrapped(|ui| {
                        if ui
                            .add_enabled(!swapped, egui::Button::new(i18n.t("library-fav-open")))
                            .on_hover_text(i18n.t("library-study-position-hover"))
                            .clicked()
                        {
                            actions.push(Action::StudyFen(favorite.fen.clone()));
                        }
                        if let Some(origin) = favorite.origin
                            && ui
                                .add_enabled(
                                    !swapped,
                                    egui::Button::new(i18n.t("library-fav-source")),
                                )
                                .on_hover_text(i18n.t_with(
                                    "library-fav-source-hover",
                                    &[("ply", &origin.ply.to_string())],
                                ))
                                .clicked()
                        {
                            actions.push(Action::OpenFavoriteSource(origin));
                        }
                        if ui.button(i18n.t("library-fav-delete")).clicked() {
                            actions.push(Action::DeleteFavorite(favorite.id));
                        }
                    });
                });
            }
        });
}

fn draw_exercises(
    ui: &mut egui::Ui,
    i18n: &I18n,
    lib: &mut LibraryUi,
    swapped: bool,
    actions: &mut Vec<Action>,
) {
    let (kind_before, text_before) = (lib.exercise_kind, lib.exercise_text.clone());
    ui.horizontal(|ui| {
        egui::ComboBox::from_id_salt("library-exercise-kind")
            .selected_text(
                lib.exercise_kind
                    .map_or_else(|| i18n.t("library-any"), |k| exercise_kind_label(i18n, k)),
            )
            .show_ui(ui, |ui| {
                ui.selectable_value(&mut lib.exercise_kind, None, i18n.t("library-any"));
                for kind in ExerciseKind::ALL {
                    ui.selectable_value(
                        &mut lib.exercise_kind,
                        Some(kind),
                        exercise_kind_label(i18n, kind),
                    );
                }
            });
        ui.add(
            egui::TextEdit::singleline(&mut lib.exercise_text)
                .hint_text(i18n.t("library-exercise-search"))
                .desired_width(f32::INFINITY),
        );
    });
    if (kind_before, &text_before) != (lib.exercise_kind, &lib.exercise_text) {
        lib.touch();
    }
    ui.strong(i18n.t_with(
        "library-exercise-count",
        &[("count", &lib.exercises.len().to_string())],
    ));
    ui.separator();
    egui::ScrollArea::vertical()
        .auto_shrink([false, false])
        .show(ui, |ui| {
            for exercise in &lib.exercises {
                ui.group(|ui| {
                    ui.set_width(ui.available_width());
                    ui.strong(exercise.title.clone());
                    ui.small(exercise_kind_label(i18n, exercise.kind));
                    ui.small(exercise.objective.clone());
                    if ui
                        .add_enabled(
                            !swapped,
                            egui::Button::new(i18n.t("library-study-position")),
                        )
                        .on_hover_text(i18n.t("library-study-position-hover"))
                        .clicked()
                    {
                        actions.push(Action::StudyFen(exercise.fen.clone()));
                    }
                });
            }
        });
}

fn exercise_kind_label(i18n: &I18n, kind: ExerciseKind) -> String {
    i18n.t(&format!("training-kind-{}", kind.as_str()))
}

// --- Evidências (`--screenshot <dir> --panel --tabs library --library <roteiro>`) ----------

fn demo_script() -> Option<String> {
    crate::capture::option_value("--library")
}

const DEMO_PGN: &str = "\
[Event \"Estudo da Siciliana\"]
[Site \"Clube\"]
[White \"Ana Álvares\"]
[Black \"Beto Lima\"]
[Date \"2024.03.15\"]
[Result \"0-1\"]
[Opening \"Siciliana\"]

1. e4 c5 {A Siciliana} 2. Nf3 d6 (2... Nc6 3. d4 cxd4 4. Nxd4) 3. d4 cxd4 4. Nxd4 Nf6 5. Nc3 a6 $5 0-1

[Event \"Italiana\"]
[White \"Carla Dias\"]
[Black \"Davi Rocha\"]
[Date \"2023.??.??\"]
[Result \"1-0\"]

1. e4 e5 2. Nf3 Nc6 3. Bc4 Bc5 (3... Nf6 4. d3) 4. c3 Nf6 5. d4 exd4 1-0

[Event \"Gambito da Dama\"]
[White \"Eva Prado\"]
[Black \"Caio Neves\"]
[Date \"2024.07.02\"]
[Result \"1/2-1/2\"]

1. d4 d5 2. c4 e6 3. Nc3 Nf6 4. Bg5 Be7 1/2-1/2
";

/// Três Partidas fictícias "do Lichess" de `ana_alvares` (só nas capturas).
fn seed_online_demo(local: &mut Option<Mutex<Store>>) {
    let pgn = (1..=3).fold(String::new(), |mut out, n| {
        out.push_str(&format!(
            "[Event \"Partida rápida {n}\"]\n[Site \"https://lichess.org/Demo000{n}\"]\n\
             [White \"ana_alvares\"]\n[Black \"jogador{n}\"]\n[Date \"2026.11.0{}\"]\n\
             [Result \"1-0\"]\n[TimeControl \"600+0\"]\n\n1. e4 e5 2. Nf3 Nc6 1-0\n\n",
            n + 4
        ));
        out
    });
    let options = ImportOptions::new(
        None,
        Provenance::from_source(ImportSource::Lichess, ImportMode::Api, unix_ms()),
    );
    with_store(local, None, |store, profile| {
        let _ = store.import_pgn_with(profile, &pgn, &options);
    });
}

fn seed_demo(store: &mut Store) {
    use chess_game::{Command, Game, GameSetup, Timestamp};
    let _ = store.import_pgn(DEMO_PGN, Some("estudos.pgn"));
    // Duas Partidas jogadas no app (uma contra a Engine, que terminou).
    let play = |setup: GameSetup, moves: &[(&str, &str)], resign: bool| {
        let mut game = Game::new(setup);
        let mut events = Vec::new();
        let mut t = 0;
        for (from, to) in moves {
            t += 1000;
            let (Ok(from), Ok(to)) = (from.parse(), to.parse()) else {
                continue;
            };
            if let Ok(out) = game.handle(
                Command::Release { from, to },
                Timestamp::with_wall(t, 1_731_000_000_000 + t as i64),
            ) {
                events.extend(out);
            }
        }
        if resign
            && let Ok(out) = game.handle(
                Command::Resign { side: Side::White },
                Timestamp::mono(t + 1000),
            )
        {
            events.extend(out);
        }
        (game, events)
    };
    let moves = [("e2", "e4"), ("e7", "e5"), ("g1", "f3"), ("b8", "c6")];
    let engine = GameSetup::standard(PolicyKind::Assisted).with_engine(chess_game::EngineSeat {
        color: Side::Black,
        elo: 1500,
    });
    for (setup, resign) in [
        (engine, true),
        (GameSetup::standard(PolicyKind::Assisted), false),
    ] {
        let (game, events) = play(setup, &moves, resign);
        if let Ok(id) = store.create_game(game.setup(), &chess_store::GameMetadata::default()) {
            let _ = store.append_events(id, &events);
        }
    }
    if let Ok(catalog) = chess_training::Catalog::bundled() {
        let all: Vec<Exercise> = catalog.iter().cloned().collect();
        let _ = store.upsert_exercises(&all);
    }
    for (fen, name, note) in [
        (
            "rnbqkbnr/pppp1ppp/8/4p3/4P3/5N2/PPPP1PPP/RNBQKB1R b KQkq - 1 2",
            "Abertura aberta",
            "Revisar o plano das pretas.",
        ),
        (
            "4k3/8/8/8/8/8/4P3/4K3 w - - 0 1",
            "Final de peão",
            "Oposição e casas-chave.",
        ),
    ] {
        let _ = store.add_favorite(&NewFavorite {
            fen: fen.into(),
            name: name.into(),
            note: note.into(),
            origin: None,
        });
    }
}

/// Roteiro de captura: só existe em `--screenshot`, com `--library` (nunca em uso normal).
fn library_demo(
    mut lib: ResMut<LibraryUi>,
    mut dock: ResMut<crate::dock::Dock>,
    registry: Res<crate::dock::PanelRegistry>,
    capture: Option<Res<crate::capture::CaptureMode>>,
    mut frame: Local<u32>,
) {
    if capture.is_none() {
        return;
    }
    let Some(script) = demo_script() else { return };
    *frame += 1;
    if *frame == 1 {
        // Evidência: só a Biblioteca no dock, para ela ocupar a altura toda.
        for def in registry.iter() {
            dock.layout.set_visible(def.id, def.id == ids::LIBRARY);
        }
    }
    if *frame != 3 {
        return;
    }
    if let Ok(store) = Store::open_in_memory() {
        let mut store = store;
        seed_demo(&mut store);
        lib.local = Some(Mutex::new(store));
    }
    lib.touch();
    match script.as_str() {
        "search" => lib.text = "siciliana".into(),
        "position" => {
            lib.position = "rnbqkbnr/pppp1ppp/8/4p3/4P3/5N2/PPPP1PPP/RNBQKB1R b KQkq - 1 2".into();
        }
        "favorites" => lib.tab = Tab::Favorites,
        "imports" | "imports-confirm" => {
            with_store(&mut lib.local, None, |store, _| {
                crate::library_bulk::seed_demo(store);
            });
            lib.tab = Tab::Imports;
            if script == "imports-confirm" {
                let LibraryUi { bulk, local, .. } = &mut *lib;
                bulk.demo_confirm(local, |groups, selection| {
                    let lichess = Some("lichess".to_owned());
                    selection.set_provider(&lichess, true);
                    if let Some(g) = groups.iter().find(|g| g.provider.is_none()) {
                        selection.set_batch(&crate::library_bulk::BatchKey::of(g), true);
                    }
                });
            }
        }
        "exercises" => {
            lib.tab = Tab::Exercises;
            lib.exercise_kind = Some(ExerciseKind::Mate);
        }
        "detail" => {
            let id = with_store(&mut lib.local, None, |store, _| {
                store
                    .search_library(&LibraryQuery {
                        source: Some(LibrarySource::Imported),
                        text: Some("siciliana".into()),
                        ..Default::default()
                    })
                    .ok()
                    .and_then(|p| p.entries.first().map(|e| e.ref_id))
            })
            .flatten();
            if let Some(id) = id
                && let Some(Ok(game)) = with_store(&mut lib.local, None, |store, profile| {
                    store.load_imported(profile, ImportedId(id))
                })
            {
                let lines = flatten_tree(&game);
                let selected = lines
                    .first()
                    .and_then(|l| l.items.get(5))
                    .map(|i| i.id.clone())
                    .unwrap_or_default();
                lib.detail = Some(Detail {
                    id: ImportedId(id),
                    title: game_title(&game),
                    tags: game.tags.clone(),
                    origins: Vec::new(),
                    result: game.result.token().to_owned(),
                    lines,
                    game,
                    selected,
                });
            }
        }
        _ => {}
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;

    const TREE_PGN: &str = "[White \"Ana\"]\n[Black \"Beto\"]\n\n1. e4 c5 (1... e5 2. Nf3 (2. f4 $6)) 2. Nf3 {desenvolve} d6 $1 *";

    fn tree() -> Vec<TreeLine> {
        let game = chess_pgn::parse_pgn(TREE_PGN).unwrap().remove(0);
        flatten_tree(&game)
    }

    #[test]
    fn the_tree_lists_the_mainline_first_and_every_variation() {
        let lines = tree();
        assert_eq!(lines.len(), 3, "principal + 2 variações");
        let main: Vec<&str> = lines[0].items.iter().map(|i| i.label.as_str()).collect();
        assert_eq!(main, ["Início", "1. e4", "c5", "2. Nf3", "d6!"]);
        assert!(lines[0].replaces.is_none());
        // Variação de 1... c5, em profundidade 1, começando pelo lance das pretas.
        assert_eq!(lines[1].replaces.as_deref(), Some("1... c5"));
        assert_eq!(lines[1].depth, 1);
        let first: Vec<&str> = lines[1].items.iter().map(|i| i.label.as_str()).collect();
        assert_eq!(first, ["1... e5", "2. Nf3"]);
        // Variação aninhada, em profundidade 2, com NAG.
        assert_eq!(lines[2].replaces.as_deref(), Some("2. Nf3"));
        assert_eq!(lines[2].depth, 2);
        assert_eq!(lines[2].items[0].label, "2. f4?!");
    }

    #[test]
    fn mainline_items_know_their_ply_and_variations_do_not() {
        let lines = tree();
        let plies: Vec<Option<usize>> = lines[0].items.iter().map(|i| i.main_ply).collect();
        assert_eq!(plies, [Some(0), Some(1), Some(2), Some(3), Some(4)]);
        assert!(lines[1].items.iter().all(|i| i.main_ply.is_none()));
        assert_eq!(lines[0].items[3].comment, "desenvolve");
    }

    #[test]
    fn item_ids_are_unique_and_positions_follow_the_line() {
        let lines = tree();
        let mut ids: Vec<&Vec<u16>> = lines
            .iter()
            .flat_map(|l| l.items.iter().map(|i| &i.id))
            .collect();
        let total = ids.len();
        ids.sort();
        ids.dedup();
        assert_eq!(ids.len(), total);
        // Depois de 1.e4 e5 2.Nf3 (variação), o cavalo está em f3.
        assert!(
            lines[1].items[1]
                .fen
                .starts_with("rnbqkbnr/pppp1ppp/8/4p3/4P3/5N2/")
        );
        // A Posição inicial é o primeiro item da principal.
        assert_eq!(
            lines[0].items[0].fen,
            "rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 0 1"
        );
    }

    #[test]
    fn nags_have_symbols() {
        assert_eq!(nag_symbol(1), Some("!"));
        assert_eq!(nag_symbol(4), Some("??"));
        assert_eq!(nag_symbol(10), None);
    }

    #[test]
    fn filters_become_a_query() {
        let mut lib = LibraryUi {
            text: "  siciliana ".into(),
            result: ResultFilter::BlackWins,
            policy: Some(PolicyKind::FideSimulation),
            source: SourceFilter::Imported,
            page: 2,
            ..Default::default()
        };
        lib.date_from = "2024.01.01".into();
        let q = lib.query();
        assert_eq!(q.text.as_deref(), Some("siciliana"));
        assert_eq!(q.result.as_deref(), Some("0-1"));
        assert_eq!(q.policy, Some(PolicyKind::FideSimulation));
        assert_eq!(q.source, Some(LibrarySource::Imported));
        assert_eq!(q.date_from.as_deref(), Some("2024.01.01"));
        assert!(q.player.is_none() && q.position.is_none() && q.date_to.is_none());
        assert_eq!((q.limit, q.offset), (Some(PAGE_SIZE), 2 * PAGE_SIZE));
    }

    fn temp_dir(tag: &str) -> PathBuf {
        let dir =
            std::env::temp_dir().join(format!("chess-studio-library-{tag}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    fn quick_import(store: &mut Store, path: &Path) -> Result<ImportOutcome, String> {
        import_file(
            store,
            ProfileId(1),
            path,
            ImportSource::Twic,
            &AtomicBool::new(false),
            &mut |_| {},
        )
    }

    #[test]
    fn importing_twice_reports_the_second_time_as_duplicates_with_the_origin() {
        let dir = temp_dir("again");
        let path = dir.join("twic.pgn");
        std::fs::write(&path, DEMO_PGN).unwrap();
        let mut store = Store::open_in_memory().unwrap();
        let first = quick_import(&mut store, &path).unwrap();
        let second = quick_import(&mut store, &path).unwrap();
        assert_eq!((second.imported, second.duplicates), (0, first.imported));
        let id = store
            .list_imported(&chess_store::ImportedQuery::default())
            .unwrap()[0]
            .id;
        let origin = &store.origins(ProfileId(1), id).unwrap()[0];
        assert_eq!(origin.provider.as_deref(), Some("twic"));
        assert_eq!(origin.mode, ImportMode::Manual);
        assert_eq!(origin.source.as_deref(), Some("twic.pgn"));
        std::fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn civil_dates_convert_from_epoch_days() {
        assert_eq!(civil_date(0), "1970-01-01");
        assert_eq!(civil_date(19_723), "2024-01-01");
        assert_eq!(civil_date(20_593), "2026-05-20");
    }

    #[test]
    fn importing_a_file_reports_the_good_games_and_the_errors() {
        let dir = temp_dir("import");
        let path = dir.join("estudos.pgn");
        std::fs::write(
            &path,
            format!("{DEMO_PGN}\n[Event \"Quebrada\"]\n\n1. e4 e5 2. Ke7 *\n"),
        )
        .unwrap();
        let mut store = Store::open_in_memory().unwrap();
        let outcome = quick_import(&mut store, &path).unwrap();
        assert_eq!(outcome.file, "estudos.pgn");
        assert_eq!(outcome.imported, 3);
        assert_eq!(outcome.errors.len(), 1);
        assert!(outcome.errors[0].contains("Ke7"), "{:?}", outcome.errors);
        // As partidas entram na busca.
        let page = store.search_library(&LibraryQuery::default()).unwrap();
        assert_eq!(page.total, 3);
        std::fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn importing_a_missing_file_is_an_error_message() {
        let mut store = Store::open_in_memory().unwrap();
        let err = quick_import(&mut store, Path::new("/nao/existe.pgn")).unwrap_err();
        assert!(err.contains("existe.pgn"));
    }

    #[test]
    fn only_pgn_files_are_suggested_in_order() {
        let dir = temp_dir("list");
        for name in ["b.pgn", "a.PGN", "c.txt"] {
            std::fs::write(dir.join(name), "").unwrap();
        }
        let names: Vec<String> = list_pgn_files(&dir)
            .iter()
            .map(|p| p.file_name().unwrap().to_string_lossy().into_owned())
            .collect();
        assert_eq!(names, ["a.PGN", "b.pgn"]);
        assert!(list_pgn_files(Path::new("/nao/existe")).is_empty());
        std::fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn exercises_are_found_by_kind_and_the_catalog_loads_on_first_use() {
        let mut store = Store::open_in_memory().unwrap();
        let all = exercises_for(&mut store, None, "").unwrap();
        assert!(!all.is_empty(), "catálogo embutido gravado na primeira vez");
        let mates = exercises_for(&mut store, Some(ExerciseKind::Mate), "").unwrap();
        assert!(!mates.is_empty() && mates.len() < all.len());
        assert!(mates.iter().all(|e| e.kind == ExerciseKind::Mate));
    }

    #[test]
    fn marking_a_favorite_stores_the_position_on_screen_and_the_origin_game() {
        use chess_game::Command;
        let mut lib = LibraryUi::default();
        let mut session = Session::new();
        session.command(Command::Release {
            from: "e2".parse().unwrap(),
            to: "e4".parse().unwrap(),
        });
        let i18n = I18n::pt_br();
        // Sem persistência o banco é o local em memória.
        apply(
            Action::MarkFavorite {
                name: "  Minha  ".into(),
            },
            &mut lib,
            &mut session,
            None,
            &i18n,
        );
        let favorites = with_store(&mut lib.local, None, |store, _| {
            store.list_favorites(&FavoriteQuery::default()).unwrap()
        })
        .unwrap();
        assert_eq!(favorites.len(), 1);
        assert_eq!(favorites[0].name, "Minha");
        assert_eq!(favorites[0].fen, session.snapshot().position.to_fen());
        // Revisando o Lance 0, a favorita é a Posição inicial (a que está na tela).
        session.set_review_ply(Some(0));
        apply(
            Action::MarkFavorite {
                name: String::new(),
            },
            &mut lib,
            &mut session,
            None,
            &i18n,
        );
        let count = with_store(&mut lib.local, None, |store, _| {
            store
                .list_favorites(&FavoriteQuery::default())
                .unwrap()
                .len()
        })
        .unwrap();
        assert_eq!(count, 2);
    }

    #[test]
    fn a_favorite_opens_as_an_assisted_study_game() {
        let mut lib = LibraryUi::default();
        let mut session = Session::new();
        let i18n = I18n::pt_br();
        let epoch = session.epoch();
        let fen = "4k3/8/8/8/8/8/4P3/4K3 w - - 0 1";
        apply(
            Action::StudyFen(fen.into()),
            &mut lib,
            &mut session,
            None,
            &i18n,
        );
        assert_ne!(
            session.epoch(),
            epoch,
            "Partida nova (a persistência a grava)"
        );
        assert_eq!(session.snapshot().position.to_fen(), fen);
        assert_eq!(session.snapshot().policy, PolicyKind::Assisted);
        // FEN inválido não troca nada.
        let epoch = session.epoch();
        apply(
            Action::StudyFen("lixo".into()),
            &mut lib,
            &mut session,
            None,
            &i18n,
        );
        assert_eq!(session.epoch(), epoch);
        assert!(lib.status.is_some());
    }

    #[test]
    fn opening_an_imported_game_lists_the_tree_and_studying_replays_the_mainline() {
        let mut lib = LibraryUi::default();
        let mut session = Session::new();
        let i18n = I18n::pt_br();
        with_store(&mut lib.local, None, |store, _| {
            store.import_pgn(DEMO_PGN, None).unwrap();
        });
        let id = with_store(&mut lib.local, None, |store, _| {
            store
                .search_library(&LibraryQuery {
                    text: Some("siciliana".into()),
                    ..Default::default()
                })
                .unwrap()
                .entries[0]
                .ref_id
        })
        .unwrap();
        apply(
            Action::OpenImported(ImportedId(id)),
            &mut lib,
            &mut session,
            None,
            &i18n,
        );
        let detail = lib.detail.as_ref().unwrap();
        assert_eq!(detail.title, "Ana Álvares × Beto Lima");
        assert_eq!(detail.lines.len(), 2, "principal + 1 variação");
        let events = session.game().events().len();
        apply(
            Action::StudyMainline { ply: 3 },
            &mut lib,
            &mut session,
            None,
            &i18n,
        );
        assert_eq!(session.snapshot().history.len(), 10);
        assert_eq!(session.review().unwrap().ply, 3);
        assert!(session.game().events().len() > events);
    }
}
