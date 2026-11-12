//! Vários perfis locais (US-29, DEC-50 estendida na V1). Um **perfil local** é um jogador
//! deste computador: nome, preferências, Perfis de interface, controles, Partidas e
//! Resoluções são dele. Esta tela cria, renomeia, troca e exclui perfis locais; o seletor da
//! abertura aparece quando há mais de um.
//!
//! Não confundir com o Perfil de interface (`profiles`), com o Perfil de auxílios nem com o
//! Perfil de treino: aqueles são *dentro* de um perfil local.
//!
//! Trocar de perfil grava a Partida que sai (retomável depois), carrega a do perfil que
//! entra (em Pausa, DEC-42) e reaplica as preferências. É só Apresentação e preferência:
//! nenhuma regra da Partida muda (ADR-0001).

use crate::aids::AidSettings;
use crate::controls::Controls;
use crate::dock::Dock;
use crate::engine::EngineUi;
use crate::focus::ViewState;
use crate::i18n::I18n;
use crate::persistence::{Persistence, PersistenceHandle, ProfileData};
use crate::presentation::{GraphicsQuality, Presentation, UltraEffects};
use crate::profiles::InterfaceProfiles;
use crate::session::Session;
use crate::study::StudyNotes;
use crate::training_ui::TrainingUi;
use bevy::ecs::system::SystemParam;
use bevy::prelude::*;
use bevy_egui::{EguiContexts, EguiPrimaryContextPass, egui};
use chess_store::{ProfileId, ProfileInfo, StoreError};

/// Pedido da interface, atendido por [`run_ops`] fora do quadro de desenho.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Op {
    Switch(ProfileId),
    /// Cria e já troca para o novo perfil.
    Create(String),
    Rename(ProfileId, String),
    Delete(ProfileId),
}

/// Por que um pedido não foi atendido (chave i18n).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OpError {
    InvalidName,
    NameTaken,
    Busy,
    /// O Cofre não apagou as autorizações do Perfil: nada foi removido.
    Vault,
    Other,
}

impl OpError {
    fn key(self) -> &'static str {
        match self {
            OpError::InvalidName => "lp-error-name",
            OpError::NameTaken => "lp-error-taken",
            OpError::Busy => "lp-error-busy",
            OpError::Vault => "lp-error-vault",
            OpError::Other => "lp-error-other",
        }
    }
}

impl From<&StoreError> for OpError {
    fn from(err: &StoreError) -> Self {
        match err {
            StoreError::InvalidProfileName => OpError::InvalidName,
            StoreError::ProfileNameTaken(_) => OpError::NameTaken,
            _ => OpError::Other,
        }
    }
}

/// Perfis locais conhecidos e o estado das telas de perfis.
#[derive(Resource, Debug, Default)]
pub struct LocalProfiles {
    list: Vec<ProfileInfo>,
    active: Option<ProfileId>,
    /// Seletor da abertura aberto.
    pub picker_open: bool,
    /// Janela "Perfis locais" aberta.
    pub manager_open: bool,
    new_name: String,
    /// Perfil sendo renomeado e o texto em edição.
    renaming: Option<(ProfileId, String)>,
    confirm_delete: Option<ProfileId>,
    error: Option<OpError>,
    ops: Vec<Op>,
}

impl LocalProfiles {
    /// Lê a lista do banco; o seletor da abertura abre se houver mais de um perfil.
    pub fn from_persistence(persistence: &Persistence) -> Self {
        let mut state = Self::default();
        state.refresh(persistence);
        state.picker_open = state.list.len() > 1;
        state
    }

    /// Perfis de exemplo para as capturas (sem banco): `--profile-picker`.
    pub fn sample(picker: bool, manager: bool) -> Self {
        let named = |id: i64, name: &str| ProfileInfo {
            id: ProfileId(id),
            name: name.to_owned(),
            created_at_ms: id,
            last_used_at_ms: 0,
        };
        Self {
            list: vec![named(1, "Ana"), named(2, "Beto"), named(3, "Clara")],
            active: Some(ProfileId(1)),
            picker_open: picker,
            manager_open: manager,
            ..Self::default()
        }
    }

    fn refresh(&mut self, persistence: &Persistence) {
        self.list = persistence.profiles();
        self.active = Some(persistence.profile());
    }

    pub fn list(&self) -> &[ProfileInfo] {
        &self.list
    }

    pub fn active(&self) -> Option<ProfileId> {
        self.active
    }

    pub fn active_name(&self) -> Option<&str> {
        let active = self.active?;
        self.list
            .iter()
            .find(|p| p.id == active)
            .map(|p| p.name.as_str())
    }

    /// Enfileira um pedido (a interface nunca mexe no banco).
    pub fn request(&mut self, op: Op) {
        self.error = None;
        self.ops.push(op);
    }

    #[cfg(test)]
    pub fn error(&self) -> Option<OpError> {
        self.error
    }
}

pub struct LocalProfilesPlugin;

impl Plugin for LocalProfilesPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<LocalProfiles>()
            .add_systems(Update, run_ops)
            .add_systems(EguiPrimaryContextPass, (picker_window, manager_window));
    }
}

/// Tudo que a troca de perfil reaplica.
#[derive(SystemParam)]
struct Applied<'w> {
    session: ResMut<'w, Session>,
    aids: ResMut<'w, AidSettings>,
    presentation: ResMut<'w, Presentation>,
    quality: ResMut<'w, GraphicsQuality>,
    effects: ResMut<'w, UltraEffects>,
    coordinates: ResMut<'w, crate::coordinates::BoardCoordinates>,
    hover: ResMut<'w, crate::hover::HoverHighlight>,
    appearance: ResMut<'w, crate::ui_theme::Appearance>,
    view: ResMut<'w, ViewState>,
    dock: ResMut<'w, Dock>,
    hidden: ResMut<'w, crate::actions::HiddenPanels>,
    screen_layouts: ResMut<'w, crate::screen_layouts::ScreenLayouts>,
    ribbon: Option<ResMut<'w, crate::ribbon::RibbonState>>,
    interface: ResMut<'w, InterfaceProfiles>,
    engine: ResMut<'w, EngineUi>,
    notes: ResMut<'w, StudyNotes>,
    controls: ResMut<'w, Controls>,
}

impl Applied<'_> {
    fn apply(&mut self, data: ProfileData) {
        let ProfileData {
            prefs,
            restored,
            layout,
            interface,
            controls,
            notes,
            ribbon,
            hidden,
            screen_layouts,
        } = data;
        let mut session = restored.session;
        session.supersede(&self.session);
        let epoch = session.epoch();
        *self.session = session;
        *self.aids = prefs.aids;
        *self.presentation = prefs.presentation;
        *self.quality = prefs.quality;
        *self.effects = prefs.ultra_effects;
        *self.coordinates = prefs.coordinates;
        *self.hover = prefs.hover_highlight;
        *self.appearance = prefs.appearance;
        *self.view = prefs.view;
        self.dock.set_layout(layout);
        *self.hidden = hidden;
        *self.screen_layouts = screen_layouts;
        if let (Some(setup), Some(state)) = (&ribbon, self.ribbon.as_mut()) {
            state.apply_setup(setup);
        }
        *self.interface = InterfaceProfiles::with_state(interface);
        self.engine
            .apply_saved(prefs.engine_user_side, prefs.engine_elo);
        *self.notes = StudyNotes::loaded(epoch, notes);
        *self.controls = Controls::with_map(controls);
    }
}

fn run_ops(
    mut state: ResMut<LocalProfiles>,
    handle: Option<Res<PersistenceHandle>>,
    training: Res<TrainingUi>,
    net: Option<Res<crate::net_service::NetService>>,
    mut applied: Applied,
) {
    if state.ops.is_empty() {
        return;
    }
    let ops = std::mem::take(&mut state.ops);
    // Sem banco (capturas, benchmark) não há perfis para trocar.
    let Some(handle) = handle else { return };
    let Ok(mut persistence) = handle.0.lock() else {
        return;
    };
    for op in ops {
        let vault = net.as_deref().map(crate::net_service::NetService::vault);
        if let Err(error) = execute(&mut persistence, &training, &mut applied, vault, op) {
            state.error = Some(error);
        }
    }
    state.refresh(&persistence);
    // Um perfil excluído ou trocado fecha edições pendentes.
    state.renaming = None;
    state.confirm_delete = None;
}

fn execute(
    persistence: &mut Persistence,
    training: &TrainingUi,
    applied: &mut Applied,
    vault: Option<&chess_vault::SecretVault>,
    op: Op,
) -> Result<(), OpError> {
    let fail = |e: StoreError| OpError::from(&e);
    match op {
        Op::Switch(id) => {
            if id == persistence.profile() {
                return Ok(());
            }
            // Durante um Exercício a `Session` não é a Partida do usuário.
            if training.session_swapped() {
                return Err(OpError::Busy);
            }
            let data = persistence
                .switch_profile(id, Some(&applied.session))
                .map_err(fail)?;
            applied.apply(data);
        }
        Op::Create(name) => {
            let id = persistence.create_profile(&name).map_err(fail)?;
            return execute(persistence, training, applied, vault, Op::Switch(id));
        }
        Op::Rename(id, name) => persistence.rename_profile(id, &name).map_err(fail)?,
        Op::Delete(id) => delete_profile(persistence, vault, id)?,
    }
    Ok(())
}

/// Exclui o Perfil local `id`. Antes, os segredos dele saem do Cofre: o `id` das Conexões é a
/// chave deles, e um segredo órfão sobraria para sempre. Se o Cofre recusar, o Perfil fica. O
/// Perfil em uso nunca é excluído (troque antes).
fn delete_profile(
    persistence: &mut Persistence,
    vault: Option<&chess_vault::SecretVault>,
    id: ProfileId,
) -> Result<(), OpError> {
    match vault {
        Some(vault) if id != persistence.profile() => {
            let (store, _) = persistence.store_and_profile();
            crate::secrets::forget_profile(store, vault, id).map_err(|error| match error {
                crate::secrets::ForgetError::Vault(_) => OpError::Vault,
                crate::secrets::ForgetError::Store(_) => OpError::Other,
            })
        }
        _ => persistence
            .delete_profile(id)
            .map_err(|e| OpError::from(&e)),
    }
}

/// Seção da janela principal: perfil em uso, troca rápida e acesso ao gerenciador.
pub fn section(ui: &mut egui::Ui, i18n: &I18n, state: &mut LocalProfiles, blocked: bool) {
    // Sem banco (capturas, benchmark) não há perfis para mostrar.
    let Some(active) = state.active else {
        return;
    };
    let mut chosen = None;
    ui.add_enabled_ui(!blocked, |ui| {
        egui::ComboBox::from_label(i18n.t("lp-current"))
            .selected_text(state.active_name().unwrap_or_default())
            .show_ui(ui, |ui| {
                for profile in &state.list {
                    if ui
                        .selectable_label(profile.id == active, &profile.name)
                        .clicked()
                    {
                        chosen = Some(profile.id);
                    }
                }
            });
    });
    if let Some(id) = chosen {
        state.request(Op::Switch(id));
    }
    if blocked {
        ui.small(i18n.t("lp-busy"));
    }
    if ui.button(i18n.t("lp-manage")).clicked() {
        state.manager_open = true;
    }
}

/// Seletor da abertura: aparece quando há mais de um perfil local.
fn picker_window(
    mut contexts: EguiContexts,
    i18n: Res<I18n>,
    mut state: ResMut<LocalProfiles>,
) -> Result {
    if !state.picker_open {
        return Ok(());
    }
    let ctx = contexts.ctx_mut()?;
    // Escurece o resto: nada além do seletor recebe cliques.
    egui::Area::new(egui::Id::new("lp-picker-dim"))
        .order(egui::Order::Foreground)
        .fixed_pos(egui::Pos2::ZERO)
        .show(ctx, |ui| {
            let rect = ctx.viewport_rect();
            ui.allocate_rect(rect, egui::Sense::click_and_drag());
            ui.painter()
                .rect_filled(rect, 0.0, egui::Color32::from_black_alpha(170));
        });
    let active = state.active;
    let mut chosen = None;
    let mut keep_current = false;
    egui::Window::new(i18n.t("lp-picker-title"))
        .order(egui::Order::Foreground)
        .collapsible(false)
        .resizable(false)
        .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
        .show(ctx, |ui| {
            ui.label(i18n.t("lp-picker-prompt"));
            ui.add_space(6.0);
            for profile in &state.list {
                let last = Some(profile.id) == active;
                let label = if last {
                    i18n.t_with("lp-picker-last", &[("name", &profile.name)])
                } else {
                    profile.name.clone()
                };
                let button = egui::Button::new(egui::RichText::new(label).size(16.0))
                    .min_size(egui::vec2(260.0, 32.0));
                if ui.add(button).clicked() {
                    if last {
                        keep_current = true;
                    } else {
                        chosen = Some(profile.id);
                    }
                }
            }
        });
    if let Some(id) = chosen {
        state.picker_open = false;
        state.request(Op::Switch(id));
    } else if keep_current {
        state.picker_open = false;
    }
    Ok(())
}

/// Janela "Perfis locais": criar, renomear, trocar e excluir.
fn manager_window(
    mut contexts: EguiContexts,
    i18n: Res<I18n>,
    mut state: ResMut<LocalProfiles>,
    training: Res<TrainingUi>,
) -> Result {
    if !state.manager_open {
        return Ok(());
    }
    let ctx = contexts.ctx_mut()?;
    let busy = training.session_swapped();
    let mut open = true;
    // Edita uma cópia do estado de tela e devolve ao fim (os pedidos vão por `request`).
    let list = state.list.clone();
    let active = state.active;
    let can_delete = list.len() > 1;
    let mut requests: Vec<Op> = Vec::new();
    let mut new_name = std::mem::take(&mut state.new_name);
    let mut renaming = state.renaming.take();
    let mut confirm_delete = state.confirm_delete.take();
    let error = state.error;
    egui::Window::new(i18n.t("lp-manager-title"))
        .open(&mut open)
        .default_width(420.0)
        .show(ctx, |ui| {
            ui.small(i18n.t("lp-manager-intro"));
            ui.add_space(4.0);
            for profile in &list {
                let is_active = Some(profile.id) == active;
                ui.horizontal(|ui| {
                    if let Some((id, buffer)) = renaming.as_mut()
                        && *id == profile.id
                    {
                        let edit = ui.add(
                            egui::TextEdit::singleline(buffer)
                                .desired_width(160.0)
                                .char_limit(chess_store::MAX_PROFILE_NAME_LEN),
                        );
                        let commit = ui.button(i18n.t("lp-save")).clicked()
                            || (edit.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter)));
                        let name = buffer.clone();
                        if commit {
                            requests.push(Op::Rename(profile.id, name));
                            renaming = None;
                        } else if ui.button(i18n.t("ctl-cancel")).clicked() {
                            renaming = None;
                        }
                        return;
                    }
                    let name = egui::RichText::new(&profile.name);
                    ui.label(if is_active { name.strong() } else { name });
                    if is_active {
                        ui.weak(i18n.t("lp-in-use"));
                    } else if ui
                        .add_enabled(!busy, egui::Button::new(i18n.t("lp-use")))
                        .clicked()
                    {
                        requests.push(Op::Switch(profile.id));
                    }
                    if ui.button(i18n.t("lp-rename")).clicked() {
                        renaming = Some((profile.id, profile.name.clone()));
                        confirm_delete = None;
                    }
                    if confirm_delete == Some(profile.id) {
                        ui.colored_label(ui.visuals().error_fg_color, i18n.t("lp-confirm-delete"));
                        if ui.button(i18n.t("lp-delete-yes")).clicked() {
                            requests.push(Op::Delete(profile.id));
                            confirm_delete = None;
                        }
                        if ui.button(i18n.t("ctl-cancel")).clicked() {
                            confirm_delete = None;
                        }
                    } else if ui
                        .add_enabled(
                            can_delete && !is_active,
                            egui::Button::new(i18n.t("lp-delete")),
                        )
                        .on_disabled_hover_text(i18n.t("lp-delete-blocked"))
                        .clicked()
                    {
                        confirm_delete = Some(profile.id);
                        renaming = None;
                    }
                });
            }
            ui.separator();
            ui.label(i18n.t("lp-new"));
            ui.horizontal(|ui| {
                ui.add(
                    egui::TextEdit::singleline(&mut new_name)
                        .hint_text(i18n.t("lp-name-hint"))
                        .desired_width(180.0)
                        .char_limit(chess_store::MAX_PROFILE_NAME_LEN),
                );
                if ui
                    .add_enabled(
                        !busy && !new_name.trim().is_empty(),
                        egui::Button::new(i18n.t("lp-create")),
                    )
                    .clicked()
                {
                    requests.push(Op::Create(new_name.trim().to_owned()));
                    new_name.clear();
                }
            });
            if busy {
                ui.small(i18n.t("lp-busy"));
            }
            if let Some(error) = error {
                ui.colored_label(ui.visuals().error_fg_color, i18n.t(error.key()));
            }
        });
    state.new_name = new_name;
    state.renaming = renaming;
    state.confirm_delete = confirm_delete;
    if !open {
        state.manager_open = false;
        state.error = None;
    }
    for op in requests {
        state.request(op);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use chess_store::Store;

    fn persistence() -> Persistence {
        Persistence::new(Store::open_in_memory().unwrap())
    }

    #[test]
    fn the_picker_opens_only_with_more_than_one_profile() {
        let mut p = persistence();
        assert!(!LocalProfiles::from_persistence(&p).picker_open);
        p.create_profile("Ana").unwrap();
        let state = LocalProfiles::from_persistence(&p);
        assert!(state.picker_open);
        assert_eq!(state.list().len(), 2);
        assert_eq!(state.active(), Some(p.profile()));
    }

    #[test]
    fn deleting_a_local_profile_deletes_its_secrets_first_and_keeps_it_if_the_vault_refuses() {
        use crate::secrets::{VolatileKeyring, net_id, net_profile, volatile_vault_with};
        use chess_net::{ProviderId, Secret};
        use chess_store::{ConnectionKind, ConnectionRole};
        use std::sync::Arc;
        let keyring = Arc::new(VolatileKeyring::new());
        let opened = volatile_vault_with(keyring.clone(), "teste-perfil", None);
        let mut p = persistence();
        let ana = p.create_profile("Ana").unwrap();
        // Uma Conexão autorizada da Ana, com segredo no Cofre.
        let id = {
            let (store, _) = p.store_and_profile();
            let id = store
                .begin_authorization(
                    ana,
                    "lichess",
                    ConnectionRole::Player,
                    ConnectionKind::AuthorizedBrowser,
                )
                .unwrap();
            opened
                .vault
                .store(
                    net_profile(ana),
                    ProviderId::Lichess,
                    net_id(id),
                    Secret::new("lio_SEGREDO"),
                )
                .unwrap();
            id
        };
        assert_eq!(keyring.accounts().len(), 1);
        // O Cofre recusa: o Perfil local fica, com a Conexão.
        keyring.set_failing(true);
        assert_eq!(
            delete_profile(&mut p, Some(&opened.vault), ana),
            Err(OpError::Vault)
        );
        assert!(p.profiles().iter().any(|info| info.id == ana));
        // O perfil em uso nunca se exclui.
        keyring.set_failing(false);
        let current = p.profile();
        assert!(delete_profile(&mut p, Some(&opened.vault), current).is_err());
        // Com o Cofre de volta, o segredo sai e só então o Perfil.
        delete_profile(&mut p, Some(&opened.vault), ana).unwrap();
        assert!(keyring.accounts().is_empty());
        assert!(opened.vault.reference(net_id(id)).is_none());
        assert!(!p.profiles().iter().any(|info| info.id == ana));
    }

    #[test]
    fn store_errors_map_to_messages() {
        assert_eq!(
            OpError::from(&StoreError::InvalidProfileName),
            OpError::InvalidName
        );
        assert_eq!(
            OpError::from(&StoreError::ProfileNameTaken("x".into())),
            OpError::NameTaken
        );
        assert_eq!(OpError::from(&StoreError::LastProfile), OpError::Other);
    }

    #[test]
    fn requests_are_queued_and_clear_the_previous_error() {
        let mut state = LocalProfiles::sample(false, false);
        state.error = Some(OpError::Other);
        state.request(Op::Create("Dani".into()));
        assert_eq!(state.error(), None);
        assert_eq!(state.ops, vec![Op::Create("Dani".into())]);
        assert_eq!(state.active_name(), Some("Ana"));
    }

    /// Com um Exercício no tabuleiro, a Sessão não é a Partida do usuário: o serviço recusa a
    /// troca de Perfil local, e a Partida competitiva que o Treino guarda (com a trava dela,
    /// FP-06) continua guardada, do mesmo Perfil (#304).
    #[test]
    fn switching_profiles_is_refused_while_the_training_keeps_the_users_game() {
        use crate::new_game::NewGameConfig;
        let mut p = persistence();
        let ana = p.profile();
        let beto = p.create_profile("Beto").unwrap();
        let mut session = p.restore().session;
        session.start(&NewGameConfig::for_policy(
            chess_game::PolicyKind::FideSimulation,
        ));
        let mut training = TrainingUi::default();
        training.start_for_tests("afogamento-001", &mut session);
        let mut state = LocalProfiles::from_persistence(&p);
        state.request(Op::Switch(beto));
        let mut app = App::new();
        app.insert_resource(state)
            .insert_resource(PersistenceHandle(std::sync::Mutex::new(p)))
            .insert_resource(training)
            .insert_resource(session)
            .insert_resource(crate::presentation::UltraEffects(false))
            .init_resource::<AidSettings>()
            .init_resource::<Presentation>()
            .init_resource::<GraphicsQuality>()
            .init_resource::<crate::coordinates::BoardCoordinates>()
            .init_resource::<crate::hover::HoverHighlight>()
            .init_resource::<crate::ui_theme::Appearance>()
            .init_resource::<ViewState>()
            .init_resource::<Dock>()
            .init_resource::<crate::actions::HiddenPanels>()
            .init_resource::<crate::screen_layouts::ScreenLayouts>()
            .init_resource::<InterfaceProfiles>()
            .init_resource::<EngineUi>()
            .init_resource::<StudyNotes>()
            .init_resource::<Controls>()
            .add_systems(Update, run_ops);
        app.update();
        let world = app.world();
        assert_eq!(
            world.resource::<LocalProfiles>().error(),
            Some(OpError::Busy)
        );
        assert_eq!(world.resource::<LocalProfiles>().active(), Some(ana));
        let training = world.resource::<TrainingUi>();
        assert!(
            training.session_swapped(),
            "o Exercício continua no tabuleiro"
        );
        assert!(
            training
                .kept()
                .is_some_and(Session::competitive_in_progress),
            "a Partida competitiva continua guardada"
        );
    }
}
