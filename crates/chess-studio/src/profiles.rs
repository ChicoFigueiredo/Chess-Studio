//! Perfis de interface (C4, UI-09/UI-11). Um Perfil de interface agrupa a Apresentação, o
//! layout do dock (painéis visíveis, abas, larguras), o Perfil de auxílios, o modo Foco e a
//! Vista da câmera. Trocar de perfil aplica tudo isso **sem tocar na Partida** (ADR-0001):
//! é só Apresentação e preferência, gravada no SQLite.
//!
//! Não confundir com o Perfil de auxílios (`aids::AidProfile`), que é só uma parte deste.

use crate::aids::AidSettings;
use crate::camera::{CameraMemory, CameraRig, CameraSet};
use crate::dock::{Dock, DockLayout, ids};
use crate::focus::ViewState;
use crate::presentation::Presentation;
use bevy::prelude::*;
use bevy_egui::egui;
use chess_input::CameraView;
use serde::{Deserialize, Serialize};

/// Id reservado do painel de Análise (C8); ignorado enquanto não estiver registrado.
pub const PANEL_ANALYSIS: &str = "Analysis";
/// Id reservado do painel da Biblioteca (C7); ignorado enquanto não estiver registrado.
pub const PANEL_LIBRARY: &str = "Library";

/// Perfis de fábrica (prompt mestre §9). "Personalizado" não é um deles: é o estado atual.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum FactoryProfile {
    PhysicalGame,
    AssistedTraining,
    StudyAnalysis,
}

/// Qual perfil está em uso.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum ProfileKey {
    Factory(FactoryProfile),
    User(String),
    /// O estado atual, que não coincide com nenhum perfil.
    #[default]
    Custom,
}

/// Painéis do dock de um perfil.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum DockSetup {
    /// Só quais painéis ficam visíveis (ids estáveis); ids ausentes do layout são ignorados.
    Panels { visible: Vec<String> },
    /// O layout inteiro (abas, divisões, larguras), de um perfil salvo pelo usuário.
    Layout(Box<DockLayout>),
}

/// A parte do estado de interface que um perfil controla (a câmera vai à parte).
#[derive(Clone, Debug, PartialEq)]
pub struct InterfaceState {
    pub presentation: Presentation,
    pub dock: DockLayout,
    pub aids: AidSettings,
    pub focus: bool,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct InterfaceProfile {
    pub presentation: Presentation,
    pub dock: DockSetup,
    pub aids: AidSettings,
    pub focus: bool,
    /// Vista da câmera para a Apresentação do perfil; `None` mantém a que o usuário já tem
    /// (UI-11: as preferências de câmera por Apresentação sobrevivem à troca).
    pub camera_view: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct UserProfile {
    pub name: String,
    pub profile: InterfaceProfile,
}

/// Por que um nome de perfil foi recusado.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NameError {
    Empty,
    TooLong,
}

pub const MAX_NAME_LEN: usize = 40;

/// Perfil ativo e perfis do usuário; é o que vai para o SQLite.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct ProfilesState {
    pub active: ProfileKey,
    pub user: Vec<UserProfile>,
}

impl FactoryProfile {
    pub const ALL: [FactoryProfile; 3] = [
        FactoryProfile::PhysicalGame,
        FactoryProfile::AssistedTraining,
        FactoryProfile::StudyAnalysis,
    ];

    pub fn label_key(self) -> &'static str {
        match self {
            FactoryProfile::PhysicalGame => "iprofile-physical",
            FactoryProfile::AssistedTraining => "iprofile-training",
            FactoryProfile::StudyAnalysis => "iprofile-study",
        }
    }

    /// O perfil de fábrica. "Relógio visível": na Mesa imersiva o relógio é o da mesa, então
    /// o dock fica oculto sem esconder o relógio.
    pub fn profile(self) -> InterfaceProfile {
        let panels = |ids: &[&str]| DockSetup::Panels {
            visible: ids.iter().map(|id| (*id).to_owned()).collect(),
        };
        match self {
            FactoryProfile::PhysicalGame => InterfaceProfile {
                presentation: Presentation::ImmersiveTable,
                dock: panels(&[]),
                aids: AidSettings::physical(),
                focus: false,
                camera_view: None,
            },
            FactoryProfile::AssistedTraining => InterfaceProfile {
                presentation: Presentation::Professional3d,
                dock: panels(&[ids::NOTATION, ids::AIDS, ids::TRAINING]),
                aids: AidSettings::training(),
                focus: false,
                camera_view: None,
            },
            FactoryProfile::StudyAnalysis => InterfaceProfile {
                presentation: Presentation::Technical2d,
                dock: panels(&[ids::NOTATION, PANEL_ANALYSIS, PANEL_LIBRARY, ids::STUDY]),
                aids: AidSettings::training(),
                focus: false,
                camera_view: None,
            },
        }
    }
}

impl InterfaceProfile {
    /// Aplica o perfil ao estado. Idempotente.
    pub fn apply(&self, state: &mut InterfaceState) {
        state.presentation = self.presentation;
        state.aids = self.aids;
        state.focus = self.focus;
        match &self.dock {
            DockSetup::Layout(layout) => state.dock = (**layout).clone(),
            DockSetup::Panels { visible } => {
                for id in state.dock.pane_ids() {
                    state.dock.set_visible(&id, visible.contains(&id));
                }
                // O primeiro da lista fica na frente do seu grupo de abas.
                for id in visible.iter().rev() {
                    state.dock.activate(id);
                }
            }
        }
    }

    /// O estado coincide com o que este perfil aplicaria?
    pub fn matches(&self, state: &InterfaceState) -> bool {
        let mut applied = state.clone();
        self.apply(&mut applied);
        // A aba ativa de cada grupo não conta: trocar de aba não sai do perfil.
        applied.presentation == state.presentation
            && applied.aids == state.aids
            && applied.focus == state.focus
            && match &self.dock {
                DockSetup::Layout(_) => applied.dock == state.dock,
                DockSetup::Panels { .. } => applied
                    .dock
                    .pane_ids()
                    .iter()
                    .all(|id| applied.dock.is_visible(id) == state.dock.is_visible(id)),
            }
    }

    /// "Salvar o estado atual como perfil": guarda layout inteiro e a Vista informada.
    pub fn capture(state: &InterfaceState, camera_view: Option<&str>) -> Self {
        Self {
            presentation: state.presentation,
            dock: DockSetup::Layout(Box::new(state.dock.clone())),
            aids: state.aids,
            focus: state.focus,
            camera_view: camera_view.map(str::to_owned),
        }
    }
}

impl ProfilesState {
    pub fn profile_of(&self, key: &ProfileKey) -> Option<InterfaceProfile> {
        match key {
            ProfileKey::Factory(f) => Some(f.profile()),
            ProfileKey::User(name) => self
                .user
                .iter()
                .find(|u| u.name.eq_ignore_ascii_case(name))
                .map(|u| u.profile.clone()),
            ProfileKey::Custom => None,
        }
    }

    /// Salva (ou substitui, sem distinguir maiúsculas) um perfil do usuário e o torna ativo.
    pub fn save_user(&mut self, name: &str, profile: InterfaceProfile) -> Result<(), NameError> {
        let name = name.trim();
        if name.is_empty() {
            return Err(NameError::Empty);
        }
        if name.chars().count() > MAX_NAME_LEN {
            return Err(NameError::TooLong);
        }
        match self
            .user
            .iter_mut()
            .find(|u| u.name.to_lowercase() == name.to_lowercase())
        {
            Some(existing) => {
                existing.name = name.to_owned();
                existing.profile = profile;
            }
            None => self.user.push(UserProfile {
                name: name.to_owned(),
                profile,
            }),
        }
        self.active = ProfileKey::User(name.to_owned());
        Ok(())
    }

    pub fn delete_user(&mut self, name: &str) {
        let before = self.user.len();
        self.user
            .retain(|u| u.name.to_lowercase() != name.trim().to_lowercase());
        if self.user.len() != before
            && matches!(&self.active, ProfileKey::User(a) if a.to_lowercase() == name.trim().to_lowercase())
        {
            self.active = ProfileKey::Custom;
        }
    }

    pub fn to_json(&self) -> serde_json::Value {
        serde_json::to_value(self).unwrap_or(serde_json::Value::Null)
    }

    /// Lê o gravado; ilegível volta ao padrão (nunca falha).
    pub fn from_json(value: Option<serde_json::Value>) -> Self {
        value
            .and_then(|v| serde_json::from_value(v).ok())
            .unwrap_or_default()
    }
}

/// Pedido da interface ao sistema que aplica perfis (a UI nunca mexe nos recursos direto).
#[derive(Clone, Debug, PartialEq)]
pub enum ProfileRequest {
    /// Aplica (ou reaplica, "restaurar") o perfil.
    Apply(ProfileKey),
    /// Salva o estado atual com este nome.
    SaveCurrent(String),
    Delete(String),
}

/// Perfis em uso, o pedido pendente e o rascunho do nome do formulário.
#[derive(Resource, Debug, Default)]
pub struct InterfaceProfiles {
    pub state: ProfilesState,
    pub request: Option<ProfileRequest>,
    pub new_name: String,
    pub name_error: Option<NameError>,
}

impl InterfaceProfiles {
    pub fn with_state(state: ProfilesState) -> Self {
        Self {
            state,
            ..Self::default()
        }
    }

    /// Nome para mostrar do perfil ativo (rótulo i18n ou nome do usuário).
    pub fn active_label(&self, i18n: &crate::i18n::I18n) -> String {
        match &self.state.active {
            ProfileKey::Factory(f) => i18n.t(f.label_key()),
            ProfileKey::User(name) => name.clone(),
            ProfileKey::Custom => i18n.t("iprofile-custom"),
        }
    }
}

fn view_from_name(name: &str) -> Option<CameraView> {
    CameraView::ALL
        .into_iter()
        .find(|v| format!("{v:?}") == name)
}

/// Aplica um pedido. Não recebe a Partida: só estado de interface e câmera (UI-11).
pub fn handle_request(
    profiles: &mut ProfilesState,
    state: &mut InterfaceState,
    rig: &mut CameraRig,
    memory: &mut CameraMemory,
    request: ProfileRequest,
) -> Result<(), NameError> {
    match request {
        ProfileRequest::Apply(key) => {
            match profiles.profile_of(&key) {
                Some(profile) => {
                    profile.apply(state);
                    // Só a Vista da Apresentação do perfil muda; a memória das outras fica.
                    if let Some(view) = profile.camera_view.as_deref().and_then(view_from_name)
                        && state.presentation.is_3d()
                    {
                        memory.set(state.presentation, view.pose(), Some(view));
                        rig.pose = view.pose();
                        rig.view = Some(view);
                    }
                    profiles.active = key;
                }
                None => profiles.active = ProfileKey::Custom,
            }
            Ok(())
        }
        ProfileRequest::SaveCurrent(name) => {
            let view = rig.view.map(|v| format!("{v:?}"));
            let profile = InterfaceProfile::capture(state, view.as_deref());
            profiles.save_user(&name, profile)
        }
        ProfileRequest::Delete(name) => {
            profiles.delete_user(&name);
            Ok(())
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn apply_requests(
    mut profiles: ResMut<InterfaceProfiles>,
    mut presentation: ResMut<Presentation>,
    mut dock: ResMut<Dock>,
    mut aids: ResMut<AidSettings>,
    mut view_state: ResMut<ViewState>,
    mut rig: ResMut<CameraRig>,
    mut memory: ResMut<CameraMemory>,
) {
    let Some(request) = profiles.request.take() else {
        return;
    };
    let mut state = InterfaceState {
        presentation: *presentation,
        dock: dock.layout.clone(),
        aids: *aids,
        focus: view_state.focus,
    };
    let was = state.clone();
    let mut next_rig = CameraRig {
        pose: rig.pose,
        view: rig.view,
    };
    let profiles = &mut *profiles;
    let result = handle_request(
        &mut profiles.state,
        &mut state,
        &mut next_rig,
        &mut memory,
        request,
    );
    profiles.name_error = result.err();
    if result.is_ok() {
        profiles.new_name.clear();
    }
    // Só escreve o que mudou: `is_changed` aciona câmeras, janela e gravação.
    if state.presentation != was.presentation {
        *presentation = state.presentation;
    }
    if state.dock != was.dock {
        dock.layout = state.dock;
    }
    if state.aids != was.aids {
        *aids = state.aids;
    }
    if state.focus != was.focus {
        view_state.focus = state.focus;
    }
    if next_rig.pose != rig.pose || next_rig.view != rig.view {
        rig.pose = next_rig.pose;
        rig.view = next_rig.view;
    }
}

/// O estado saiu do perfil ativo (mexeu num painel, num Auxílio…)? Vira "Personalizado".
fn track_active(
    mut profiles: ResMut<InterfaceProfiles>,
    presentation: Res<Presentation>,
    dock: Res<Dock>,
    aids: Res<AidSettings>,
    view_state: Res<ViewState>,
) {
    if profiles.state.active == ProfileKey::Custom || profiles.request.is_some() {
        return;
    }
    let state = InterfaceState {
        presentation: *presentation,
        dock: dock.layout.clone(),
        aids: *aids,
        focus: view_state.focus,
    };
    let still = profiles
        .state
        .profile_of(&profiles.state.active)
        .is_some_and(|p| p.matches(&state));
    if !still {
        profiles.state.active = ProfileKey::Custom;
    }
}

/// Seção "Perfil de interface" da janela principal.
pub fn profiles_section(
    ui: &mut egui::Ui,
    i18n: &crate::i18n::I18n,
    profiles: &mut InterfaceProfiles,
) {
    ui.strong(i18n.t("iprofile-heading"));
    let mut chosen: Option<ProfileKey> = None;
    egui::ComboBox::from_id_salt("interface-profile")
        .selected_text(profiles.active_label(i18n))
        .show_ui(ui, |ui| {
            for f in FactoryProfile::ALL {
                let key = ProfileKey::Factory(f);
                if ui
                    .selectable_label(profiles.state.active == key, i18n.t(f.label_key()))
                    .clicked()
                {
                    chosen = Some(key);
                }
            }
            for u in &profiles.state.user {
                let key = ProfileKey::User(u.name.clone());
                if ui
                    .selectable_label(profiles.state.active == key, &u.name)
                    .clicked()
                {
                    chosen = Some(key);
                }
            }
            if ui
                .selectable_label(
                    profiles.state.active == ProfileKey::Custom,
                    i18n.t("iprofile-custom"),
                )
                .clicked()
            {
                chosen = Some(ProfileKey::Custom);
            }
        });
    let active = profiles.state.active.clone();
    ui.horizontal_wrapped(|ui| {
        if ui
            .add_enabled(
                active != ProfileKey::Custom,
                egui::Button::new(i18n.t("iprofile-restore")),
            )
            .on_hover_text(i18n.t("iprofile-restore-hint"))
            .clicked()
        {
            chosen = Some(active.clone());
        }
        if let ProfileKey::User(name) = &active
            && ui.button(i18n.t("iprofile-delete")).clicked()
        {
            profiles.request = Some(ProfileRequest::Delete(name.clone()));
        }
    });
    ui.horizontal(|ui| {
        ui.add(
            egui::TextEdit::singleline(&mut profiles.new_name)
                .hint_text(i18n.t("iprofile-name-hint"))
                .desired_width(120.0),
        );
        if ui.button(i18n.t("iprofile-save")).clicked() {
            profiles.request = Some(ProfileRequest::SaveCurrent(profiles.new_name.clone()));
        }
    });
    match profiles.name_error {
        Some(NameError::Empty) => {
            ui.small(i18n.t("iprofile-error-empty"));
        }
        Some(NameError::TooLong) => {
            ui.small(i18n.t("iprofile-error-long"));
        }
        None => {}
    }
    if let Some(key) = chosen {
        profiles.request = Some(ProfileRequest::Apply(key));
    }
}

pub struct ProfilesPlugin;

impl Plugin for ProfilesPlugin {
    fn build(&self, app: &mut App) {
        // `init_resource` preserva os perfis carregados do banco pelo `PersistencePlugin`.
        app.init_resource::<InterfaceProfiles>().add_systems(
            Update,
            (apply_requests, track_active).chain().before(CameraSet),
        );
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;

    fn state() -> InterfaceState {
        InterfaceState {
            presentation: Presentation::Professional3d,
            dock: DockLayout::default(),
            aids: AidSettings::training(),
            focus: false,
        }
    }

    fn visible_ids(dock: &DockLayout) -> Vec<String> {
        let mut v: Vec<String> = dock
            .pane_ids()
            .into_iter()
            .filter(|id| dock.is_visible(id))
            .collect();
        v.sort();
        v
    }

    #[test]
    fn physical_game_is_the_immersive_table_with_no_dock_and_no_aids() {
        let mut s = state();
        FactoryProfile::PhysicalGame.profile().apply(&mut s);
        assert_eq!(s.presentation, Presentation::ImmersiveTable);
        assert!(!s.dock.any_visible());
        assert_eq!(s.aids, AidSettings::physical());
        assert!(!s.focus);
    }

    #[test]
    fn assisted_training_shows_notation_aids_and_training_in_the_professional_3d() {
        let mut s = state();
        s.presentation = Presentation::Technical2d;
        FactoryProfile::AssistedTraining.profile().apply(&mut s);
        assert_eq!(s.presentation, Presentation::Professional3d);
        assert_eq!(
            visible_ids(&s.dock),
            vec![ids::AIDS, ids::NOTATION, ids::TRAINING]
        );
        assert_eq!(s.aids, AidSettings::training());
    }

    #[test]
    fn study_and_analysis_shows_notation_and_annotations_in_the_technical_2d() {
        let mut s = state();
        FactoryProfile::StudyAnalysis.profile().apply(&mut s);
        assert_eq!(s.presentation, Presentation::Technical2d);
        assert_eq!(visible_ids(&s.dock), vec![ids::NOTATION, ids::STUDY]);
    }

    #[test]
    fn panels_not_registered_yet_are_ignored_and_show_up_once_registered() {
        let mut s = state();
        let before = s.dock.pane_ids().len();
        FactoryProfile::StudyAnalysis.profile().apply(&mut s);
        assert_eq!(s.dock.pane_ids().len(), before, "nenhum painel inventado");

        // C7/C8 registram os painéis: a árvore ganha as abas e o perfil as mostra.
        let mut registry = crate::dock::PanelRegistry::default();
        for id in s.dock.pane_ids() {
            registry.register(stub(&id));
        }
        registry.register(stub(PANEL_ANALYSIS));
        registry.register(stub(PANEL_LIBRARY));
        assert!(s.dock.reconcile(&registry));
        FactoryProfile::StudyAnalysis.profile().apply(&mut s);
        assert_eq!(
            visible_ids(&s.dock),
            vec![PANEL_ANALYSIS, PANEL_LIBRARY, ids::NOTATION, ids::STUDY]
        );
    }

    fn stub(id: &str) -> crate::dock::PanelDef {
        fn noop(_: &mut bevy_egui::egui::Ui, _: &crate::i18n::I18n, _: &mut bevy::prelude::World) {}
        crate::dock::PanelDef {
            id: Box::leak(id.to_owned().into_boxed_str()),
            label_key: "dock-panel-notation",
            access: crate::dock::PanelAccess::Always,
            ui: noop,
        }
    }

    #[test]
    fn applying_is_idempotent_and_matches_reports_it() {
        for factory in FactoryProfile::ALL {
            let profile = factory.profile();
            let mut s = state();
            assert!(!profile.matches(&s) || factory == FactoryProfile::AssistedTraining);
            profile.apply(&mut s);
            assert!(profile.matches(&s), "{factory:?}");
            let once = s.clone();
            profile.apply(&mut s);
            assert_eq!(s, once);
        }
    }

    #[test]
    fn any_manual_change_makes_the_state_custom() {
        let profile = FactoryProfile::AssistedTraining.profile();
        let mut s = state();
        profile.apply(&mut s);
        s.dock.set_visible(ids::CLOCK, true);
        assert!(!profile.matches(&s));
        profile.apply(&mut s);
        s.aids.ghosts = true;
        assert!(!profile.matches(&s));
    }

    #[test]
    fn restoring_a_factory_profile_undoes_manual_changes() {
        let profile = FactoryProfile::PhysicalGame.profile();
        let mut s = state();
        profile.apply(&mut s);
        s.dock.set_visible(ids::NOTATION, true);
        s.aids.destinations = true;
        s.presentation = Presentation::Technical2d;
        profile.apply(&mut s);
        assert!(profile.matches(&s));
    }

    #[test]
    fn a_saved_profile_brings_back_the_whole_layout() {
        let mut s = state();
        s.presentation = Presentation::Technical2d;
        s.dock.set_visible(ids::NOTATION, false);
        s.dock.width = 420.0;
        s.focus = true;
        let saved = InterfaceProfile::capture(&s, Some("Top"));
        let mut other = state();
        saved.apply(&mut other);
        assert_eq!(other, s);
        assert!(saved.matches(&other));
        assert_eq!(saved.camera_view.as_deref(), Some("Top"));
    }

    #[test]
    fn user_profiles_are_saved_replaced_and_deleted_by_name() {
        let mut profiles = ProfilesState::default();
        let a = InterfaceProfile::capture(&state(), None);
        profiles.save_user("  Meu estudo ", a.clone()).unwrap();
        assert_eq!(profiles.active, ProfileKey::User("Meu estudo".into()));
        let mut changed = state();
        changed.focus = true;
        profiles
            .save_user("meu ESTUDO", InterfaceProfile::capture(&changed, None))
            .unwrap();
        assert_eq!(profiles.user.len(), 1, "mesmo nome (sem caixa) substitui");
        assert!(profiles.user[0].profile.focus);
        assert_eq!(
            profiles.profile_of(&ProfileKey::User("Meu estudo".into())),
            Some(profiles.user[0].profile.clone())
        );
        assert_eq!(
            profiles.profile_of(&ProfileKey::Factory(FactoryProfile::PhysicalGame)),
            Some(FactoryProfile::PhysicalGame.profile())
        );
        assert_eq!(profiles.profile_of(&ProfileKey::Custom), None);
        profiles.delete_user("MEU estudo");
        assert!(profiles.user.is_empty());
        assert_eq!(profiles.active, ProfileKey::Custom);
    }

    #[test]
    fn invalid_names_are_refused() {
        let mut profiles = ProfilesState::default();
        let p = InterfaceProfile::capture(&state(), None);
        assert_eq!(profiles.save_user("   ", p.clone()), Err(NameError::Empty));
        let long = "x".repeat(MAX_NAME_LEN + 1);
        assert_eq!(profiles.save_user(&long, p), Err(NameError::TooLong));
        assert!(profiles.user.is_empty());
    }

    #[test]
    fn profiles_state_survives_a_json_round_trip() {
        let mut profiles = ProfilesState::default();
        let mut s = state();
        s.dock.width = 301.0;
        profiles
            .save_user("Aula", InterfaceProfile::capture(&s, Some("Oblique")))
            .unwrap();
        let back = ProfilesState::from_json(Some(profiles.to_json()));
        assert_eq!(back, profiles);
        let factory = ProfilesState {
            active: ProfileKey::Factory(FactoryProfile::StudyAnalysis),
            user: vec![],
        };
        assert_eq!(ProfilesState::from_json(Some(factory.to_json())), factory);
    }

    #[test]
    fn unreadable_profiles_fall_back_to_the_default() {
        assert_eq!(ProfilesState::from_json(None), ProfilesState::default());
        assert_eq!(
            ProfilesState::from_json(Some(serde_json::json!("lixo"))),
            ProfilesState::default()
        );
    }

    // --- Sistemas: a Partida não muda e as câmeras mantêm suas preferências (UI-11) ---

    use bevy::ecs::system::RunSystemOnce;
    use chess_game::{Command, Game, GameSetup, PolicyKind};

    fn world() -> World {
        let mut w = World::new();
        let mut session = crate::session::Session::from_game(Game::new(GameSetup::standard(
            PolicyKind::Assisted,
        )));
        session.command(Command::Release {
            from: "e2".parse().unwrap(),
            to: "e4".parse().unwrap(),
        });
        w.insert_resource(session);
        w.insert_resource(Presentation::Professional3d);
        w.insert_resource(Dock::default());
        w.insert_resource(AidSettings::training());
        w.insert_resource(ViewState::default());
        w.insert_resource(CameraRig::default());
        w.insert_resource(CameraMemory::default());
        w.insert_resource(InterfaceProfiles::default());
        w
    }

    fn request(w: &mut World, r: ProfileRequest) {
        w.resource_mut::<InterfaceProfiles>().request = Some(r);
        w.run_system_once(apply_requests).unwrap();
    }

    #[test]
    fn switching_profiles_never_changes_the_game() {
        let mut w = world();
        let events = w
            .resource::<crate::session::Session>()
            .game()
            .events()
            .len();
        let fen = w.resource::<crate::session::Session>().snapshot().clone();
        for f in FactoryProfile::ALL {
            request(&mut w, ProfileRequest::Apply(ProfileKey::Factory(f)));
            let session = w.resource::<crate::session::Session>();
            assert_eq!(session.game().events().len(), events, "{f:?}");
            assert_eq!(session.snapshot().history, fen.history, "{f:?}");
            assert_eq!(session.snapshot().side_to_move, fen.side_to_move);
        }
        assert_eq!(
            *w.resource::<Presentation>(),
            Presentation::Technical2d,
            "o último perfil (Estudo) foi aplicado"
        );
        assert_eq!(
            w.resource::<InterfaceProfiles>().state.active,
            ProfileKey::Factory(FactoryProfile::StudyAnalysis)
        );
    }

    #[test]
    fn camera_preferences_of_each_presentation_survive_profile_switches() {
        let mut w = world();
        // O usuário deixou a Mesa na Vista superior e o 3D profissional na oblíqua.
        {
            let mut memory = w.resource_mut::<CameraMemory>();
            memory.set(
                Presentation::ImmersiveTable,
                CameraView::Top.pose(),
                Some(CameraView::Top),
            );
            memory.set(
                Presentation::Professional3d,
                CameraView::Oblique.pose(),
                Some(CameraView::Oblique),
            );
        }
        request(
            &mut w,
            ProfileRequest::Apply(ProfileKey::Factory(FactoryProfile::StudyAnalysis)),
        );
        request(
            &mut w,
            ProfileRequest::Apply(ProfileKey::Factory(FactoryProfile::PhysicalGame)),
        );
        // Um perfil de fábrica não tem Vista própria: nada da memória foi sobrescrito.
        let mut rig = CameraRig::default();
        let mut memory = std::mem::take(&mut *w.resource_mut::<CameraMemory>());
        memory.switch(&mut rig, None, Presentation::ImmersiveTable);
        assert_eq!(rig.view, Some(CameraView::Top));
        memory.switch(
            &mut rig,
            Some(Presentation::ImmersiveTable),
            Presentation::Professional3d,
        );
        assert_eq!(rig.view, Some(CameraView::Oblique));
    }

    #[test]
    fn a_saved_profile_restores_the_camera_view_of_its_presentation() {
        let mut w = world();
        w.resource_mut::<CameraRig>().view = Some(CameraView::Inverted);
        request(&mut w, ProfileRequest::SaveCurrent("Preto embaixo".into()));
        w.resource_mut::<CameraRig>().view = Some(CameraView::Top);
        request(
            &mut w,
            ProfileRequest::Apply(ProfileKey::User("Preto embaixo".into())),
        );
        assert_eq!(w.resource::<CameraRig>().view, Some(CameraView::Inverted));
    }

    #[test]
    fn an_invalid_name_is_reported_and_nothing_is_saved() {
        let mut w = world();
        request(&mut w, ProfileRequest::SaveCurrent("  ".into()));
        let profiles = w.resource::<InterfaceProfiles>();
        assert_eq!(profiles.name_error, Some(NameError::Empty));
        assert!(profiles.state.user.is_empty());
    }

    #[test]
    fn leaving_the_profile_by_hand_turns_the_active_one_into_custom() {
        let mut w = world();
        request(
            &mut w,
            ProfileRequest::Apply(ProfileKey::Factory(FactoryProfile::AssistedTraining)),
        );
        w.run_system_once(track_active).unwrap();
        assert_eq!(
            w.resource::<InterfaceProfiles>().state.active,
            ProfileKey::Factory(FactoryProfile::AssistedTraining)
        );
        w.resource_mut::<AidSettings>().ghosts = true;
        w.run_system_once(track_active).unwrap();
        assert_eq!(
            w.resource::<InterfaceProfiles>().state.active,
            ProfileKey::Custom
        );
    }
}
