//! Controles remapeáveis (US-28, DEC-51). O mapa ação -> atalhos (`chess_input::controls`) é
//! um recurso único, consultado por todos os sistemas de entrada (câmera, jogo, relógio,
//! Apresentação, Foco e tela cheia) em vez de teclas fixas. Aqui moram:
//!
//! - o recurso [`Controls`] e as consultas sobre o estado do Bevy (teclas, botões, gestos);
//! - a captura do próximo atalho (a tela "Controles" pede "pressione o novo atalho");
//! - a tela "Controles": reatribuir, acrescentar, remover, detectar conflito e restaurar.
//!
//! O mapa é gravado por perfil local (ver `persistence`). Nada aqui toca a Partida
//! (ADR-0001): os atalhos só decidem *qual intenção* o gesto expressa.

use crate::camera::CameraSet;
use crate::i18n::I18n;
use crate::play::PlaySet;
use bevy::input::gestures::{PanGesture, PinchGesture};
use bevy::input::mouse::{MouseScrollUnit, MouseWheel};
use bevy::prelude::*;
use bevy_egui::input::EguiWantsInput;
use bevy_egui::{EguiContexts, EguiPrimaryContextPass, egui};
use chess_input::controls::{
    Action, AssignError, AssignMode, ControlMap, GestureKind, Input, ModSet, MouseBtn, Trigger,
};

/// O mapa de controles em uso, mais o estado de "captura do próximo atalho".
#[derive(Resource, Debug, Default)]
pub struct Controls {
    map: ControlMap,
    capturing: bool,
}

impl Controls {
    pub fn with_map(map: ControlMap) -> Self {
        Self {
            map,
            capturing: false,
        }
    }

    pub fn map(&self) -> &ControlMap {
        &self.map
    }

    /// A tela "Controles" está esperando um novo atalho: nada mais deve reagir à entrada.
    pub fn is_capturing(&self) -> bool {
        self.capturing
    }

    /// As Ações cujo atalho de teclado acabou de ser apertado, na ordem das teclas. Vazio
    /// durante a captura. É por aqui que o atalho vira `ActionRequested` (`actions`).
    pub fn pressed_actions(&self, keys: &ButtonInput<KeyCode>) -> Vec<Action> {
        if self.capturing {
            return Vec::new();
        }
        let held = held_mods(keys);
        keys.get_just_pressed()
            .filter_map(|key| self.map.resolve(&key_input(*key), held))
            .collect()
    }

    /// Algum botão que seleciona (padrão: o esquerdo) acabou de ser apertado?
    pub fn select_just_pressed(
        &self,
        buttons: &ButtonInput<MouseButton>,
        keys: &ButtonInput<KeyCode>,
    ) -> bool {
        buttons
            .get_just_pressed()
            .any(|b| self.mouse_action(*b, keys) == Some(Action::Select))
    }

    /// A ação do botão do mouse com os modificadores apertados agora.
    pub fn mouse_action(&self, button: MouseButton, keys: &ButtonInput<KeyCode>) -> Option<Action> {
        if self.capturing {
            return None;
        }
        let button = mouse_btn(button)?;
        self.map.resolve(&Input::Mouse(button), held_mods(keys))
    }

    /// A ação do gesto de trackpad (ou da roda) com os modificadores apertados agora.
    pub fn input_action(&self, input: &Input, keys: &ButtonInput<KeyCode>) -> Option<Action> {
        if self.capturing {
            return None;
        }
        self.map.resolve(input, held_mods(keys))
    }
}

/// Modificadores apertados agora.
pub fn held_mods(keys: &ButtonInput<KeyCode>) -> ModSet {
    ModSet {
        ctrl: keys.any_pressed([KeyCode::ControlLeft, KeyCode::ControlRight]),
        shift: keys.any_pressed([KeyCode::ShiftLeft, KeyCode::ShiftRight]),
        alt: keys.any_pressed([KeyCode::AltLeft, KeyCode::AltRight]),
    }
}

/// O nome da tecla no mapa é o do `KeyCode` (`Digit1`, `F11`, `Tab`...).
pub fn key_input(code: KeyCode) -> Input {
    Input::Key(format!("{code:?}"))
}

fn mouse_btn(button: MouseButton) -> Option<MouseBtn> {
    match button {
        MouseButton::Left => Some(MouseBtn::Left),
        MouseButton::Middle => Some(MouseBtn::Middle),
        MouseButton::Right => Some(MouseBtn::Right),
        MouseButton::Back => Some(MouseBtn::Back),
        MouseButton::Forward => Some(MouseBtn::Forward),
        MouseButton::Other(_) => None,
    }
}

fn is_modifier_key(code: KeyCode) -> bool {
    matches!(
        code,
        KeyCode::ShiftLeft
            | KeyCode::ShiftRight
            | KeyCode::ControlLeft
            | KeyCode::ControlRight
            | KeyCode::AltLeft
            | KeyCode::AltRight
            | KeyCode::SuperLeft
            | KeyCode::SuperRight
    )
}

/// Nome legível de uma tecla (`Digit1` -> `1`, `Numpad1` -> `Num 1`, `KeyA` -> `A`).
pub fn key_label(name: &str) -> String {
    if let Some(d) = name.strip_prefix("Digit") {
        return d.to_owned();
    }
    if let Some(d) = name.strip_prefix("Numpad") {
        return format!("Num {d}");
    }
    if let Some(l) = name.strip_prefix("Key")
        && l.len() == 1
    {
        return l.to_owned();
    }
    match name {
        "Escape" => "Esc".to_owned(),
        "Space" => "Espaço".to_owned(),
        "ArrowUp" => "↑".to_owned(),
        "ArrowDown" => "↓".to_owned(),
        "ArrowLeft" => "←".to_owned(),
        "ArrowRight" => "→".to_owned(),
        other => other.to_owned(),
    }
}

/// Texto de um atalho para a tela (`Shift + Botão do meio`).
pub fn trigger_label(i18n: &I18n, trigger: &Trigger) -> String {
    let mut parts: Vec<String> = Vec::new();
    if trigger.mods.ctrl {
        parts.push("Ctrl".to_owned());
    }
    if trigger.mods.shift {
        parts.push("Shift".to_owned());
    }
    if trigger.mods.alt {
        parts.push("Alt".to_owned());
    }
    parts.push(match &trigger.input {
        Input::Key(name) => key_label(name),
        Input::Mouse(b) => i18n.t(match b {
            MouseBtn::Left => "ctl-mouse-left",
            MouseBtn::Middle => "ctl-mouse-middle",
            MouseBtn::Right => "ctl-mouse-right",
            MouseBtn::Back => "ctl-mouse-back",
            MouseBtn::Forward => "ctl-mouse-forward",
        }),
        Input::Wheel => i18n.t("ctl-wheel"),
        Input::Gesture(GestureKind::TwoFinger) => i18n.t("ctl-gesture-two-finger"),
        Input::Gesture(GestureKind::Pinch) => i18n.t("ctl-gesture-pinch"),
    });
    parts.join(" + ")
}

/// Aviso da tela depois de uma tentativa de atribuir.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Notice {
    Unsupported(Action),
    WouldLeaveEmpty(Action),
    Replaced(Action, Action),
}

/// Um atalho que já pertence a outra ação, esperando o usuário decidir.
#[derive(Debug, Clone, PartialEq, Eq)]
struct Pending {
    action: Action,
    trigger: Trigger,
    mode: AssignMode,
    owner: Action,
}

/// Estado da tela "Controles".
#[derive(Resource, Debug, Default)]
pub struct ControlsUi {
    pub open: bool,
    /// Ação esperando o próximo atalho.
    capture: Option<(Action, AssignMode)>,
    pending: Option<Pending>,
    notice: Option<Notice>,
}

impl ControlsUi {
    /// Evidências (`--controls`): abre a tela.
    pub fn opened() -> Self {
        Self {
            open: true,
            ..Self::default()
        }
    }
}

pub struct ControlsPlugin;

impl Plugin for ControlsPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<Controls>()
            .init_resource::<ControlsUi>()
            .add_systems(Update, capture_input.before(CameraSet).before(PlaySet))
            .add_systems(EguiPrimaryContextPass, controls_window);
    }
}

/// Aplica uma atribuição pedida pela captura (ou pelo "Substituir").
fn apply(
    map: &mut ControlMap,
    ui: &mut ControlsUi,
    action: Action,
    trigger: Trigger,
    mode: AssignMode,
) {
    ui.notice = None;
    match map.assign(action, trigger.clone(), mode) {
        Ok(()) => {}
        Err(AssignError::Unsupported) => ui.notice = Some(Notice::Unsupported(action)),
        Err(AssignError::WouldLeaveEmpty) => ui.notice = Some(Notice::WouldLeaveEmpty(action)),
        Err(AssignError::Conflict(owner)) => {
            ui.pending = Some(Pending {
                action,
                trigger,
                mode,
                owner,
            });
        }
    }
}

/// Lê o próximo atalho que o usuário faz, enquanto a tela "Controles" espera um. Roda antes
/// de tudo que usa a entrada; durante a captura `Controls::is_capturing` silencia o resto.
#[allow(clippy::too_many_arguments)]
pub(crate) fn capture_input(
    mut ui: ResMut<ControlsUi>,
    mut controls: ResMut<Controls>,
    keys: Res<ButtonInput<KeyCode>>,
    buttons: Res<ButtonInput<MouseButton>>,
    egui: Res<EguiWantsInput>,
    mut wheel: MessageReader<MouseWheel>,
    mut pinch: MessageReader<PinchGesture>,
    mut pans: MessageReader<PanGesture>,
) {
    // Esvazia os eventos sempre, para não acumularem.
    let wheel: Vec<MouseWheel> = wheel.read().cloned().collect();
    let pinched = pinch.read().count() > 0;
    let panned = pans.read().count() > 0;

    let capturing = ui.capture.is_some();
    if controls.capturing != capturing {
        controls.capturing = capturing;
    }
    let Some((action, mode)) = ui.capture else {
        return;
    };
    if keys.just_pressed(KeyCode::Escape) {
        ui.capture = None;
        return;
    }
    let mods = held_mods(&keys);
    let mut input = keys
        .get_just_pressed()
        .find(|k| !is_modifier_key(**k))
        .map(|k| key_input(*k));
    if input.is_none() {
        input = buttons
            .get_just_pressed()
            .filter_map(|b| mouse_btn(*b))
            // Clicar na interface é operar a tela, não escolher o botão esquerdo.
            .find(|b| *b != MouseBtn::Left || !egui.is_pointer_over_area())
            .map(Input::Mouse);
    }
    if input.is_none() {
        input = wheel
            .iter()
            .map(|w| match w.unit {
                MouseScrollUnit::Line => Input::Wheel,
                MouseScrollUnit::Pixel => Input::Gesture(GestureKind::TwoFinger),
            })
            .next();
    }
    if input.is_none() && pinched {
        input = Some(Input::Gesture(GestureKind::Pinch));
    }
    if input.is_none() && panned {
        input = Some(Input::Gesture(GestureKind::TwoFinger));
    }
    let Some(input) = input else { return };
    ui.capture = None;
    controls.capturing = false;
    apply(
        &mut controls.map,
        &mut ui,
        action,
        Trigger::new(input, mods),
        mode,
    );
}

/// Ações do mouse e do trackpad, depois as do teclado (a ordem da tela).
const POINTER_ACTIONS: [Action; 5] = [
    Action::Select,
    Action::Adjust,
    Action::Orbit,
    Action::Pan,
    Action::Zoom,
];
const KEY_ACTIONS: [Action; 23] = [
    Action::Cancel,
    Action::ClockPress,
    Action::ViewTop,
    Action::ViewOblique,
    Action::ViewStandard,
    Action::ViewInverted,
    Action::ResetCamera,
    Action::CyclePresentation,
    Action::ToggleFullscreen,
    Action::ToggleFocus,
    Action::HideAllPanels,
    Action::ToggleHud,
    Action::ToggleCoordinates,
    // Ações de menu com atalho de fábrica (DS-14).
    Action::NewGame,
    Action::Undo,
    Action::Redo,
    Action::FlipBoard,
    Action::OpenOptions,
    Action::OpenPalette,
    Action::ToggleRibbonCollapse,
    Action::UiZoomIn,
    Action::UiZoomOut,
    Action::UiZoomReset,
];

/// A Ação tem linha na tela "Controles".
#[cfg(test)]
pub(crate) fn listed_on_controls_screen(action: Action) -> bool {
    POINTER_ACTIONS.contains(&action) || KEY_ACTIONS.contains(&action)
}

pub fn action_label(i18n: &I18n, action: Action) -> String {
    i18n.t(chess_input::actions::meta(action).label)
}

/// Linhas de ajuda da janela principal, montadas do mapa atual (seguem o remapeamento).
pub fn help_lines(i18n: &I18n, map: &ControlMap) -> Vec<String> {
    let line = |actions: &[Action]| {
        actions
            .iter()
            .map(|a| {
                let triggers: Vec<String> = map
                    .triggers(*a)
                    .iter()
                    .map(|t| trigger_label(i18n, t))
                    .collect();
                format!("{}: {}", action_label(i18n, *a), triggers.join(" / "))
            })
            .collect::<Vec<_>>()
            .join(" | ")
    };
    vec![
        line(&[Action::Orbit, Action::Pan]),
        line(&[Action::Zoom, Action::ResetCamera]),
        line(&[Action::CyclePresentation, Action::Cancel]),
        line(&[Action::ToggleFullscreen, Action::ToggleFocus]),
    ]
}

fn controls_window(
    mut contexts: EguiContexts,
    i18n: Res<I18n>,
    mut state: ResMut<ControlsUi>,
    mut controls: ResMut<Controls>,
) -> Result {
    if !state.open {
        return Ok(());
    }
    let ctx = contexts.ctx_mut()?;
    let mut open = true;
    // Edita cópias e só devolve o que mudou: `ResMut` tocado todo quadro acionaria
    // `is_changed` em quem observa o mapa.
    let mut map = controls.map.clone();
    let mut ui_state = std::mem::take(&mut *state);
    let screen = ctx.viewport_rect();
    egui::Window::new(i18n.t("ctl-title"))
        .open(&mut open)
        .default_width(760.0)
        .default_height(560.0)
        .max_height((screen.height() - 40.0).max(240.0))
        .show(ctx, |ui| {
            egui::ScrollArea::vertical().show(ui, |ui| {
                ui.small(i18n.t("ctl-intro"));
                ui.add_space(4.0);
                ui.strong(i18n.t("ctl-group-pointer"));
                egui::Grid::new("ctl-pointer-grid")
                    .num_columns(2)
                    .spacing([12.0, 6.0])
                    .show(ui, |ui| {
                        for action in POINTER_ACTIONS {
                            action_row(ui, &i18n, &mut map, &mut ui_state, action);
                        }
                    });
                ui.add_space(6.0);
                ui.strong(i18n.t("ctl-group-keyboard"));
                egui::Grid::new("ctl-key-grid")
                    .num_columns(2)
                    .spacing([12.0, 6.0])
                    .show(ui, |ui| {
                        for action in KEY_ACTIONS {
                            action_row(ui, &i18n, &mut map, &mut ui_state, action);
                        }
                    });
                ui.add_space(6.0);
                status_area(ui, &i18n, &mut map, &mut ui_state);
                ui.separator();
                ui.add_enabled_ui(!map.is_default(), |ui| {
                    if ui.button(i18n.t("ctl-reset-all")).clicked() {
                        map.reset_all();
                        ui_state.pending = None;
                        ui_state.notice = None;
                    }
                });
            });
        });
    if !open {
        ui_state.open = false;
        ui_state.capture = None;
        ui_state.pending = None;
        ui_state.notice = None;
    }
    if map != controls.map {
        controls.map = map;
    }
    *state = ui_state;
    Ok(())
}

fn action_row(
    ui: &mut egui::Ui,
    i18n: &I18n,
    map: &mut ControlMap,
    state: &mut ControlsUi,
    action: Action,
) {
    ui.label(action_label(i18n, action));
    ui.horizontal(|ui| {
        let waiting = state.capture.is_some_and(|(a, _)| a == action);
        let triggers: Vec<Trigger> = map.triggers(action).to_vec();
        if triggers.is_empty() {
            ui.weak(i18n.t("ctl-none"));
        }
        let mut remove = None;
        for (index, trigger) in triggers.iter().enumerate() {
            ui.group(|ui| {
                ui.label(trigger_label(i18n, trigger));
                if ui
                    .small_button("×")
                    .on_hover_text(i18n.t("ctl-remove"))
                    .clicked()
                {
                    remove = Some(index);
                }
            });
        }
        if let Some(index) = remove {
            state.notice = match map.remove(action, index) {
                Err(AssignError::WouldLeaveEmpty) => Some(Notice::WouldLeaveEmpty(action)),
                _ => None,
            };
        }
        if waiting {
            ui.colored_label(ui.visuals().warn_fg_color, i18n.t("ctl-waiting"));
            if ui.button(i18n.t("ctl-cancel")).clicked() {
                state.capture = None;
            }
        } else {
            if ui.button(i18n.t("ctl-reassign")).clicked() {
                state.capture = Some((action, AssignMode::Replace));
                state.pending = None;
                state.notice = None;
            }
            if ui.button(i18n.t("ctl-add")).clicked() {
                state.capture = Some((action, AssignMode::Add));
                state.pending = None;
                state.notice = None;
            }
            if !map.is_action_default(action) && ui.button(i18n.t("ctl-default")).clicked() {
                map.reset_action(action);
                state.pending = None;
                state.notice = None;
            }
        }
    });
    ui.end_row();
}

/// Conflito pendente e avisos.
fn status_area(ui: &mut egui::Ui, i18n: &I18n, map: &mut ControlMap, state: &mut ControlsUi) {
    if let Some(pending) = state.pending.clone() {
        ui.group(|ui| {
            ui.colored_label(
                ui.visuals().warn_fg_color,
                i18n.t_with(
                    "ctl-conflict",
                    &[
                        ("trigger", &trigger_label(i18n, &pending.trigger)),
                        ("owner", &action_label(i18n, pending.owner)),
                        ("action", &action_label(i18n, pending.action)),
                    ],
                ),
            );
            ui.horizontal(|ui| {
                if ui.button(i18n.t("ctl-conflict-replace")).clicked() {
                    state.notice = match map.assign_stealing(
                        pending.action,
                        pending.trigger.clone(),
                        pending.mode,
                    ) {
                        Ok(Some(from)) => Some(Notice::Replaced(pending.action, from)),
                        Ok(None) => None,
                        Err(AssignError::WouldLeaveEmpty) => {
                            Some(Notice::WouldLeaveEmpty(pending.owner))
                        }
                        Err(_) => Some(Notice::Unsupported(pending.action)),
                    };
                    state.pending = None;
                }
                if ui.button(i18n.t("ctl-cancel")).clicked() {
                    state.pending = None;
                }
            });
        });
    }
    match state.notice {
        Some(Notice::Unsupported(action)) => {
            ui.colored_label(
                ui.visuals().error_fg_color,
                i18n.t_with(
                    "ctl-unsupported",
                    &[("action", &action_label(i18n, action))],
                ),
            );
        }
        Some(Notice::WouldLeaveEmpty(action)) => {
            ui.colored_label(
                ui.visuals().error_fg_color,
                i18n.t_with("ctl-needs-one", &[("action", &action_label(i18n, action))]),
            );
        }
        Some(Notice::Replaced(action, from)) => {
            ui.label(i18n.t_with(
                "ctl-replaced",
                &[
                    ("action", &action_label(i18n, action)),
                    ("from", &action_label(i18n, from)),
                ],
            ));
        }
        None => {}
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fired(controls: &Controls, action: Action, keys: &ButtonInput<KeyCode>) -> bool {
        controls.pressed_actions(keys).contains(&action)
    }

    fn keys_with(pressed: &[KeyCode], just: &[KeyCode]) -> ButtonInput<KeyCode> {
        let mut keys = ButtonInput::<KeyCode>::default();
        for k in pressed.iter().chain(just) {
            keys.press(*k);
        }
        keys.clear_just_pressed_all(pressed);
        keys
    }

    trait ClearAll {
        fn clear_just_pressed_all(&mut self, keys: &[KeyCode]);
    }

    impl ClearAll for ButtonInput<KeyCode> {
        fn clear_just_pressed_all(&mut self, keys: &[KeyCode]) {
            for k in keys {
                self.clear_just_pressed(*k);
            }
        }
    }

    #[test]
    fn default_controls_react_to_the_dec_51_keys() {
        let controls = Controls::default();
        let keys = keys_with(&[], &[KeyCode::Digit2]);
        assert!(fired(&controls, Action::ViewOblique, &keys));
        assert!(!fired(&controls, Action::ViewTop, &keys));
        let keys = keys_with(&[], &[KeyCode::F11]);
        assert!(fired(&controls, Action::ToggleFullscreen, &keys));
    }

    #[test]
    fn remapped_keys_replace_the_old_ones() {
        let mut map = ControlMap::default();
        map.assign(
            Action::CyclePresentation,
            Trigger::key("KeyP"),
            AssignMode::Replace,
        )
        .unwrap();
        let controls = Controls::with_map(map);
        assert!(!fired(
            &controls,
            Action::CyclePresentation,
            &keys_with(&[], &[KeyCode::Tab])
        ));
        assert!(fired(
            &controls,
            Action::CyclePresentation,
            &keys_with(&[], &[KeyCode::KeyP])
        ));
    }

    #[test]
    fn modifiers_choose_between_mouse_actions() {
        let controls = Controls::default();
        let plain = keys_with(&[], &[]);
        let alt = keys_with(&[KeyCode::AltLeft], &[]);
        let shift = keys_with(&[KeyCode::ShiftRight], &[]);
        assert_eq!(
            controls.mouse_action(MouseButton::Left, &plain),
            Some(Action::Select)
        );
        assert_eq!(
            controls.mouse_action(MouseButton::Left, &alt),
            Some(Action::Adjust)
        );
        assert_eq!(
            controls.mouse_action(MouseButton::Middle, &plain),
            Some(Action::Orbit)
        );
        assert_eq!(
            controls.mouse_action(MouseButton::Middle, &shift),
            Some(Action::Pan)
        );
        assert_eq!(controls.mouse_action(MouseButton::Other(9), &plain), None);
    }

    #[test]
    fn capturing_silences_every_query() {
        let controls = Controls {
            capturing: true,
            ..Controls::default()
        };
        assert!(!fired(
            &controls,
            Action::Cancel,
            &keys_with(&[], &[KeyCode::Escape])
        ));
        assert_eq!(
            controls.mouse_action(MouseButton::Left, &keys_with(&[], &[])),
            None
        );
    }

    #[test]
    fn key_labels_are_readable() {
        assert_eq!(key_label("Digit1"), "1");
        assert_eq!(key_label("Numpad1"), "Num 1");
        assert_eq!(key_label("KeyT"), "T");
        assert_eq!(key_label("Escape"), "Esc");
        assert_eq!(key_label("Space"), "Espaço");
        assert_eq!(key_label("F11"), "F11");
    }

    #[test]
    fn key_names_match_the_default_map() {
        // O nome gravado no mapa é o `Debug` do KeyCode: a DEC-51 depende disso.
        assert_eq!(key_input(KeyCode::Digit1), Input::Key("Digit1".into()));
        assert_eq!(key_input(KeyCode::Numpad0), Input::Key("Numpad0".into()));
        assert_eq!(key_input(KeyCode::Escape), Input::Key("Escape".into()));
        assert_eq!(key_input(KeyCode::Space), Input::Key("Space".into()));
    }
}
