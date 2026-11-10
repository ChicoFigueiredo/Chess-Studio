//! Conjuntos de Painéis (#186): grupos nomeados de Painéis que se ligam e desligam juntos pela
//! aba Exibir e pela Paleta ("Abrir Conjunto: <nome>"). Quatro vêm de fábrica (Partida,
//! Análise, Estudo, Treino); o usuário salva os seus a partir dos Painéis abertos, por Perfil
//! local.
//!
//! Não é Perfil de interface: aplicar um Conjunto só liga e desliga Painéis (e junta os que
//! abrem em abas no dock). Partida, Apresentação, Ribbon e Auxílios não são tocados; o Perfil
//! de interface pode virar "Personalizado", o que é esperado (ver `profiles::track_active`).
//!
//! * **Abrir** (clique): os Painéis do Conjunto entram como abas no dock, sem fechar os outros.
//! * **Mostrar só** (Shift+clique): só os Painéis do Conjunto ficam visíveis.
//!
//! Com "Ocultar todos os Painéis" ativo, o estado termina antes: os Painéis guardados voltam
//! e então o Conjunto é aplicado.

use crate::actions::HiddenPanels;
use crate::dock::{Dock, DockLayout, PanelRegistry, ids};
use crate::session::Session;
use bevy::prelude::*;
use serde::{Deserialize, Serialize};

/// Comprimento máximo do nome de um Conjunto salvo.
pub const MAX_NAME_LEN: usize = 40;

/// Id do Painel Engines (gerenciador de Engines; `dock::ids` ainda não o nomeia).
const ENGINES: &str = "Engines";

/// Um Conjunto de fábrica.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FactorySet {
    /// Id estável, em inglês.
    pub id: &'static str,
    /// Chave i18n do nome.
    pub label_key: &'static str,
    pub panels: &'static [&'static str],
}

/// Os Conjuntos de fábrica, na ordem em que aparecem.
pub const FACTORY: [FactorySet; 4] = [
    FactorySet {
        id: "match",
        label_key: "panel-set-match",
        panels: &[ids::CLOCK, ids::NOTATION, ids::CAPTURED],
    },
    FactorySet {
        id: "analysis",
        label_key: "panel-set-analysis",
        panels: &[ids::ANALYSIS, ENGINES, ids::OPENINGS],
    },
    FactorySet {
        id: "study",
        label_key: "panel-set-study",
        panels: &[ids::NOTATION, ids::STUDY, ids::BOARD2D, ids::LIBRARY],
    },
    FactorySet {
        id: "training",
        label_key: "panel-set-training",
        panels: &[ids::TRAINING, ids::AIDS, ids::NOTATION],
    },
];

/// Um Conjunto salvo pelo usuário.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SavedSet {
    pub name: String,
    pub panels: Vec<String>,
}

/// Qual Conjunto: de fábrica (pelo id) ou salvo (pelo nome).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SetRef {
    Factory(&'static str),
    Saved(String),
}

/// Por que um pedido de salvar ou renomear foi recusado.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SetError {
    EmptyName,
    TooLong,
    /// O nome já é de outro Conjunto (de fábrica ou salvo).
    NameTaken,
    /// Nenhum Painel aberto para salvar.
    NoPanels,
}

impl SetError {
    /// Chave i18n da mensagem.
    pub fn key(self) -> &'static str {
        match self {
            SetError::EmptyName => "panel-set-error-empty",
            SetError::TooLong => "panel-set-error-long",
            SetError::NameTaken => "panel-set-error-taken",
            SetError::NoPanels => "panel-set-error-no-panels",
        }
    }
}

/// Como aplicar um Conjunto.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ApplyMode {
    /// Abre os Painéis do Conjunto, sem fechar os outros.
    Open,
    /// Só os Painéis do Conjunto ficam visíveis.
    ShowOnly,
}

/// Pedido da interface (aba Exibir, Paleta) atendido por [`handle_requests`].
#[derive(Message, Debug, Clone, PartialEq, Eq)]
pub enum PanelSetRequested {
    Open(SetRef),
    ShowOnly(SetRef),
    /// Salva os Painéis abertos agora com este nome (substitui um salvo de mesmo nome).
    SaveCurrent(String),
    Rename {
        from: String,
        to: String,
    },
    Delete(String),
}

/// Os Conjuntos salvos do Perfil local e a última recusa.
#[derive(Resource, Debug, Default, Clone, PartialEq, Eq)]
pub struct PanelSets {
    saved: Vec<SavedSet>,
    /// Última recusa (mostrada no menu até o próximo pedido).
    pub error: Option<SetError>,
}

fn normalize(name: &str) -> String {
    name.trim().to_lowercase()
}

impl PanelSets {
    /// Reconstrói do gravado (descarta entradas sem nome ou sem Painéis).
    pub fn from_saved(saved: Vec<SavedSet>) -> Self {
        Self {
            saved: saved
                .into_iter()
                .filter(|s| !s.name.trim().is_empty() && !s.panels.is_empty())
                .collect(),
            error: None,
        }
    }

    pub fn saved(&self) -> &[SavedSet] {
        &self.saved
    }

    /// Os Painéis de um Conjunto, se ele existe.
    pub fn panels_of(&self, set: &SetRef) -> Option<Vec<String>> {
        match set {
            SetRef::Factory(id) => FACTORY
                .iter()
                .find(|f| f.id == *id)
                .map(|f| f.panels.iter().map(|p| (*p).to_owned()).collect()),
            SetRef::Saved(name) => self
                .saved
                .iter()
                .find(|s| s.name == *name)
                .map(|s| s.panels.clone()),
        }
    }

    fn check_name(&self, name: &str, except: Option<&str>) -> Result<String, SetError> {
        let name = name.trim();
        if name.is_empty() {
            return Err(SetError::EmptyName);
        }
        if name.chars().count() > MAX_NAME_LEN {
            return Err(SetError::TooLong);
        }
        Ok(name.to_owned()).and_then(|n| {
            let key = normalize(&n);
            let factory_clash = FACTORY.iter().any(|f| normalize(factory_name(f)) == key);
            let saved_clash = self
                .saved
                .iter()
                .any(|s| normalize(&s.name) == key && Some(s.name.as_str()) != except);
            if factory_clash || saved_clash {
                Err(SetError::NameTaken)
            } else {
                Ok(n)
            }
        })
    }

    /// Salva `panels` com `name`. Um nome já salvo é substituído; o de um de fábrica é recusado.
    pub fn save(&mut self, name: &str, panels: Vec<String>) -> Result<(), SetError> {
        let trimmed = name.trim();
        if panels.is_empty() {
            return Err(SetError::NoPanels);
        }
        let existing = self
            .saved
            .iter()
            .position(|s| normalize(&s.name) == normalize(trimmed));
        let name = match existing {
            Some(i) => {
                let current = self.saved[i].name.clone();
                self.check_name(trimmed, Some(&current))?
            }
            None => self.check_name(trimmed, None)?,
        };
        match existing {
            Some(i) => self.saved[i] = SavedSet { name, panels },
            None => self.saved.push(SavedSet { name, panels }),
        }
        Ok(())
    }

    pub fn rename(&mut self, from: &str, to: &str) -> Result<(), SetError> {
        let Some(i) = self.saved.iter().position(|s| s.name == from) else {
            return Ok(());
        };
        let name = self.check_name(to, Some(from))?;
        self.saved[i].name = name;
        Ok(())
    }

    pub fn delete(&mut self, name: &str) {
        self.saved.retain(|s| s.name != name);
    }
}

/// Nome de fábrica em pt-BR (para a checagem de colisão com os salvos, sem depender do
/// `I18n`): os mesmos textos de `panel-set-*` em `ribbon.ftl`.
fn factory_name(set: &FactorySet) -> &'static str {
    match set.id {
        "match" => "Partida",
        "analysis" => "Análise",
        "study" => "Estudo",
        _ => "Treino",
    }
}

/// Os Painéis abertos agora (visíveis, na ordem do layout); com "tudo oculto", os que
/// estavam abertos antes.
pub fn open_panels(layout: &DockLayout, hidden: &HiddenPanels) -> Vec<String> {
    match hidden.open_set() {
        Some(open) => layout
            .pane_ids()
            .into_iter()
            .filter(|id| open.contains(id))
            .collect(),
        None => layout
            .pane_ids()
            .into_iter()
            .filter(|id| layout.is_visible(id))
            .collect(),
    }
}

/// Aplica os `panels` ao layout. `allowed` filtra os Painéis que não podem abrir agora
/// (acesso bloqueado numa Partida competitiva). Só visibilidade e agrupamento em abas mudam;
/// Painéis em janela do sistema ficam como estão (como em "Ocultar todos").
pub fn apply(
    layout: &mut DockLayout,
    hidden: &mut HiddenPanels,
    panels: &[String],
    mode: ApplyMode,
    allowed: impl Fn(&str) -> bool,
) {
    // "Ocultar todos" termina antes: os Painéis guardados voltam.
    if hidden.is_hidden() {
        *layout = hidden.restored(layout);
        hidden.clear();
    }
    let present = layout.pane_ids();
    let wanted: Vec<&String> = panels
        .iter()
        .filter(|id| present.contains(id) && allowed(id))
        .collect();
    let was_visible: Vec<bool> = wanted.iter().map(|id| layout.is_visible(id)).collect();
    if mode == ApplyMode::ShowOnly {
        for id in &present {
            if layout.window_tile_of(id).is_none() && !wanted.contains(&id) {
                layout.set_visible(id, false);
            }
        }
    }
    // O grupo de abas âncora: o de um Painel do Conjunto já visível e encaixado; senão, o do
    // primeiro encaixado. Os Painéis que acabam de abrir entram nele como abas.
    let docked_ids: Vec<&String> = wanted
        .iter()
        .copied()
        .filter(|id| layout.tile_of(id).is_some() && layout.window_tile_of(id).is_none())
        .collect();
    let docked = |id: &str| docked_ids.iter().any(|d| d.as_str() == id);
    let anchor = wanted
        .iter()
        .zip(&was_visible)
        .find(|(id, vis)| **vis && docked(id))
        .map(|(id, _)| (*id).clone())
        .or_else(|| wanted.iter().find(|id| docked(id)).map(|id| (*id).clone()));
    for (id, was) in wanted.iter().zip(&was_visible) {
        layout.set_visible(id, true);
        if !*was
            && let Some(anchor) = &anchor
            && anchor != *id
            && docked(id)
        {
            layout.move_into_group_of(id, anchor);
        }
    }
    if let Some(first) = wanted.first() {
        layout.activate(first);
    }
}

pub struct PanelSetsPlugin;

impl Plugin for PanelSetsPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<PanelSets>()
            .add_message::<PanelSetRequested>()
            .add_systems(Update, handle_requests);
    }
}

/// Atende os pedidos da interface. Só layout e Conjuntos mudam: nem a `Session` (ADR-0001),
/// nem Apresentação, Ribbon ou Auxílios.
pub fn handle_requests(
    mut requests: MessageReader<PanelSetRequested>,
    mut sets: ResMut<PanelSets>,
    mut dock: ResMut<Dock>,
    mut hidden: ResMut<HiddenPanels>,
    session: Res<Session>,
    registry: Option<Res<PanelRegistry>>,
) {
    for request in requests.read() {
        let outcome = match request {
            PanelSetRequested::Open(set) | PanelSetRequested::ShowOnly(set) => {
                let mode = if matches!(request, PanelSetRequested::Open(_)) {
                    ApplyMode::Open
                } else {
                    ApplyMode::ShowOnly
                };
                if let Some(panels) = sets.panels_of(set) {
                    let allowed = |id: &str| {
                        registry
                            .as_ref()
                            .and_then(|r| r.get(id))
                            .is_none_or(|def| def.accessible(&session))
                    };
                    apply(&mut dock.layout, &mut hidden, &panels, mode, allowed);
                }
                Ok(())
            }
            PanelSetRequested::SaveCurrent(name) => {
                let open = open_panels(&dock.layout, &hidden);
                sets.save(name, open)
            }
            PanelSetRequested::Rename { from, to } => sets.rename(from, to),
            PanelSetRequested::Delete(name) => {
                sets.delete(name);
                Ok(())
            }
        };
        sets.error = outcome.err();
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;
    use crate::dock::{PanelAccess, PanelDef};
    use crate::i18n::I18n;

    fn noop(_: &mut bevy_egui::egui::Ui, _: &I18n, _: &mut World) {}

    fn def(id: &'static str, access: PanelAccess) -> PanelDef {
        PanelDef {
            id,
            label_key: "dock-panel-notation",
            access,
            ui: noop,
        }
    }

    const ALL: [&str; 15] = [
        ids::CLOCK,
        ids::CAPTURED,
        ids::BOARD2D,
        ids::NOTATION,
        ids::STUDY,
        ids::AIDS,
        ids::REVIEW,
        ids::ANALYSIS,
        ids::ARBITRATION,
        ids::TRAINING,
        ids::LIBRARY,
        ids::OPENINGS,
        ids::SCORESHEET,
        ENGINES,
        "Extra",
    ];

    fn registry() -> PanelRegistry {
        let mut r = PanelRegistry::default();
        for id in ALL {
            let access = if matches!(
                id,
                ids::STUDY | ids::ANALYSIS | ids::LIBRARY | ids::OPENINGS
            ) {
                PanelAccess::OutsideCompetitive
            } else {
                PanelAccess::Always
            };
            r.register(def(id, access));
        }
        r
    }

    /// Layout padrão com os Painéis que o padrão não traz (Biblioteca, Aberturas, Engines…)
    /// como abas extras, como o app faz ao registrá-los.
    fn full_layout() -> DockLayout {
        let mut layout = DockLayout::default();
        layout.reconcile(&registry());
        layout
    }

    fn strings(ids: &[&str]) -> Vec<String> {
        ids.iter().map(|s| (*s).to_owned()).collect()
    }

    fn visible(layout: &DockLayout) -> Vec<String> {
        let mut v: Vec<String> = layout
            .pane_ids()
            .into_iter()
            .filter(|id| layout.is_visible(id))
            .collect();
        v.sort();
        v
    }

    fn hide_all(layout: &mut DockLayout) {
        for id in layout.pane_ids() {
            layout.set_visible(&id, false);
        }
    }

    fn group_of(layout: &DockLayout, id: &str) -> Option<egui_tiles::TileId> {
        layout
            .tile_of(id)
            .and_then(|t| layout.tree.tiles.parent_of(t))
    }

    #[test]
    fn factory_sets_match_the_issue() {
        let by = |id: &str| FACTORY.iter().find(|f| f.id == id).unwrap().panels;
        assert_eq!(by("match"), [ids::CLOCK, ids::NOTATION, ids::CAPTURED]);
        assert_eq!(by("analysis"), [ids::ANALYSIS, ENGINES, ids::OPENINGS]);
        assert_eq!(
            by("study"),
            [ids::NOTATION, ids::STUDY, ids::BOARD2D, ids::LIBRARY]
        );
        assert_eq!(by("training"), [ids::TRAINING, ids::AIDS, ids::NOTATION]);
    }

    #[test]
    fn open_joins_the_panels_into_one_tab_group_and_keeps_the_others() {
        let mut layout = full_layout();
        hide_all(&mut layout);
        layout.set_visible(ids::REVIEW, true);
        let mut hidden = HiddenPanels::default();
        let panels = strings(&[ids::ANALYSIS, ENGINES, ids::OPENINGS]);
        apply(&mut layout, &mut hidden, &panels, ApplyMode::Open, |_| true);
        for id in &panels {
            assert!(layout.is_visible(id), "{id}");
        }
        assert!(layout.is_visible(ids::REVIEW), "os outros seguem abertos");
        assert!(!layout.is_visible(ids::CLOCK));
        let g = group_of(&layout, ids::ANALYSIS);
        assert!(g.is_some());
        assert_eq!(group_of(&layout, ENGINES), g);
        assert_eq!(group_of(&layout, ids::OPENINGS), g);
        assert!(layout.is_valid());
    }

    #[test]
    fn open_does_not_move_a_panel_that_was_already_open() {
        let mut layout = full_layout();
        // Relógio e Notação abertos em grupos diferentes do padrão.
        let clock_group = group_of(&layout, ids::CLOCK);
        let panels = strings(&[ids::CLOCK, ids::NOTATION, ids::CAPTURED]);
        let mut hidden = HiddenPanels::default();
        apply(&mut layout, &mut hidden, &panels, ApplyMode::Open, |_| true);
        assert_eq!(group_of(&layout, ids::CLOCK), clock_group);
        assert_ne!(
            group_of(&layout, ids::NOTATION),
            clock_group,
            "já estava aberto: fica onde está"
        );
    }

    #[test]
    fn show_only_hides_every_other_panel() {
        let mut layout = full_layout();
        let mut hidden = HiddenPanels::default();
        let panels = strings(&[ids::CLOCK, ids::NOTATION, ids::CAPTURED]);
        apply(
            &mut layout,
            &mut hidden,
            &panels,
            ApplyMode::ShowOnly,
            |_| true,
        );
        let mut want = panels.clone();
        want.sort();
        assert_eq!(visible(&layout), want);
    }

    #[test]
    fn panels_not_allowed_now_are_skipped() {
        let mut layout = full_layout();
        let mut hidden = HiddenPanels::default();
        let panels = strings(&[ids::ANALYSIS, ids::OPENINGS, ids::NOTATION]);
        apply(
            &mut layout,
            &mut hidden,
            &panels,
            ApplyMode::ShowOnly,
            |id| id == ids::NOTATION,
        );
        assert_eq!(visible(&layout), strings(&[ids::NOTATION]));
    }

    #[test]
    fn applying_while_hide_all_is_active_ends_that_state_first() {
        let mut layout = full_layout();
        let open = visible(&layout);
        // Estado "tudo oculto" como a Ação o deixa.
        let mut hidden = HiddenPanels::default();
        hidden.hide_for_test(open.clone());
        hide_all(&mut layout);
        let panels = strings(&[ids::TRAINING]);
        apply(&mut layout, &mut hidden, &panels, ApplyMode::Open, |_| true);
        assert!(!hidden.is_hidden());
        assert_eq!(
            visible(&layout),
            open,
            "os guardados voltaram e o Conjunto abriu"
        );
    }

    #[test]
    fn show_only_while_hide_all_is_active_shows_just_the_set() {
        let mut layout = full_layout();
        let mut hidden = HiddenPanels::default();
        hidden.hide_for_test(visible(&layout));
        hide_all(&mut layout);
        let panels = strings(&[ids::TRAINING, ids::AIDS]);
        apply(
            &mut layout,
            &mut hidden,
            &panels,
            ApplyMode::ShowOnly,
            |_| true,
        );
        assert!(!hidden.is_hidden());
        assert_eq!(visible(&layout), strings(&[ids::AIDS, ids::TRAINING]));
    }

    #[test]
    fn unknown_panels_in_a_set_are_ignored() {
        let mut layout = full_layout();
        let mut hidden = HiddenPanels::default();
        let panels = strings(&["Inexistente", ids::CLOCK]);
        apply(
            &mut layout,
            &mut hidden,
            &panels,
            ApplyMode::ShowOnly,
            |_| true,
        );
        assert_eq!(visible(&layout), strings(&[ids::CLOCK]));
    }

    #[test]
    fn saving_uses_the_open_panels_and_validates_the_name() {
        let mut sets = PanelSets::default();
        assert_eq!(sets.save("  ", strings(&["A"])), Err(SetError::EmptyName));
        assert_eq!(
            sets.save(&"x".repeat(41), strings(&["A"])),
            Err(SetError::TooLong)
        );
        assert_eq!(sets.save("Meu", vec![]), Err(SetError::NoPanels));
        assert_eq!(
            sets.save("análise", strings(&["A"])),
            Err(SetError::NameTaken),
            "nome de Conjunto de fábrica"
        );
        sets.save(" Meu ", strings(&["A", "B"])).unwrap();
        assert_eq!(sets.saved()[0].name, "Meu");
        // Mesmo nome: substitui.
        sets.save("meu", strings(&["C"])).unwrap();
        assert_eq!(sets.saved().len(), 1);
        assert_eq!(sets.saved()[0].panels, strings(&["C"]));
    }

    #[test]
    fn rename_and_delete_only_touch_saved_sets() {
        let mut sets = PanelSets::default();
        sets.save("Um", strings(&["A"])).unwrap();
        sets.save("Dois", strings(&["B"])).unwrap();
        assert_eq!(sets.rename("Um", "Dois"), Err(SetError::NameTaken));
        assert_eq!(sets.rename("Um", "Treino"), Err(SetError::NameTaken));
        sets.rename("Um", "Primeiro").unwrap();
        // Renomear para o mesmo nome (só caixa) é permitido.
        sets.rename("Primeiro", "primeiro").unwrap();
        assert_eq!(sets.saved()[0].name, "primeiro");
        sets.delete("Dois");
        assert_eq!(sets.saved().len(), 1);
        sets.delete("Treino");
        assert_eq!(sets.saved().len(), 1);
    }

    #[test]
    fn saved_sets_survive_a_json_round_trip_and_drop_junk() {
        let mut sets = PanelSets::default();
        sets.save("Meu", strings(&["A", "B"])).unwrap();
        let json = serde_json::to_value(sets.saved()).unwrap();
        let back: Vec<SavedSet> = serde_json::from_value(json).unwrap();
        assert_eq!(PanelSets::from_saved(back), sets);
        let junk = vec![
            SavedSet {
                name: " ".into(),
                panels: strings(&["A"]),
            },
            SavedSet {
                name: "Vazio".into(),
                panels: vec![],
            },
        ];
        assert!(PanelSets::from_saved(junk).saved().is_empty());
    }

    #[test]
    fn open_panels_read_the_stored_set_while_everything_is_hidden() {
        let mut layout = full_layout();
        let visible_now = visible(&layout);
        let mut hidden = HiddenPanels::default();
        hidden.hide_for_test(strings(&[ids::CLOCK, ids::TRAINING]));
        hide_all(&mut layout);
        let mut got = open_panels(&layout, &hidden);
        got.sort();
        assert_eq!(got, strings(&[ids::CLOCK, ids::TRAINING]));
        assert!(visible_now.len() > 2);
    }

    // --- Pelo App: o que aplicar um Conjunto nunca toca ---

    fn app() -> App {
        let mut app = crate::actions::test_app();
        app.insert_resource(registry())
            .init_resource::<PanelSets>()
            .add_message::<PanelSetRequested>()
            .add_systems(Update, handle_requests);
        let layout = full_layout();
        app.world_mut().resource_mut::<Dock>().set_layout(layout);
        app
    }

    fn send(app: &mut App, request: PanelSetRequested) {
        app.world_mut().write_message(request);
        app.update();
    }

    #[test]
    fn applying_a_set_never_touches_game_presentation_ribbon_or_aids() {
        let mut app = app();
        let before = (
            format!("{:?}", app.world().resource::<Session>().snapshot()),
            *app.world().resource::<crate::presentation::Presentation>(),
            *app.world().resource::<crate::aids::AidSettings>(),
        );
        for set in FACTORY {
            send(&mut app, PanelSetRequested::Open(SetRef::Factory(set.id)));
            send(
                &mut app,
                PanelSetRequested::ShowOnly(SetRef::Factory(set.id)),
            );
        }
        let after = (
            format!("{:?}", app.world().resource::<Session>().snapshot()),
            *app.world().resource::<crate::presentation::Presentation>(),
            *app.world().resource::<crate::aids::AidSettings>(),
        );
        assert_eq!(before.0, after.0, "Partida");
        assert_eq!(before.1, after.1, "Apresentação");
        assert_eq!(before.2, after.2, "Auxílios");
        assert!(
            app.world()
                .get_resource::<crate::ribbon::RibbonState>()
                .is_none(),
            "o Ribbon nem é consultado"
        );
    }

    #[test]
    fn show_only_through_a_request_then_save_and_reopen_the_saved_set() {
        let mut app = app();
        send(
            &mut app,
            PanelSetRequested::ShowOnly(SetRef::Factory("training")),
        );
        let layout = app.world().resource::<Dock>().layout.clone();
        assert_eq!(
            visible(&layout),
            strings(&[ids::AIDS, ids::NOTATION, ids::TRAINING])
        );
        send(
            &mut app,
            PanelSetRequested::SaveCurrent("Meu treino".into()),
        );
        assert_eq!(app.world().resource::<PanelSets>().error, None);
        send(
            &mut app,
            PanelSetRequested::ShowOnly(SetRef::Factory("match")),
        );
        send(
            &mut app,
            PanelSetRequested::ShowOnly(SetRef::Saved("Meu treino".into())),
        );
        let layout = app.world().resource::<Dock>().layout.clone();
        assert_eq!(
            visible(&layout),
            strings(&[ids::AIDS, ids::NOTATION, ids::TRAINING])
        );
    }

    #[test]
    fn a_refused_request_is_reported_and_cleared_by_the_next_one() {
        let mut app = app();
        send(&mut app, PanelSetRequested::SaveCurrent("Partida".into()));
        assert_eq!(
            app.world().resource::<PanelSets>().error,
            Some(SetError::NameTaken)
        );
        send(&mut app, PanelSetRequested::SaveCurrent("Livre".into()));
        assert_eq!(app.world().resource::<PanelSets>().error, None);
        assert_eq!(app.world().resource::<PanelSets>().saved().len(), 1);
    }

    #[test]
    fn saving_while_hide_all_is_active_saves_the_stored_open_panels() {
        let mut app = app();
        send(
            &mut app,
            PanelSetRequested::ShowOnly(SetRef::Factory("match")),
        );
        // Liga "Ocultar todos" pela Ação.
        let hidden_set = visible(&app.world().resource::<Dock>().layout.clone());
        app.world_mut()
            .resource_mut::<HiddenPanels>()
            .hide_for_test(hidden_set.clone());
        let mut dock = app.world_mut().resource_mut::<Dock>();
        let ids_all = dock.layout.pane_ids();
        for id in ids_all {
            dock.layout.set_visible(&id, false);
        }
        send(&mut app, PanelSetRequested::SaveCurrent("Oculto".into()));
        let saved = app.world().resource::<PanelSets>().saved()[0].clone();
        let mut got = saved.panels;
        got.sort();
        assert_eq!(got, hidden_set);
    }
}
