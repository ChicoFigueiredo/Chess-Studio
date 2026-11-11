//! Perfis de interface (C4, UI-09/UI-11). Um Perfil de interface agrupa a Apresentação, o
//! layout do dock (painéis visíveis, abas, larguras), o Perfil de auxílios, o modo Foco e a
//! Vista da câmera. Trocar de perfil aplica tudo isso **sem tocar na Partida** (ADR-0001):
//! é só Apresentação e preferência, gravada no SQLite.
//!
//! Não confundir com o Perfil de auxílios (`aids::AidProfile`), que é só uma parte deste.
//!
//! Desde o DS-07 (#166) o perfil guarda também o estado do Ribbon (modo, recolhido, aba ativa),
//! a Barra de Acesso Rápido e os Painéis flutuantes (dentro do layout do dock). Tema,
//! Densidade e Escala **não** entram: são do Perfil local (`ui_theme`). Perfis gravados antes
//! do DS-07 não têm os campos novos e carregam com os padrões (nada do Ribbon é tocado).

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

/// O Perfil de interface com que uma instalação nova ou um Perfil local novo abre: Jogo físico
/// ("tabuleiro primeiro": Ribbon simples, só Relógio e Notação).
pub const FIRST_RUN_PROFILE: FactoryProfile = FactoryProfile::PhysicalGame;

/// Perfis de fábrica (prompt mestre §9). "Personalizado" não é um deles: é o estado atual.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum FactoryProfile {
    PhysicalGame,
    AssistedTraining,
    StudyAnalysis,
    /// Súmula + Relógio, sem Auxílios, Notação oculta, Ribbon recolhido.
    Competition,
    /// Só o tabuleiro e o Relógio, sem dock, Ribbon recolhido.
    Minimal,
}

/// Disposição do Ribbon aberto (espelha `ribbon::RibbonMode` sem depender do módulo).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum RibbonLayout {
    #[default]
    Classic,
    Simple,
}

/// A parte do Ribbon que um Perfil de interface guarda.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct RibbonSetup {
    #[serde(default)]
    pub layout: RibbonLayout,
    #[serde(default)]
    pub collapsed: bool,
    /// Nome estável da aba (`RibbonView::id`); `None` mantém a aba atual.
    #[serde(default)]
    pub active_tab: Option<String>,
    /// Ids das Ações fixas na Barra de Acesso Rápido, na ordem.
    #[serde(default = "default_quick_access")]
    pub quick_access: Vec<String>,
}

fn default_quick_access() -> Vec<String> {
    use chess_input::controls::Action;
    [Action::Undo, Action::NewGame, Action::FlipBoard]
        .into_iter()
        .map(|a| a.id().to_owned())
        .collect()
}

impl Default for RibbonSetup {
    fn default() -> Self {
        Self {
            layout: RibbonLayout::Classic,
            collapsed: false,
            active_tab: None,
            quick_access: default_quick_access(),
        }
    }
}

impl RibbonSetup {
    fn factory(layout: RibbonLayout, collapsed: bool) -> Self {
        Self {
            layout,
            collapsed,
            ..Self::default()
        }
    }

    /// Mesmo modo, recolhimento e Barra de Acesso Rápido? A aba ativa não conta: trocar de
    /// aba não tira do perfil (como a aba ativa de um grupo do dock).
    fn same_as(&self, other: &Self) -> bool {
        self.layout == other.layout
            && self.collapsed == other.collapsed
            && self.quick_access == other.quick_access
    }
}

/// Janela do sistema com um Painel ou grupo de abas destacado (#168): abas, posição, tamanho
/// e monitor. O perfil as guarda aqui, fora do layout do dock.
pub use crate::detached::DetachedWindow;

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
    /// `None` quando o app roda sem o Ribbon (testes mínimos): o perfil não o toca.
    pub ribbon: Option<RibbonSetup>,
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
    /// Ribbon e Barra de Acesso Rápido; `None` (perfis antigos) não mexe no Ribbon.
    #[serde(default)]
    pub ribbon: Option<RibbonSetup>,
    /// Painéis e grupos de abas destacados em janelas do sistema (#168).
    #[serde(default)]
    pub detached: Vec<DetachedWindow>,
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
    pub const ALL: [FactoryProfile; 5] = [
        FactoryProfile::PhysicalGame,
        FactoryProfile::AssistedTraining,
        FactoryProfile::StudyAnalysis,
        FactoryProfile::Competition,
        FactoryProfile::Minimal,
    ];

    pub fn label_key(self) -> &'static str {
        match self {
            FactoryProfile::PhysicalGame => "iprofile-physical",
            FactoryProfile::AssistedTraining => "iprofile-training",
            FactoryProfile::StudyAnalysis => "iprofile-study",
            FactoryProfile::Competition => "iprofile-competition",
            FactoryProfile::Minimal => "iprofile-minimal",
        }
    }

    /// O perfil de fábrica, conforme a tabela do DS-07. No Mínimo a Apresentação é a Mesa
    /// imersiva: o relógio é o da mesa, então o dock fica oculto sem esconder o Relógio.
    pub fn profile(self) -> InterfaceProfile {
        let panels = |ids: &[&str]| DockSetup::Panels {
            visible: ids.iter().map(|id| (*id).to_owned()).collect(),
        };
        let (presentation, dock, aids, ribbon) = match self {
            FactoryProfile::PhysicalGame => (
                Presentation::Professional3d,
                panels(&[ids::CLOCK, ids::NOTATION]),
                AidSettings::physical(),
                RibbonSetup::factory(RibbonLayout::Simple, false),
            ),
            FactoryProfile::AssistedTraining => (
                Presentation::Professional3d,
                panels(&[ids::TRAINING, ids::NOTATION, ids::BOARD2D, ids::AIDS]),
                AidSettings::training(),
                RibbonSetup::factory(RibbonLayout::Simple, false),
            ),
            FactoryProfile::StudyAnalysis => (
                Presentation::Technical2d,
                panels(&[
                    PANEL_ANALYSIS,
                    ids::EXPLORER,
                    ids::OPENINGS,
                    ids::NOTATION,
                    PANEL_LIBRARY,
                ]),
                AidSettings::training(),
                RibbonSetup::factory(RibbonLayout::Classic, false),
            ),
            FactoryProfile::Competition => (
                Presentation::Professional3d,
                panels(&[ids::SCORESHEET, ids::CLOCK]),
                AidSettings::physical(),
                RibbonSetup::factory(RibbonLayout::Simple, true),
            ),
            FactoryProfile::Minimal => (
                Presentation::ImmersiveTable,
                panels(&[]),
                AidSettings::physical(),
                RibbonSetup::factory(RibbonLayout::Simple, true),
            ),
        };
        InterfaceProfile {
            presentation,
            dock,
            aids,
            focus: false,
            camera_view: None,
            ribbon: Some(ribbon),
            detached: Vec::new(),
        }
    }
}

impl InterfaceProfile {
    /// Aplica o perfil ao estado. Idempotente.
    pub fn apply(&self, state: &mut InterfaceState) {
        state.presentation = self.presentation;
        state.aids = self.aids;
        state.focus = self.focus;
        if let (Some(wanted), Some(current)) = (&self.ribbon, &state.ribbon) {
            let mut next = wanted.clone();
            if next.active_tab.is_none() {
                next.active_tab = current.active_tab.clone();
            }
            state.ribbon = Some(next);
        }
        match &self.dock {
            DockSetup::Layout(layout) => {
                state.dock = (**layout).clone();
                state.dock.windows = self.detached.clone();
                // Um Painel não vive em dois lugares: o que o perfil destaca sai do dock.
                state.dock.sanitize_windows();
            }
            DockSetup::Panels { visible } => {
                if !self.detached.is_empty() {
                    state.dock.windows = self.detached.clone();
                    state.dock.sanitize_windows();
                }
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
            && match (&applied.ribbon, &state.ribbon) {
                (Some(a), Some(b)) => a.same_as(b),
                _ => true,
            }
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
        // As janelas destacadas vão no campo próprio do perfil, não dentro do layout.
        let mut dock = state.dock.clone();
        let detached = std::mem::take(&mut dock.windows);
        Self {
            presentation: state.presentation,
            dock: DockSetup::Layout(Box::new(dock)),
            aids: state.aids,
            focus: state.focus,
            camera_view: camera_view.map(str::to_owned),
            ribbon: state.ribbon.clone(),
            detached,
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
    mut hidden: ResMut<crate::actions::HiddenPanels>,
    mut aids: ResMut<AidSettings>,
    mut view_state: ResMut<ViewState>,
    mut rig: ResMut<CameraRig>,
    mut memory: ResMut<CameraMemory>,
    mut ribbon: Option<ResMut<crate::ribbon::RibbonState>>,
) {
    let Some(request) = profiles.request.take() else {
        return;
    };
    // Aplicar (ou restaurar) um Perfil devolve os Painéis dele: acaba o "tudo oculto".
    let hidden_before = hidden.clone();
    if matches!(request, ProfileRequest::Apply(_)) {
        hidden.clear();
    }
    let saving = matches!(request, ProfileRequest::SaveCurrent(_));
    // Salvar com "tudo oculto" guarda os Painéis que estavam abertos, não o layout vazio.
    let layout = if saving {
        hidden_before.restored(&dock.layout)
    } else {
        dock.layout.clone()
    };
    let mut state = InterfaceState {
        presentation: *presentation,
        dock: layout,
        aids: *aids,
        focus: view_state.focus,
        ribbon: ribbon.as_ref().map(|r| r.setup()),
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
    if state.dock != was.dock && !saving {
        dock.set_layout(state.dock);
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
    if state.ribbon != was.ribbon
        && let (Some(setup), Some(ribbon)) = (&state.ribbon, ribbon.as_mut())
    {
        ribbon.apply_setup(setup);
    }
}

/// O estado saiu do perfil ativo (mexeu num painel, num Auxílio…)? Vira "Personalizado".
fn track_active(
    mut profiles: ResMut<InterfaceProfiles>,
    presentation: Res<Presentation>,
    dock: Res<Dock>,
    hidden: Res<crate::actions::HiddenPanels>,
    aids: Res<AidSettings>,
    view_state: Res<ViewState>,
    ribbon: Option<Res<crate::ribbon::RibbonState>>,
) {
    // "Ocultar todos os Painéis" é um estado temporário por cima do Perfil, não um desvio dele.
    if profiles.state.active == ProfileKey::Custom
        || profiles.request.is_some()
        || hidden.is_hidden()
    {
        return;
    }
    let state = InterfaceState {
        presentation: *presentation,
        dock: dock.layout.clone(),
        aids: *aids,
        focus: view_state.focus,
        ribbon: ribbon.as_ref().map(|r| r.setup()),
    };
    let still = profiles
        .state
        .profile_of(&profiles.state.active)
        .is_some_and(|p| p.matches(&state));
    if !still {
        profiles.state.active = ProfileKey::Custom;
    }
}

/// Seletor de Perfil de interface (Opções e bastidores). Sem título próprio: quem o hospeda
/// já o rotula (linha das Opções, título da página dos bastidores).
pub fn profiles_section(
    ui: &mut egui::Ui,
    i18n: &crate::i18n::I18n,
    profiles: &mut InterfaceProfiles,
) {
    let mut chosen: Option<ProfileKey> = None;
    // Cartões de escolha (Backstage): o perfil ativo em `accent-subtle` com contorno `accent`.
    ui.horizontal_wrapped(|ui| {
        let mut card = |ui: &mut egui::Ui, key: ProfileKey, label: &str| {
            if crate::ui_theme::cs_choice_card(ui, profiles.state.active == key, label, None)
                .clicked()
            {
                chosen = Some(key);
            }
        };
        for f in FactoryProfile::ALL {
            card(ui, ProfileKey::Factory(f), &i18n.t(f.label_key()));
        }
        for u in &profiles.state.user {
            card(ui, ProfileKey::User(u.name.clone()), &u.name);
        }
        card(ui, ProfileKey::Custom, &i18n.t("iprofile-custom"));
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
        crate::ui_theme::cs_text_field(
            ui,
            &mut profiles.new_name,
            &i18n.t("iprofile-name-hint"),
            None,
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
        app.init_resource::<InterfaceProfiles>()
            .init_resource::<crate::actions::HiddenPanels>()
            .add_systems(
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
            ribbon: Some(RibbonSetup::default()),
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
    fn physical_game_is_the_professional_3d_with_clock_and_notation_and_no_aids() {
        let mut s = state();
        s.presentation = Presentation::Technical2d;
        FactoryProfile::PhysicalGame.profile().apply(&mut s);
        assert_eq!(s.presentation, Presentation::Professional3d);
        assert_eq!(visible_ids(&s.dock), vec![ids::CLOCK, ids::NOTATION]);
        assert_eq!(s.aids, AidSettings::physical());
        assert!(!s.focus);
        assert_eq!(s.ribbon.unwrap().layout, RibbonLayout::Simple);
    }

    #[test]
    fn assisted_training_shows_training_notation_and_the_2d_board_with_aids_on() {
        let mut s = state();
        s.presentation = Presentation::Technical2d;
        FactoryProfile::AssistedTraining.profile().apply(&mut s);
        assert_eq!(s.presentation, Presentation::Professional3d);
        let visible = visible_ids(&s.dock);
        for id in [ids::TRAINING, ids::NOTATION, ids::BOARD2D] {
            assert!(visible.iter().any(|v| v == id), "{id}");
        }
        assert_eq!(s.aids, AidSettings::training());
        let ribbon = s.ribbon.unwrap();
        assert_eq!(
            (ribbon.layout, ribbon.collapsed),
            (RibbonLayout::Simple, false)
        );
    }

    #[test]
    fn study_and_analysis_shows_analysis_openings_notation_in_a_classic_ribbon() {
        let mut s = state();
        FactoryProfile::StudyAnalysis.profile().apply(&mut s);
        assert_eq!(s.presentation, Presentation::Technical2d);
        // Aberturas e Biblioteca só entram quando registradas (ver o teste seguinte).
        let visible = visible_ids(&s.dock);
        assert!(visible.iter().any(|v| v == PANEL_ANALYSIS));
        assert!(visible.iter().any(|v| v == ids::NOTATION));
        assert!(!visible.iter().any(|v| v == ids::CLOCK));
        let ribbon = s.ribbon.unwrap();
        assert_eq!(
            (ribbon.layout, ribbon.collapsed),
            (RibbonLayout::Classic, false)
        );
    }

    #[test]
    fn competition_shows_scoresheet_and_clock_without_notation_or_aids_and_collapses_the_ribbon() {
        let mut s = state();
        let mut registry = crate::dock::PanelRegistry::default();
        for id in s.dock.pane_ids() {
            registry.register(stub(&id));
        }
        registry.register(stub(ids::SCORESHEET));
        s.dock.reconcile(&registry);
        FactoryProfile::Competition.profile().apply(&mut s);
        assert_eq!(visible_ids(&s.dock), vec![ids::CLOCK, ids::SCORESHEET]);
        assert!(!s.dock.is_visible(ids::NOTATION));
        assert!(!s.dock.is_visible(ids::AIDS));
        assert_eq!(s.aids, AidSettings::physical());
        assert!(s.ribbon.unwrap().collapsed);
    }

    #[test]
    fn minimal_has_no_dock_visible_and_a_collapsed_ribbon() {
        let mut s = state();
        FactoryProfile::Minimal.profile().apply(&mut s);
        assert!(!s.dock.any_visible());
        assert_eq!(s.aids, AidSettings::physical());
        assert!(s.ribbon.unwrap().collapsed);
    }

    #[test]
    fn there_are_five_factory_profiles_with_labels() {
        assert_eq!(FactoryProfile::ALL.len(), 5);
        let keys: std::collections::HashSet<_> =
            FactoryProfile::ALL.iter().map(|f| f.label_key()).collect();
        assert_eq!(keys.len(), 5);
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
        registry.register(stub(ids::OPENINGS));
        registry.register(stub(ids::EXPLORER));
        s.dock.reconcile(&registry);
        FactoryProfile::StudyAnalysis.profile().apply(&mut s);
        let mut expected = vec![
            PANEL_ANALYSIS,
            PANEL_LIBRARY,
            ids::NOTATION,
            ids::OPENINGS,
            ids::EXPLORER,
        ];
        expected.sort();
        assert_eq!(visible_ids(&s.dock), expected);
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
        let profile = FactoryProfile::Minimal.profile();
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
        w.insert_resource(crate::actions::HiddenPanels::default());
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
            Presentation::ImmersiveTable,
            "o último perfil (Mínimo) foi aplicado"
        );
        assert_eq!(
            w.resource::<InterfaceProfiles>().state.active,
            ProfileKey::Factory(FactoryProfile::Minimal)
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

    // --- DS-07 (#166): Ribbon, Barra de Acesso Rápido, migração e Personalizado ---

    fn ribbon_setup() -> RibbonSetup {
        RibbonSetup {
            layout: RibbonLayout::Simple,
            collapsed: true,
            active_tab: Some("analysis".into()),
            quick_access: vec!["undo".into(), "flip-board".into()],
        }
    }

    #[test]
    fn a_saved_profile_round_trips_ribbon_quick_access_and_floating_panels() {
        let mut s = state();
        s.ribbon = Some(ribbon_setup());
        s.dock.detach(ids::STUDY, [111.0, 77.0], [333.0, 222.0]);
        let mut profiles = ProfilesState::default();
        profiles
            .save_user("Aula", InterfaceProfile::capture(&s, None))
            .unwrap();
        let back = ProfilesState::from_json(Some(profiles.to_json()));
        assert_eq!(back, profiles);
        let saved = &back.user[0].profile;
        assert_eq!(saved.ribbon, Some(ribbon_setup()));
        assert!(
            saved.detached.is_empty(),
            "sem janela destacada neste estado"
        );
        let mut other = state();
        saved.apply(&mut other);
        assert_eq!(other.ribbon, Some(ribbon_setup()));
        assert!(other.dock.is_floating(ids::STUDY));
        assert_eq!(other.dock.floating[0].pos, [111.0, 77.0]);
    }

    #[test]
    fn a_saved_profile_keeps_detached_windows_in_its_own_field() {
        let mut s = state();
        let id = s
            .dock
            .detach_group_to_window(ids::NOTATION, [500.0, 400.0])
            .unwrap();
        s.dock
            .set_window_rect(id, Some([2000, 80]), [520.0, 410.0], Some(1));
        let capture = InterfaceProfile::capture(&s, None);
        assert_eq!(capture.detached.len(), 1);
        assert_eq!(capture.detached[0].pos, Some([2000, 80]));
        assert_eq!(capture.detached[0].monitor, Some(1));
        match &capture.dock {
            DockSetup::Layout(layout) => assert!(layout.windows.is_empty()),
            DockSetup::Panels { .. } => panic!("captura guarda o layout"),
        }
        let mut profiles = ProfilesState::default();
        profiles.save_user("Dois monitores", capture).unwrap();
        let back = ProfilesState::from_json(Some(profiles.to_json()));
        assert_eq!(back, profiles);
        let mut other = state();
        back.user[0].profile.apply(&mut other);
        assert_eq!(other.dock.windows, s.dock.windows);
        assert!(other.dock.tile_of(ids::NOTATION).is_none());
        assert!(back.user[0].profile.matches(&other));
    }

    #[test]
    fn profiles_saved_before_ds7_load_with_defaults_and_leave_the_ribbon_alone() {
        // Formato anterior: sem `ribbon` nem `detached` no perfil de usuário.
        let old = serde_json::json!({
            "active": { "User": "Antigo" },
            "user": [{
                "name": "Antigo",
                "profile": {
                    "presentation": "Technical2d",
                    "dock": { "Panels": { "visible": ["Notation"] } },
                    "aids": serde_json::to_value(AidSettings::physical()).unwrap(),
                    "focus": true,
                    "camera_view": null
                }
            }]
        });
        let loaded = ProfilesState::from_json(Some(old));
        assert_eq!(loaded.user.len(), 1, "o perfil antigo carrega");
        let profile = &loaded.user[0].profile;
        assert_eq!(profile.ribbon, None);
        assert!(profile.detached.is_empty());
        assert!(profile.focus);
        let mut s = state();
        s.ribbon = Some(ribbon_setup());
        profile.apply(&mut s);
        assert_eq!(s.ribbon, Some(ribbon_setup()), "Ribbon intocado");
        assert!(profile.matches(&s));
        // Um RibbonSetup parcial também ganha padrões.
        let partial: RibbonSetup = serde_json::from_value(serde_json::json!({})).unwrap();
        assert_eq!(partial, RibbonSetup::default());
        assert_eq!(partial.quick_access, vec!["undo", "new-game", "flip-board"]);
    }

    #[test]
    fn leaving_the_ribbon_setup_makes_the_state_custom_but_changing_tab_does_not() {
        let profile = FactoryProfile::Competition.profile();
        let mut s = state();
        profile.apply(&mut s);
        assert!(profile.matches(&s));
        s.ribbon.as_mut().unwrap().active_tab = Some("training".into());
        assert!(profile.matches(&s), "a aba ativa não conta");
        s.ribbon.as_mut().unwrap().collapsed = false;
        assert!(!profile.matches(&s));
        profile.apply(&mut s);
        s.ribbon.as_mut().unwrap().quick_access.pop();
        assert!(!profile.matches(&s), "a Barra de Acesso Rápido conta");
    }

    #[test]
    fn without_a_ribbon_the_ribbon_never_makes_the_state_custom() {
        let profile = FactoryProfile::Competition.profile();
        let mut s = state();
        s.ribbon = None;
        profile.apply(&mut s);
        assert_eq!(s.ribbon, None);
        assert!(profile.matches(&s));
    }

    #[test]
    fn appearance_is_not_part_of_the_profile() {
        // Tema, Densidade e Escala vivem em `ui_theme`; nenhum campo do perfil as guarda.
        let json = serde_json::to_string(&FactoryProfile::Minimal.profile()).unwrap();
        for word in ["theme", "density", "scale"] {
            assert!(!json.to_lowercase().contains(word), "{word}");
        }
    }

    #[test]
    fn applying_competition_and_minimal_in_the_world_sets_panels_and_ribbon() {
        use crate::ribbon::{RibbonMode, RibbonState};
        let mut w = world();
        w.insert_resource(RibbonState::default());
        request(
            &mut w,
            ProfileRequest::Apply(ProfileKey::Factory(FactoryProfile::Competition)),
        );
        {
            let ribbon = w.resource::<RibbonState>();
            assert!(ribbon.collapsed);
            assert_eq!(ribbon.mode, RibbonMode::Simple);
        }
        request(
            &mut w,
            ProfileRequest::Apply(ProfileKey::Factory(FactoryProfile::Minimal)),
        );
        assert!(!w.resource::<Dock>().layout.any_visible());
        assert!(w.resource::<RibbonState>().collapsed);
        request(
            &mut w,
            ProfileRequest::Apply(ProfileKey::Factory(FactoryProfile::StudyAnalysis)),
        );
        let ribbon = w.resource::<RibbonState>();
        assert!(!ribbon.collapsed);
        assert_eq!(ribbon.mode, RibbonMode::Classic);
        w.run_system_once(track_active).unwrap();
        assert_eq!(
            w.resource::<InterfaceProfiles>().state.active,
            ProfileKey::Factory(FactoryProfile::StudyAnalysis),
            "o Ribbon aplicado bate com o perfil"
        );
        // Mexer no Ribbon manualmente vira Personalizado.
        w.resource_mut::<RibbonState>().collapsed = true;
        w.run_system_once(track_active).unwrap();
        assert_eq!(
            w.resource::<InterfaceProfiles>().state.active,
            ProfileKey::Custom
        );
    }

    #[test]
    fn switching_among_the_five_profiles_never_touches_the_game() {
        let mut w = world();
        w.insert_resource(crate::ribbon::RibbonState::default());
        let before = w
            .resource::<crate::session::Session>()
            .game()
            .events()
            .len();
        for f in FactoryProfile::ALL {
            request(&mut w, ProfileRequest::Apply(ProfileKey::Factory(f)));
            let session = w.resource::<crate::session::Session>();
            assert_eq!(session.game().events().len(), before, "{f:?}");
        }
    }

    /// #183: "Ocultar todos os Painéis" não tira o app do Perfil ativo; abrir um Painel à
    /// mão sim, e trocar de Perfil encerra o "tudo oculto".
    #[test]
    fn hiding_all_panels_alone_does_not_turn_the_profile_into_custom() {
        let mut w = world();
        request(
            &mut w,
            ProfileRequest::Apply(ProfileKey::Factory(FactoryProfile::PhysicalGame)),
        );
        // Oculta tudo, como a Ação faz.
        let open: Vec<String> = {
            let layout = &w.resource::<Dock>().layout;
            layout
                .pane_ids()
                .into_iter()
                .filter(|id| layout.is_visible(id))
                .collect()
        };
        assert!(!open.is_empty());
        w.resource_scope(|_, mut dock: Mut<Dock>| {
            for id in dock.layout.pane_ids() {
                dock.layout.set_visible(&id, false);
            }
        });
        w.resource_mut::<crate::actions::HiddenPanels>()
            .hide_for_test(open.clone());
        w.run_system_once(track_active).unwrap();
        assert_eq!(
            w.resource::<InterfaceProfiles>().state.active,
            ProfileKey::Factory(FactoryProfile::PhysicalGame),
            "tudo oculto não é Personalizado"
        );
        // Restaura: continua no perfil.
        w.resource_mut::<crate::actions::HiddenPanels>().clear();
        w.resource_scope(|_, mut dock: Mut<Dock>| {
            for id in &open {
                dock.layout.set_visible(id, true);
            }
        });
        w.run_system_once(track_active).unwrap();
        assert_eq!(
            w.resource::<InterfaceProfiles>().state.active,
            ProfileKey::Factory(FactoryProfile::PhysicalGame)
        );
        // Abrir um Painel à mão é mudança de verdade.
        w.resource_mut::<Dock>()
            .layout
            .set_visible(ids::CAPTURED, true);
        w.run_system_once(track_active).unwrap();
        assert_eq!(
            w.resource::<InterfaceProfiles>().state.active,
            ProfileKey::Custom
        );
    }

    #[test]
    fn saving_the_current_state_while_all_is_hidden_saves_the_panels_that_were_open() {
        let mut w = world();
        request(
            &mut w,
            ProfileRequest::Apply(ProfileKey::Factory(FactoryProfile::PhysicalGame)),
        );
        let open = vec![ids::CLOCK.to_owned(), ids::NOTATION.to_owned()];
        w.resource_scope(|_, mut dock: Mut<Dock>| {
            for id in dock.layout.pane_ids() {
                dock.layout.set_visible(&id, false);
            }
        });
        w.resource_mut::<crate::actions::HiddenPanels>()
            .hide_for_test(open.clone());
        request(&mut w, ProfileRequest::SaveCurrent("Meu".into()));
        // O dock segue oculto e o "tudo oculto" também.
        assert!(!w.resource::<Dock>().layout.any_visible());
        assert!(w.resource::<crate::actions::HiddenPanels>().is_hidden());
        let saved = w.resource::<InterfaceProfiles>().state.user[0]
            .profile
            .clone();
        let DockSetup::Layout(layout) = saved.dock else {
            panic!("layout inteiro")
        };
        assert_eq!(visible_ids(&layout), vec![ids::CLOCK, ids::NOTATION]);
    }

    #[test]
    fn applying_a_profile_ends_the_hidden_state() {
        let mut w = world();
        w.resource_mut::<crate::actions::HiddenPanels>()
            .hide_for_test(vec![ids::CLOCK.to_owned()]);
        request(
            &mut w,
            ProfileRequest::Apply(ProfileKey::Factory(FactoryProfile::Minimal)),
        );
        assert!(!w.resource::<crate::actions::HiddenPanels>().is_hidden());
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
