//! Perfis de interface (C4, UI-09/UI-11). Um Perfil de interface agrupa a Apresentação, o
//! layout do dock (painéis visíveis, abas, larguras), o Perfil de auxílios, o modo Foco e a
//! Vista da câmera. Trocar de perfil aplica tudo isso **sem tocar na Partida** (ADR-0001):
//! é só Apresentação e preferência, gravada no SQLite.
//!
//! Não confundir com o Perfil de auxílios (`aids::AidProfile`), que é só uma parte deste.

use crate::aids::AidSettings;
use crate::dock::DockLayout;
use crate::presentation::Presentation;
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
    Layout(DockLayout),
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
        todo!()
    }

    pub fn profile(self) -> InterfaceProfile {
        todo!()
    }
}

impl InterfaceProfile {
    /// Aplica o perfil ao estado. Idempotente.
    pub fn apply(&self, state: &mut InterfaceState) {
        let _ = state;
        todo!()
    }

    /// O estado coincide com o que este perfil aplicaria?
    pub fn matches(&self, state: &InterfaceState) -> bool {
        let _ = state;
        todo!()
    }

    /// "Salvar o estado atual como perfil": guarda layout inteiro e a Vista informada.
    pub fn capture(state: &InterfaceState, camera_view: Option<&str>) -> Self {
        let _ = (state, camera_view);
        todo!()
    }
}

impl ProfilesState {
    pub fn profile_of(&self, key: &ProfileKey) -> Option<InterfaceProfile> {
        let _ = key;
        todo!()
    }

    /// Salva (ou substitui, sem distinguir maiúsculas) um perfil do usuário e o torna ativo.
    pub fn save_user(&mut self, name: &str, profile: InterfaceProfile) -> Result<(), NameError> {
        let _ = (name, profile);
        todo!()
    }

    pub fn delete_user(&mut self, name: &str) {
        let _ = name;
        todo!()
    }

    pub fn to_json(&self) -> serde_json::Value {
        todo!()
    }

    /// Lê o gravado; ilegível volta ao padrão (nunca falha).
    pub fn from_json(value: Option<serde_json::Value>) -> Self {
        let _ = value;
        todo!()
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;
    use crate::dock::ids;

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
            vec![ids::NOTATION, PANEL_ANALYSIS, PANEL_LIBRARY, ids::STUDY]
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
}
