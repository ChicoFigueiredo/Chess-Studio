//! Execução da lista única de Ações (DS-14). Os atalhos de teclado do Mapa de controles viram
//! [`ActionRequested`]; o sistema `perform_actions` confere o estado da Ação
//! (`chess_input::actions::state`, pura) e a executa. Ribbon, Barra de Acesso Rápido, KeyTips e
//! Paleta de Ações vão escrever o mesmo [`ActionRequested`]: um só caminho para tudo.
//!
//! ADR-0001: aqui o Bevy só envia **intenções** à `Session` (Desfazer, Abandonar, Propor
//! empate…) e mexe em estado de interface (Apresentação, câmera, Painéis). Nada muta a Partida
//! por fora do controlador.
//!
//! `ClockPress` e `Cancel` são Ações de teclado cujo efeito depende de estado de quem as
//! consome (animação do botão do relógio, intérprete de gestos): `perform_actions` não as
//! executa; `clock` e `play` leem o mesmo [`ActionRequested`].

use crate::analysis::Analysis;
use crate::camera::CameraRig;
use crate::controls::{Controls, ControlsUi};
use crate::coordinates::BoardCoordinates;
use crate::dock::Dock;
use crate::focus::ViewState;
use crate::hover::HoverHighlight;
use crate::hud::HudSettings;
use crate::new_game::NewGameDialog;
use crate::presentation::Presentation;
use crate::session::Session;
use bevy::ecs::system::SystemParam;
use bevy::prelude::*;
use bevy_egui::input::EguiWantsInput;
use chess_game::Command;
use chess_input::CameraView;
use chess_input::actions::{ActionContext, PresentationId, state};
use chess_input::controls::Action;

/// O conjunto de Painéis que estava aberto quando "Ocultar todos os Painéis" foi acionada
/// (#183). `Some` = tudo oculto agora; a mesma Ação o devolve exatamente. Não é Perfil de
/// interface: o estado "tudo oculto" não vira "Personalizado" (ver `profiles::track_active`).
#[derive(Resource, Debug, Default, Clone, PartialEq, Eq)]
pub struct HiddenPanels(Option<Vec<String>>);

impl HiddenPanels {
    pub fn is_hidden(&self) -> bool {
        self.0.is_some()
    }

    #[cfg(test)]
    pub(crate) fn hide_for_test(&mut self, open: Vec<String>) {
        self.0 = Some(open);
    }

    /// Esquece o conjunto guardado (o usuário mexeu nos Painéis ou trocou de Perfil).
    pub fn clear(&mut self) {
        self.0 = None;
    }
}

/// Pedido para executar uma Ação (atalho, Ribbon, Paleta…).
#[derive(Message, Clone, Copy, Debug, PartialEq, Eq)]
pub struct ActionRequested(pub Action);

pub struct ActionsPlugin;

impl Plugin for ActionsPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<crate::ui_theme::Appearance>()
            .init_resource::<HiddenPanels>()
            .add_message::<ActionRequested>()
            .add_systems(
                Update,
                (dispatch_shortcuts, perform_actions)
                    .chain()
                    .after(crate::controls::capture_input)
                    .before(crate::camera::CameraSet)
                    .before(crate::play::PlaySet),
            );
    }
}

/// Atalhos de teclado do Mapa de controles -> pedidos de Ação. Com um campo de texto em foco
/// só valem as Ações que não digitam (Esc e teclas de função).
pub(crate) fn dispatch_shortcuts(
    keys: Res<ButtonInput<KeyCode>>,
    controls: Res<Controls>,
    egui: Option<Res<EguiWantsInput>>,
    ribbon: Option<Res<crate::ribbon::RibbonState>>,
    mut out: MessageWriter<ActionRequested>,
) {
    // Com os KeyTips ativos, as letras conduzem o Ribbon e não viram atalhos.
    if ribbon.is_some_and(|r| r.keys_captured) {
        return;
    }
    let typing = egui.is_some_and(|e| e.wants_keyboard_input());
    for action in controls.pressed_actions(&keys) {
        if !typing || action.fires_while_typing() {
            out.write(ActionRequested(action));
        }
    }
}

fn perform_actions(mut requests: MessageReader<ActionRequested>, mut env: ActionEnv) {
    for request in requests.read() {
        env.perform(request.0);
    }
}

/// Tudo de que o estado e a execução das Ações precisam.
#[derive(SystemParam)]
pub struct ActionEnv<'w> {
    session: ResMut<'w, Session>,
    dialog: ResMut<'w, NewGameDialog>,
    aids: Res<'w, crate::aids::AidSettings>,
    presentation: ResMut<'w, Presentation>,
    rig: ResMut<'w, CameraRig>,
    view: ResMut<'w, ViewState>,
    hud: ResMut<'w, HudSettings>,
    coordinates: ResMut<'w, BoardCoordinates>,
    hover: ResMut<'w, HoverHighlight>,
    analysis: ResMut<'w, Analysis>,
    dock: ResMut<'w, Dock>,
    hidden: ResMut<'w, HiddenPanels>,
    controls_ui: ResMut<'w, ControlsUi>,
    options_ui: ResMut<'w, crate::options::OptionsUi>,
    training: Res<'w, crate::training_ui::TrainingUi>,
    board2d: Option<ResMut<'w, crate::board2d_panel::Board2dPanel>>,
    appearance: ResMut<'w, crate::ui_theme::Appearance>,
    flip: ResMut<'w, crate::board2d::BoardFlip>,
    ribbon: Option<Res<'w, crate::ribbon::RibbonState>>,
    pub(crate) palette: ResMut<'w, crate::palette::PaletteState>,
}

fn presentation_id(p: Presentation) -> PresentationId {
    match p {
        Presentation::Professional3d => PresentationId::Professional3d,
        Presentation::ImmersiveTable => PresentationId::ImmersiveTable,
        Presentation::Technical2d => PresentationId::Technical2d,
    }
}

impl ActionEnv<'_> {
    /// O instantâneo do app de que o estado das Ações depende.
    pub fn context(&self) -> ActionContext {
        let snapshot = self.session.snapshot();
        let visible_panels = self
            .dock
            .layout
            .pane_ids()
            .into_iter()
            .filter(|id| self.dock.layout.is_visible(id))
            .collect();
        ActionContext {
            competitive: self.session.is_competitive(),
            competitive_in_progress: self.session.competitive_in_progress(),
            game_running: snapshot.outcome.is_none(),
            can_undo: snapshot.can_undo,
            can_redo: snapshot.can_redo,
            has_clock: snapshot.clock.is_some(),
            paused: snapshot.paused,
            draw_offer_pending: snapshot.pending_draw_offer.is_some(),
            claimable_draw: snapshot.claimable_draw.is_some(),
            engine_consented: self.analysis.engine_available(),
            analysis_on: self.analysis.is_enabled(),
            training_on_screen: self.training.session_swapped(),
            presentation: presentation_id(*self.presentation),
            camera_view: self.rig.view,
            fullscreen: self.view.fullscreen,
            focus: self.view.focus,
            hud_enabled: self.hud.enabled,
            coordinates: self.coordinates.0,
            hover_highlight: self.hover.0,
            board_flipped: self.flip.0,
            ribbon_collapsed: self.ribbon.as_ref().is_some_and(|r| r.collapsed),
            ribbon_visible: self.ribbon.is_some(),
            visible_panels,
            has_docked_panel: self.dock.layout.active_docked_panel().is_some(),
            has_floating_panel: !self.dock.layout.floating.is_empty(),
            panels_hidden: self.hidden.is_hidden(),
        }
    }

    /// Executa a Ação se ela está habilitada agora. Devolve se executou.
    pub fn perform(&mut self, action: Action) -> bool {
        if !state(action, &self.context()).is_enabled() {
            return false;
        }
        match action {
            Action::NewGame => {
                // A Nova Partida abre a tela de configuração (AC-01); a atual segue até
                // "Iniciar".
                let config = self.session.config().clone();
                self.dialog.open_from(&config, &self.aids);
            }
            Action::OpenControls => self.controls_ui.open = true,
            Action::OpenOptions => {
                self.options_ui.open = true;
                // Vinda da Paleta: ela fecha, o diálogo é modal.
                self.palette.close();
            }
            Action::OpenPalette => self.palette.toggle(),
            Action::Undo => self.session.undo(),
            Action::Redo => self.session.command(Command::Redo),
            Action::Resign => self.session.resign_side_to_move(),
            Action::OfferDraw => {
                let side = self.session.snapshot().side_to_move;
                self.session.command(Command::OfferDraw { side });
            }
            Action::ClaimDraw => self.session.claim_draw(None),
            Action::PauseClock => self.session.toggle_pause(),
            Action::PresentProfessional => self.present(Presentation::Professional3d),
            Action::PresentImmersive => self.present(Presentation::ImmersiveTable),
            Action::PresentTechnical => self.present(Presentation::Technical2d),
            Action::CyclePresentation => {
                let next = self.presentation.next();
                self.present(next);
            }
            Action::ViewTop => self.set_view(CameraView::Top),
            Action::ViewOblique => self.set_view(CameraView::Oblique),
            // "Resetar" é a Vista padrão.
            Action::ViewStandard | Action::ResetCamera => self.set_view(CameraView::Standard),
            Action::ViewInverted => self.set_view(CameraView::Inverted),
            // O enquadramento é refeito sobre a Vista ativa (`framing`); com a câmera livre,
            // volta à Vista padrão.
            Action::FrameBoard => self.set_view(self.rig.view.unwrap_or(CameraView::Standard)),
            Action::ToggleFullscreen => self.view.toggle_fullscreen(),
            Action::ToggleFocus => self.view.toggle_focus(),
            Action::ToggleHud => self.hud.enabled = !self.hud.enabled,
            Action::ToggleCoordinates => self.coordinates.0 = !self.coordinates.0,
            Action::ToggleHoverHighlight => self.hover.0 = !self.hover.0,
            Action::FlipBoard => self.flip_board(),
            Action::ToggleAnalysis => {
                let on = !self.analysis.is_enabled();
                self.analysis.set_enabled(on);
            }
            Action::FloatPanel => {
                if let Some(id) = self.dock.layout.active_docked_panel() {
                    self.dock.float(&id);
                }
            }
            Action::DockPanel => {
                if let Some(id) = self.dock.layout.floating.last().map(|f| f.id.0.clone()) {
                    self.dock.layout.dock_back(&id);
                }
            }
            Action::ResetLayout => {
                self.hidden.clear();
                self.dock.reset();
            }
            Action::HideAllPanels => self.toggle_hide_all(),
            Action::UiZoomIn => self.appearance.scale = self.appearance.scale.zoom_in(),
            Action::UiZoomOut => self.appearance.scale = self.appearance.scale.zoom_out(),
            other => match other.panel() {
                Some((id, _)) => self.toggle_panel(id),
                // Gestos, ClockPress/Cancel (consumidos por `clock` e `play`) e superfícies
                // que ainda não existem.
                None => return false,
            },
        }
        true
    }

    /// Vira o tabuleiro para o outro lado em todas as superfícies: a Vista 3D (padrão <->
    /// invertida; qualquer outra gira a câmera meia-volta e fica livre), o 2D técnico e o
    /// Painel Tabuleiro 2D.
    fn flip_board(&mut self) {
        self.flip.0 = !self.flip.0;
        if let Some(board) = self.board2d.as_mut() {
            board.flipped = !board.flipped;
        }
        match self.rig.view {
            Some(CameraView::Standard) => self.set_view(CameraView::Inverted),
            Some(CameraView::Inverted) => self.set_view(CameraView::Standard),
            _ => {
                self.rig.pose.yaw =
                    (self.rig.pose.yaw + std::f32::consts::PI).rem_euclid(std::f32::consts::TAU);
                self.rig.view = None;
            }
        }
    }

    fn present(&mut self, to: Presentation) {
        if *self.presentation != to {
            *self.presentation = to;
        }
    }

    fn set_view(&mut self, view: CameraView) {
        self.rig.pose = view.pose();
        self.rig.view = Some(view);
    }

    /// "Ocultar todos os Painéis": guarda quais estavam visíveis (encaixados e flutuantes) e
    /// oculta todos; de novo, devolve exatamente esses. Só a visibilidade muda: abas, divisões
    /// e janelas flutuantes ficam como estão.
    fn toggle_hide_all(&mut self) {
        let layout = &mut self.dock.layout;
        match self.hidden.0.take() {
            Some(open) => {
                for id in layout.pane_ids() {
                    layout.set_visible(&id, open.contains(&id));
                }
            }
            None => {
                let open: Vec<String> = layout
                    .pane_ids()
                    .into_iter()
                    .filter(|id| layout.is_visible(id))
                    .collect();
                for id in layout.pane_ids() {
                    layout.set_visible(&id, false);
                }
                self.hidden.0 = Some(open);
            }
        }
    }

    fn toggle_panel(&mut self, id: &str) {
        // Mexer num Painel encerra o "tudo oculto": vale o que o usuário acabou de escolher.
        self.hidden.clear();
        let visible = self.dock.layout.is_visible(id);
        self.dock.layout.set_visible(id, !visible);
        if !visible {
            self.dock.layout.activate(id);
        }
    }
}

/// App mínimo com tudo de que as Ações precisam, para os testes de atalho de outros módulos.
#[cfg(test)]
pub(crate) fn test_app() -> App {
    let mut app = App::new();
    app.init_resource::<ButtonInput<KeyCode>>()
        .init_resource::<Controls>()
        .init_resource::<ControlsUi>()
        .init_resource::<crate::options::OptionsUi>()
        .init_resource::<Session>()
        .init_resource::<NewGameDialog>()
        .init_resource::<crate::aids::AidSettings>()
        .init_resource::<Presentation>()
        .init_resource::<CameraRig>()
        .init_resource::<ViewState>()
        .init_resource::<HudSettings>()
        .init_resource::<BoardCoordinates>()
        .init_resource::<HoverHighlight>()
        .init_resource::<crate::ui_theme::Appearance>()
        .init_resource::<crate::board2d::BoardFlip>()
        .init_resource::<Analysis>()
        .init_resource::<Dock>()
        .init_resource::<HiddenPanels>()
        .init_resource::<crate::training_ui::TrainingUi>()
        .init_resource::<crate::palette::PaletteState>()
        .add_message::<ActionRequested>()
        .add_systems(Update, (dispatch_shortcuts, perform_actions).chain());
    app
}

#[cfg(test)]
pub(crate) fn press(app: &mut App, key: KeyCode) {
    app.world_mut()
        .resource_mut::<ButtonInput<KeyCode>>()
        .press(key);
    app.update();
    let mut keys = app.world_mut().resource_mut::<ButtonInput<KeyCode>>();
    keys.release(key);
    keys.clear();
}

#[cfg(test)]
mod tests {
    use super::*;
    use bevy::ecs::system::RunSystemOnce;

    #[test]
    fn dec_51_keys_reach_their_actions_through_one_path() {
        let mut app = test_app();
        press(&mut app, KeyCode::Tab);
        assert_eq!(
            *app.world().resource::<Presentation>(),
            Presentation::ImmersiveTable
        );
        press(&mut app, KeyCode::KeyH);
        assert!(!app.world().resource::<HudSettings>().enabled);
        press(&mut app, KeyCode::KeyH);
        assert!(app.world().resource::<HudSettings>().enabled);
        press(&mut app, KeyCode::KeyC);
        assert!(!app.world().resource::<BoardCoordinates>().0);
        press(&mut app, KeyCode::F11);
        assert!(app.world().resource::<ViewState>().fullscreen);
        press(&mut app, KeyCode::F10);
        assert!(app.world().resource::<ViewState>().focus);
    }

    #[test]
    fn view_keys_set_the_camera_view() {
        let mut app = test_app();
        press(&mut app, KeyCode::Digit1);
        assert_eq!(
            app.world().resource::<CameraRig>().view,
            Some(CameraView::Top)
        );
        press(&mut app, KeyCode::Numpad4);
        assert_eq!(
            app.world().resource::<CameraRig>().view,
            Some(CameraView::Inverted)
        );
        press(&mut app, KeyCode::Digit0);
        assert_eq!(
            app.world().resource::<CameraRig>().view,
            Some(CameraView::Standard)
        );
    }

    #[test]
    fn camera_views_are_ignored_in_the_2d_presentation() {
        let mut app = test_app();
        *app.world_mut().resource_mut::<Presentation>() = Presentation::Technical2d;
        press(&mut app, KeyCode::Digit1);
        assert_eq!(
            app.world().resource::<CameraRig>().view,
            Some(CameraView::Standard)
        );
    }

    #[test]
    fn action_requests_from_other_surfaces_use_the_same_path() {
        let mut app = test_app();
        app.world_mut()
            .write_message(ActionRequested(Action::PresentTechnical));
        app.update();
        assert_eq!(
            *app.world().resource::<Presentation>(),
            Presentation::Technical2d
        );
        app.world_mut()
            .write_message(ActionRequested(Action::ToggleHoverHighlight));
        app.update();
        assert!(!app.world().resource::<HoverHighlight>().0);
    }

    #[test]
    fn flip_board_turns_the_3d_view_the_2d_technical_and_the_board2d_panel() {
        let mut app = test_app();
        app.world_mut()
            .write_message(ActionRequested(Action::FlipBoard));
        app.update();
        assert_eq!(
            app.world().resource::<CameraRig>().view,
            Some(CameraView::Inverted)
        );
        assert!(app.world().resource::<crate::board2d::BoardFlip>().0);
        // Outra vez: volta à Vista padrão.
        app.world_mut()
            .write_message(ActionRequested(Action::FlipBoard));
        app.update();
        assert_eq!(
            app.world().resource::<CameraRig>().view,
            Some(CameraView::Standard)
        );
        assert!(!app.world().resource::<crate::board2d::BoardFlip>().0);
    }

    #[test]
    fn flipping_from_another_view_rotates_the_camera_half_a_turn() {
        let mut app = test_app();
        app.world_mut()
            .write_message(ActionRequested(Action::ViewOblique));
        app.update();
        let yaw = app.world().resource::<CameraRig>().pose.yaw;
        app.world_mut()
            .write_message(ActionRequested(Action::FlipBoard));
        app.update();
        let rig = app.world().resource::<CameraRig>();
        assert_eq!(rig.view, None);
        let diff = (rig.pose.yaw - yaw).rem_euclid(std::f32::consts::TAU);
        assert!((diff - std::f32::consts::PI).abs() < 1e-4);
    }

    #[test]
    fn disabled_actions_do_not_run() {
        let mut app = test_app();
        // Sem nada para desfazer: Ctrl+Z não faz nada (e não derruba a Partida).
        let revision = app.world().resource::<Session>().revision();
        app.world_mut()
            .resource_mut::<ButtonInput<KeyCode>>()
            .press(KeyCode::ControlLeft);
        press(&mut app, KeyCode::KeyZ);
        assert_eq!(app.world().resource::<Session>().revision(), revision);
    }

    #[test]
    fn new_game_opens_the_dialog_without_touching_the_game() {
        let mut app = test_app();
        let revision = app.world().resource::<Session>().revision();
        app.world_mut()
            .write_message(ActionRequested(Action::NewGame));
        app.update();
        assert!(app.world().resource::<NewGameDialog>().open);
        assert_eq!(app.world().resource::<Session>().revision(), revision);
    }

    #[test]
    fn panel_actions_toggle_the_dock_layout() {
        let mut app = test_app();
        let before = app
            .world()
            .resource::<Dock>()
            .layout
            .is_visible(crate::dock::ids::NOTATION);
        app.world_mut()
            .write_message(ActionRequested(Action::PanelNotation));
        app.update();
        let after = app
            .world()
            .resource::<Dock>()
            .layout
            .is_visible(crate::dock::ids::NOTATION);
        assert_ne!(before, after);
    }

    #[test]
    fn every_action_text_exists_in_pt_br() {
        use chess_input::actions::{RibbonTab, meta};
        let i18n = crate::i18n::I18n::pt_br();
        let known = |id: &str| i18n.t(id) != id;
        for tab in RibbonTab::ALL {
            assert!(known(tab.label()), "{}", tab.label());
        }
        for action in Action::ALL {
            let m = meta(action);
            assert!(known(m.label), "{}", m.label);
            assert!(known(m.description), "{}", m.description);
            if let Some(group) = m.group {
                assert!(known(group), "{group}");
            }
        }
        // Todo motivo de desabilitação possível tem texto.
        let contexts = [
            ActionContext::default(),
            ActionContext {
                competitive: true,
                competitive_in_progress: true,
                presentation: PresentationId::Technical2d,
                ..ActionContext::default()
            },
        ];
        for ctx in &contexts {
            for action in Action::ALL {
                if let Some(reason) = state(action, ctx).disabled_reason {
                    assert!(known(reason), "{reason}");
                }
            }
        }
    }

    #[test]
    fn every_control_screen_row_is_an_action_with_a_shortcut() {
        // As linhas da tela "Controles" são Ações; as que têm atalho de fábrica aparecem lá.
        let map = chess_input::controls::ControlMap::default();
        for action in Action::ALL {
            if !map.triggers(action).is_empty() {
                assert!(
                    crate::controls::listed_on_controls_screen(action),
                    "{action:?} tem atalho mas não aparece na tela Controles"
                );
            }
        }
    }

    fn visible_set(app: &App) -> Vec<String> {
        let layout = &app.world().resource::<Dock>().layout;
        let mut v: Vec<String> = layout
            .pane_ids()
            .into_iter()
            .filter(|id| layout.is_visible(id))
            .collect();
        v.sort();
        v
    }

    fn request(app: &mut App, action: Action) {
        app.world_mut().write_message(ActionRequested(action));
        app.update();
    }

    #[test]
    fn hiding_all_panels_and_doing_it_again_restores_the_exact_set_including_floating() {
        use crate::dock::ids;
        let mut app = test_app();
        {
            let mut dock = app.world_mut().resource_mut::<Dock>();
            for id in dock.layout.pane_ids() {
                dock.layout.set_visible(&id, false);
            }
            for id in [ids::NOTATION, ids::CLOCK, ids::BOARD2D] {
                dock.layout.set_visible(id, true);
            }
            assert!(dock.float(ids::BOARD2D));
        }
        let before = visible_set(&app);
        assert_eq!(before.len(), 3);
        assert!(
            app.world()
                .resource::<Dock>()
                .layout
                .is_floating(ids::BOARD2D)
        );

        request(&mut app, Action::HideAllPanels);
        assert!(
            visible_set(&app).is_empty(),
            "tudo oculto, flutuantes também"
        );
        assert!(!app.world().resource::<Dock>().layout.any_visible());
        // O Painel flutuante continua flutuante (só oculto).
        assert!(
            app.world()
                .resource::<Dock>()
                .layout
                .is_floating(ids::BOARD2D)
        );
        assert!(app.world().resource::<HiddenPanels>().is_hidden());

        request(&mut app, Action::HideAllPanels);
        assert_eq!(visible_set(&app), before);
        assert!(
            app.world()
                .resource::<Dock>()
                .layout
                .is_floating(ids::BOARD2D)
        );
        assert!(!app.world().resource::<HiddenPanels>().is_hidden());
    }

    #[test]
    fn ctrl_shift_f12_hides_and_restores_the_panels() {
        let mut app = test_app();
        let before = visible_set(&app);
        assert!(!before.is_empty());
        {
            let mut keys = app.world_mut().resource_mut::<ButtonInput<KeyCode>>();
            keys.press(KeyCode::ControlLeft);
            keys.press(KeyCode::ShiftLeft);
        }
        press(&mut app, KeyCode::F12);
        assert!(visible_set(&app).is_empty());
        press(&mut app, KeyCode::F12);
        assert_eq!(visible_set(&app), before);
    }

    #[test]
    fn opening_a_panel_while_all_are_hidden_ends_the_hidden_state_and_keeps_the_rest_hidden() {
        use crate::dock::ids;
        let mut app = test_app();
        request(&mut app, Action::HideAllPanels);
        request(&mut app, Action::PanelNotation);
        assert_eq!(visible_set(&app), vec![ids::NOTATION.to_owned()]);
        assert!(!app.world().resource::<HiddenPanels>().is_hidden());
        // Outra vez "ocultar": agora esconde só o que está aberto, e restaura isso.
        request(&mut app, Action::HideAllPanels);
        request(&mut app, Action::HideAllPanels);
        assert_eq!(visible_set(&app), vec![ids::NOTATION.to_owned()]);
    }

    #[test]
    fn the_checkbox_state_follows_the_dock_and_the_hidden_toggle_is_checked_while_hidden() {
        use crate::dock::ids;
        let mut app = test_app();
        app.world_mut()
            .resource_mut::<Dock>()
            .layout
            .set_visible(ids::NOTATION, true);
        let ctx = |app: &mut App| {
            app.world_mut()
                .run_system_once(|env: ActionEnv| env.context())
                .unwrap()
        };
        let c = ctx(&mut app);
        assert_eq!(state(Action::PanelNotation, &c).checked, Some(true));
        assert_eq!(state(Action::HideAllPanels, &c).checked, Some(false));
        request(&mut app, Action::HideAllPanels);
        let c = ctx(&mut app);
        assert_eq!(state(Action::PanelNotation, &c).checked, Some(false));
        assert_eq!(state(Action::HideAllPanels, &c).checked, Some(true));
    }

    #[test]
    fn every_dock_panel_id_has_an_action() {
        use crate::dock::ids;
        let ids = [
            ids::NOTATION,
            ids::CLOCK,
            ids::CAPTURED,
            ids::STUDY,
            ids::AIDS,
            ids::REVIEW,
            ids::ARBITRATION,
            ids::TRAINING,
            ids::ANALYSIS,
            ids::LIBRARY,
            ids::OPENINGS,
            ids::SCORESHEET,
            ids::BOARD2D,
            "Engines",
        ];
        for id in ids {
            assert!(
                Action::ALL
                    .into_iter()
                    .any(|a| a.panel().is_some_and(|(p, _)| p == id)),
                "Painel {id} sem Ação"
            );
        }
        let panels = Action::ALL
            .into_iter()
            .filter(|a| a.panel().is_some())
            .count();
        assert_eq!(panels, ids.len());
    }
}
