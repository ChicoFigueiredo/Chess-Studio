//! Painel Explorador (CX-9, #226): aberturas (Biblioteca local, Mestres, Lichess, Um jogador)
//! e finais exatos de até 7 peças, mais a linha "Em cache (Lichess)" do Painel Análise.
//!
//! Organização:
//!
//! - `model`: o que cada aba mostra (tabelas, avisos, Origem), sem Bevy nem egui;
//! - `lookup`: o debounce de ~300 ms e a consulta que nunca trava a interface;
//! - `driver`: a regra de domínio (Provedor ligado? Trava de fair play? autorizado?) e o
//!   pedido ao `NetService`; testado com o servidor falso;
//! - `view`: o desenho, só com tokens e widgets do projeto;
//! - este arquivo: o Painel no dock, o estado do Bevy e as Ações que o Painel dispara.
//!
//! Só Apresentação (ADR-0001): o Painel lê a Posição mostrada e, ao clicar num Lance, mostra
//! a variante como a Análise (`Session::show_variation`), sem Comando nem Evento. Fica
//! indisponível numa Partida competitiva em andamento (a Trava de fair play do serviço é a
//! garantia; o Painel só explica) e durante um Exercício de Treino.

mod demo;
mod driver;
mod lookup;
mod model;
mod view;

pub use driver::{CloudDriver, CloudRow, Explorer, Input};
pub use model::{Authorization, Lock, fmt_count, minutes_until};

use crate::actions::ActionRequested;
use crate::dock::{PanelAccess, PanelDef, ids};
use crate::i18n::I18n;
use crate::net_service::NetService;
use crate::notation;
use crate::persistence::PersistenceHandle;
use crate::provider_consent::ConsentDialog;
use crate::session::Session;
use crate::training_ui::{TrainingUi, with_store};
use crate::ui_theme::{cs_dropdown, cs_segmented, cs_text_field};
use bevy::prelude::*;
use bevy_egui::egui;
use chess_core::{CanonicalPosition, Color};
use chess_input::controls::Action;
use chess_openings::OpeningBook;
use chess_store::Store;
use driver::{EndgamesView, OpeningsView};
use model::{Notice, OpeningsTable, Source, Tab};
use std::sync::Mutex;
use std::time::{Duration, Instant};
pub(crate) use view::UiAction;

/// A estatística local se refaz sozinha neste intervalo (importação nova, Partida terminando).
const LIBRARY_REFRESH: Duration = Duration::from_secs(5);

/// Estado do Painel e da linha "Em cache" (um recurso só: o Painel Análise e o Explorador
/// leem a mesma Posição).
#[derive(Resource, Default)]
pub struct ExplorerUi {
    pub(crate) explorer: Explorer,
    pub(crate) cloud: CloudDriver,
    library: LibraryCache,
    /// Lance (SAN) em exibição como variante.
    preview: Option<String>,
    /// Banco em memória quando não há persistência (capturas e testes).
    pub(crate) local: Option<Mutex<Store>>,
    /// Dados de demonstração das capturas (`--explorer-demo`): nenhum pedido sai.
    pub(crate) demo: Option<demo::Scene>,
}

#[derive(Default)]
struct LibraryCache {
    key: Option<(u64, usize, usize, String)>,
    refreshed: Option<Instant>,
    table: Option<OpeningsTable>,
    error: Option<String>,
}

pub struct ExplorerPlugin;

impl Plugin for ExplorerPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<ExplorerUi>()
            .add_systems(Update, demo::apply_demo);
        crate::dock::register_panel(
            app,
            PanelDef {
                id: ids::EXPLORER,
                label_key: "dock-panel-explorer",
                // Sempre acessível: numa Partida competitiva o Painel explica a Trava de fair
                // play em vez de sumir (nada é consultado; o serviço também recusaria).
                access: PanelAccess::Always,
                ui: explorer_panel,
            },
        );
    }
}

/// A Posição que o tabuleiro mostra (a da revisão ou a ao vivo), sem a variante.
struct Shown {
    position: CanonicalPosition,
    fen: String,
    ply: usize,
    total: usize,
}

fn shown(session: &Session) -> Option<Shown> {
    let ply = notation::current_ply(session);
    let total = session.snapshot().history.len();
    let position = session.game().review_at_ply(ply)?.position;
    Some(Shown {
        fen: position.to_fen(),
        position,
        ply,
        total,
    })
}

fn lock_of(session: &Session, training: bool) -> Option<Lock> {
    if session.competitive_in_progress() {
        Some(Lock::Competitive)
    } else if training {
        Some(Lock::Training)
    } else {
        None
    }
}

fn explorer_panel(ui: &mut egui::Ui, i18n: &I18n, world: &mut World) {
    // Lances em SAN são resposta pronta: ocultos com a Súmula manual aberta.
    if world
        .get_resource::<crate::scoresheet::ScoresheetUi>()
        .is_some_and(crate::scoresheet::ScoresheetUi::hides_notation)
    {
        crate::scoresheet::hidden_notice(ui, i18n);
        return;
    }
    if !world.contains_resource::<ExplorerUi>() || !world.contains_resource::<NetService>() {
        return;
    }
    let training = world
        .get_resource::<TrainingUi>()
        .is_some_and(TrainingUi::session_swapped);
    let mut actions = Vec::new();
    world.resource_scope(|world, mut state: Mut<ExplorerUi>| {
        world.resource_scope(|world, mut session: Mut<Session>| {
            world.resource_scope(|world, net: Mut<NetService>| {
                let handle = world.get_resource::<PersistenceHandle>();
                state.draw(ui, i18n, &session, &net, handle, training, &mut actions);
            });
            state.apply_play(&mut session, &actions);
        });
    });
    apply_world_actions(world, i18n, &actions);
}

impl ExplorerUi {
    /// Desenha o Painel. Não toca a Partida: as intenções saem em `actions`.
    #[allow(clippy::too_many_arguments)]
    fn draw(
        &mut self,
        ui: &mut egui::Ui,
        i18n: &I18n,
        session: &Session,
        net: &NetService,
        handle: Option<&PersistenceHandle>,
        training: bool,
        actions: &mut Vec<UiAction>,
    ) {
        let lock = lock_of(session, training);
        let Some(here) = shown(session) else {
            return;
        };
        let now = Instant::now();
        let at = NetService::now();
        let authorization = Authorization::lichess_now(net);
        let input = Input {
            fen: &here.fen,
            now,
            at,
            lock,
            authorization,
        };
        if self.demo.is_none() {
            self.explorer.drive(&input, net);
        }
        if !session.showing_variation() {
            self.preview = None;
        }
        // Trava: o Painel só explica e nada é consultado (o serviço também recusaria).
        if let Some(lock) = lock {
            let notice = match lock {
                Lock::Competitive => Notice::FairPlay,
                Lock::Training => Notice::Training,
            };
            view::notice(ui, i18n, &notice, false, at, actions);
            return;
        }
        self.header(ui, i18n);
        ui.add_space(8.0);
        match self.explorer.tab {
            Tab::Openings => {
                self.openings_tab(ui, i18n, session, net, handle, &here, &input, actions)
            }
            Tab::Endgames => self.endgames_tab(ui, i18n, net, &input, actions),
        }
        if session.showing_variation() {
            ui.add_space(12.0);
            view::muted(ui, i18n.t("explorer-showing"));
            ui.add_space(4.0);
            if ui
                .add(
                    crate::ui_theme::CsButton::new(&i18n.t("explorer-back"))
                        .icon(egui_phosphor::regular::ARROW_U_UP_LEFT),
                )
                .clicked()
            {
                actions.push(UiAction::BackToGame);
            }
        }
        // Enquanto espera a Posição parar ou o pedido chegar, a interface acorda sozinha.
        if self.explorer.is_busy() {
            let wait = self
                .explorer
                .settle_remaining(now)
                .map_or(Duration::from_millis(50), |d| d + Duration::from_millis(5));
            ui.ctx()
                .request_repaint_after(wait.min(Duration::from_millis(100)));
        }
    }

    fn header(&mut self, ui: &mut egui::Ui, i18n: &I18n) {
        let tabs = [
            (Tab::Openings, i18n.t("explorer-tab-openings")),
            (Tab::Endgames, i18n.t("explorer-tab-endgames")),
        ];
        cs_segmented(ui, &mut self.explorer.tab, &tabs);
    }

    #[allow(clippy::too_many_arguments)]
    fn openings_tab(
        &mut self,
        ui: &mut egui::Ui,
        i18n: &I18n,
        session: &Session,
        net: &NetService,
        handle: Option<&PersistenceHandle>,
        here: &Shown,
        input: &Input<'_>,
        actions: &mut Vec<UiAction>,
    ) {
        ui.add_space(4.0);
        view::muted(ui, i18n.t("explorer-source-label"));
        let sources: Vec<(Source, String)> = Source::ALL
            .into_iter()
            .map(|s| (s, view::source_label(i18n, s)))
            .collect();
        ui.scope(|ui| {
            ui.spacing_mut().combo_width = ui.available_width();
            cs_dropdown(ui, "explorer-source", &mut self.explorer.source, &sources);
        });
        if self.explorer.source == Source::Player {
            ui.add_space(4.0);
            cs_text_field(
                ui,
                &mut self.explorer.player,
                &i18n.t("explorer-player-hint"),
                None,
            );
            ui.add_space(4.0);
            let colors = [
                (Color::White, i18n.t("explorer-color-white")),
                (Color::Black, i18n.t("explorer-color-black")),
            ];
            cs_segmented(ui, &mut self.explorer.player_color, &colors);
        }
        ui.add_space(12.0);
        let selected = self.preview.clone();
        if let Some(scene) = self.demo.as_ref().and_then(demo::Scene::openings_view) {
            self.show_openings(ui, i18n, scene, selected.as_deref(), actions);
            return;
        }
        if self.explorer.source == Source::Library {
            self.library_tab(
                ui,
                i18n,
                session,
                handle,
                here,
                selected.as_deref(),
                actions,
            );
            return;
        }
        let view = self.explorer.openings_view(input, net);
        self.show_openings(ui, i18n, view, selected.as_deref(), actions);
    }

    fn show_openings(
        &self,
        ui: &mut egui::Ui,
        i18n: &I18n,
        view: OpeningsView,
        selected: Option<&str>,
        actions: &mut Vec<UiAction>,
    ) {
        match view {
            OpeningsView::Notice(notice) => {
                view::notice(ui, i18n, &notice, false, NetService::now(), actions);
            }
            OpeningsView::Table(table) => {
                view::stale_notice(ui, i18n, &table.origin, NetService::now());
                view::openings_table(ui, i18n, &table, selected, actions);
                view::footer(ui, i18n, &table.origin);
            }
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn library_tab(
        &mut self,
        ui: &mut egui::Ui,
        i18n: &I18n,
        session: &Session,
        handle: Option<&PersistenceHandle>,
        here: &Shown,
        selected: Option<&str>,
        actions: &mut Vec<UiAction>,
    ) {
        self.library.refresh(session, handle, here, &mut self.local);
        if let Some(error) = &self.library.error {
            view::muted(
                ui,
                i18n.t_with("explorer-library-error", &[("error", error)]),
            );
            return;
        }
        match &self.library.table {
            Some(table) if !table.rows.is_empty() => {
                view::openings_table(ui, i18n, table, selected, actions);
                view::footer(ui, i18n, &table.origin);
            }
            Some(table) => {
                // Sem Partidas aqui: a abertura do livro ainda vale como contexto.
                if let Some((eco, name)) = &table.opening {
                    ui.label(
                        egui::RichText::new(
                            i18n.t_with("explorer-opening", &[("eco", eco), ("name", name)]),
                        )
                        .strong(),
                    );
                    ui.add_space(8.0);
                }
                view::notice(
                    ui,
                    i18n,
                    &Notice::EmptyLibrary,
                    false,
                    NetService::now(),
                    actions,
                );
                view::footer(ui, i18n, &table.origin);
            }
            None => {}
        }
    }

    fn endgames_tab(
        &mut self,
        ui: &mut egui::Ui,
        i18n: &I18n,
        net: &NetService,
        input: &Input<'_>,
        actions: &mut Vec<UiAction>,
    ) {
        let view = match self.demo.as_ref().and_then(demo::Scene::endgames_view) {
            Some(view) => view,
            None => self.explorer.endgames_view(input, net),
        };
        match view {
            EndgamesView::Notice(notice) => {
                view::notice(ui, i18n, &notice, true, NetService::now(), actions);
            }
            EndgamesView::Table(table) => {
                view::stale_notice(ui, i18n, &table.origin, NetService::now());
                view::endgames_table(ui, i18n, &table, actions);
                view::footer(ui, i18n, &table.origin);
            }
        }
    }

    /// Mostra o Lance como variante (somente leitura): nenhum Comando, nenhum Evento.
    fn apply_play(&mut self, session: &mut Session, actions: &[UiAction]) {
        for action in actions {
            match action {
                UiAction::Play(san) => self.show(session, san),
                UiAction::BackToGame => {
                    let ply = notation::current_ply(session);
                    notation::navigate(session, ply);
                    self.preview = None;
                }
                UiAction::Retry => {
                    self.explorer.retry();
                    self.cloud.retry();
                }
                UiAction::UseLibrary => self.explorer.source = Source::Library,
                _ => {}
            }
        }
    }

    fn show(&mut self, session: &mut Session, san: &str) {
        // A Partida competitiva nunca é tocada, nem para mostrar variante (ADR-0001).
        if session.competitive_in_progress() {
            return;
        }
        let ply = notation::current_ply(session);
        let total = session.snapshot().history.len();
        let Some(base) = session.game().review_at_ply(ply).map(|v| v.position) else {
            return;
        };
        if let Some(view) = crate::openings::variation_after(&base, ply, total, san) {
            session.show_variation(view);
            self.preview = Some(san.to_owned());
        }
    }
}

impl LibraryCache {
    /// Consulta o livro e o índice quando a Posição mudou ou a cada [`LIBRARY_REFRESH`].
    fn refresh(
        &mut self,
        session: &Session,
        handle: Option<&PersistenceHandle>,
        here: &Shown,
        local: &mut Option<Mutex<Store>>,
    ) {
        let key = (session.epoch(), here.ply, here.total, here.fen.clone());
        let due = self
            .refreshed
            .is_none_or(|at| at.elapsed() >= LIBRARY_REFRESH);
        if self.key.as_ref() == Some(&key) && !due {
            return;
        }
        self.key = Some(key);
        self.refreshed = Some(Instant::now());
        let opening = crate::openings::opening_at(OpeningBook::embedded(), session, here.ply)
            .map(|o| (o.eco, o.name));
        let stats = with_store(local, handle, |store, profile| {
            store.opening_stats(profile, &here.fen)
        });
        match stats {
            Some(Ok(stats)) => {
                self.error = None;
                self.table = Some(OpeningsTable::from_library(&stats, opening));
            }
            Some(Err(err)) => {
                self.error = Some(err.to_string());
                self.table = None;
            }
            None => self.table = Some(OpeningsTable::from_library(&[], opening)),
        }
    }
}

/// As intenções que dependem do `World` (diálogo, Ações, Biblioteca).
fn apply_world_actions(world: &mut World, i18n: &I18n, actions: &[UiAction]) {
    for action in actions {
        match action {
            UiAction::TurnOn(provider) => {
                ask_to_turn_on(world, i18n, *provider, "explorer-feature-name");
            }
            UiAction::OpenConnections => {
                world.write_message(ActionRequested(Action::OpenConnections));
            }
            UiAction::OpenLibrary => {
                world.write_message(ActionRequested(Action::OpenLibrary));
            }
            UiAction::SearchReference { white, .. } => {
                if let Some(mut library) = world.get_resource_mut::<crate::library_ui::LibraryUi>()
                {
                    library.focus_player(white);
                }
                world.write_message(ActionRequested(Action::OpenLibrary));
            }
            UiAction::Play(_) | UiAction::Retry | UiAction::BackToGame | UiAction::UseLibrary => {}
        }
    }
}

/// Abre o diálogo "Ligar <Provedor>?" com o nome do recurso na frase.
pub fn ask_to_turn_on(
    world: &mut World,
    i18n: &I18n,
    provider: chess_net::ProviderId,
    feature_key: &str,
) {
    let feature = i18n.t(feature_key);
    world.resource_scope(|world, net: Mut<NetService>| {
        if let Some(mut dialog) = world.get_resource_mut::<ConsentDialog>() {
            let _ = net.ensure_enabled(provider, Some(&feature), &mut dialog);
        }
    });
}

// --- Linha "Em cache (Lichess)" do Painel Análise -------------------------------------

/// O que a linha "Em cache" mostra neste quadro, consultado antes de desenhar.
pub struct CloudPrepared {
    row: Option<CloudRow>,
    white_to_move: bool,
}

/// Consulta (sem bloquear) a avaliação em cache da Posição mostrada. Separada da Engine
/// local: nunca a substitui nem se mistura com ela. Respeita a Trava: numa Partida
/// competitiva ou num Exercício nada é pedido.
pub fn cloud_prepare(world: &mut World, session: &Session) -> CloudPrepared {
    let training = world
        .get_resource::<TrainingUi>()
        .is_some_and(TrainingUi::session_swapped);
    let lock = lock_of(session, training);
    let Some(here) = shown(session).filter(|_| world.contains_resource::<ExplorerUi>()) else {
        return CloudPrepared {
            row: None,
            white_to_move: true,
        };
    };
    let white_to_move = here.position.side_to_move() == Color::White;
    let row = world.resource_scope(|world, mut state: Mut<ExplorerUi>| {
        if let Some(row) = state.demo.as_ref().and_then(demo::Scene::cloud_row) {
            return Some(row);
        }
        let net = world.get_resource::<NetService>()?;
        Some(
            state
                .cloud
                .drive(&here.position, Instant::now(), NetService::now(), lock, net),
        )
    });
    CloudPrepared { row, white_to_move }
}

/// Desenha a linha "Em cache (Lichess)"; as intenções saem em `actions`.
pub fn cloud_draw(
    ui: &mut egui::Ui,
    i18n: &I18n,
    prepared: &CloudPrepared,
    actions: &mut Vec<UiAction>,
) {
    if let Some(row) = &prepared.row {
        view::cloud_row(ui, i18n, row, prepared.white_to_move, actions);
    }
}

/// Aplica as intenções da linha (ligar o Provedor, tentar de novo) e acorda a interface
/// enquanto a consulta espera.
pub fn cloud_finish(world: &mut World, i18n: &I18n, actions: &[UiAction]) {
    if actions.contains(&UiAction::Retry)
        && let Some(mut state) = world.get_resource_mut::<ExplorerUi>()
    {
        state.cloud.retry();
    }
    apply_world_actions(world, i18n, actions);
}

/// A linha "Em cache" espera a Posição parar ou o pedido chegar.
pub fn cloud_wait(world: &World) -> Option<Duration> {
    let state = world.get_resource::<ExplorerUi>()?;
    state.cloud.is_busy().then(|| {
        state
            .cloud
            .settle_remaining(Instant::now())
            .map_or(Duration::from_millis(50), |d| d + Duration::from_millis(5))
            .min(Duration::from_millis(100))
    })
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;
    use crate::new_game::NewGameConfig;
    use chess_game::{Command, PolicyKind, TimeControl};

    fn assisted_with_e4() -> Session {
        let mut session = Session::new();
        session.start(&NewGameConfig::default(), None);
        session.command(Command::Release {
            from: "e2".parse().unwrap(),
            to: "e4".parse().unwrap(),
        });
        session
    }

    fn competitive() -> Session {
        let mut session = Session::new();
        session.start(
            &NewGameConfig {
                policy: PolicyKind::FideSimulation,
                time: Some(TimeControl {
                    base_ms: 600_000,
                    increment_ms: 5_000,
                }),
                ..NewGameConfig::default()
            },
            None,
        );
        session
    }

    #[test]
    fn clicking_a_move_shows_a_read_only_variation_and_never_touches_the_game() {
        let mut session = assisted_with_e4();
        let before = session.snapshot().clone();
        let events = session.game().events().len();
        let mut state = ExplorerUi::default();
        state.show(&mut session, "e5");
        assert!(session.showing_variation());
        assert_eq!(state.preview.as_deref(), Some("e5"));
        // Nenhum Comando, nenhum Evento: a Partida é a mesma.
        assert_eq!(session.snapshot(), &before);
        assert_eq!(session.game().events().len(), events);
        // Um Lance ilegal ali não faz nada.
        let mut other = assisted_with_e4();
        state.show(&mut other, "Qh8");
        assert!(!other.showing_variation());
        // Voltar à Partida sai da variante.
        state.apply_play(&mut session, &[UiAction::BackToGame]);
        assert!(!session.showing_variation());
        assert_eq!(session.snapshot(), &before);
    }

    #[test]
    fn a_competitive_game_in_progress_never_shows_a_variation() {
        let mut session = competitive();
        assert!(session.competitive_in_progress());
        let before = session.snapshot().clone();
        let events = session.game().events().len();
        let mut state = ExplorerUi::default();
        state.show(&mut session, "e4");
        assert!(!session.showing_variation());
        assert_eq!(state.preview, None);
        assert_eq!(session.snapshot(), &before);
        assert_eq!(session.game().events().len(), events);
    }

    #[test]
    fn the_lock_follows_the_game_and_the_training_exercise() {
        let mut session = Session::new();
        assert_eq!(lock_of(&session, false), None);
        assert_eq!(lock_of(&session, true), Some(Lock::Training));
        session = competitive();
        // A competitiva vence o Treino na explicação.
        assert_eq!(lock_of(&session, true), Some(Lock::Competitive));
    }
}
