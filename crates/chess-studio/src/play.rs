//! Entrada -> intérprete de gestos -> Partida. Traduz mouse, teclado e foco do Bevy em
//! eventos abstratos de `chess_input` e os entrega a `Session::gesture`. Nenhuma regra
//! mora aqui: o ponteiro só diz "pressionei na casa X"; o intérprete decide o Comando e a
//! Partida decide o Lance (ADR-0001).

use crate::camera::CameraSet;
use crate::picking::{CursorWorld, HoveredSquare, PickSet};
use crate::presentation::Presentation;
use crate::session::Session;
use bevy::prelude::*;
use bevy::window::WindowFocused;
use bevy_egui::input::EguiWantsInput;
use chess_input::{GestureEvent, Modifiers, PointerButton};

/// Sistemas que alimentam a Partida a partir da entrada (a cena se reconstrói depois).
#[derive(SystemSet, Debug, Clone, PartialEq, Eq, Hash)]
pub struct PlaySet;

pub struct PlayPlugin;

impl Plugin for PlayPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<Session>().add_systems(
            Update,
            (gesture_input, mirror_selection)
                .chain()
                .in_set(PlaySet)
                .after(PickSet)
                .after(CameraSet),
        );
    }
}

/// Teclas que mudam a Vista; trocar a câmera no meio de um gesto o cancela.
const VIEW_KEYS: [KeyCode; 10] = [
    KeyCode::Digit0,
    KeyCode::Digit1,
    KeyCode::Digit2,
    KeyCode::Digit3,
    KeyCode::Digit4,
    KeyCode::Numpad0,
    KeyCode::Numpad1,
    KeyCode::Numpad2,
    KeyCode::Numpad3,
    KeyCode::Numpad4,
];

#[allow(clippy::too_many_arguments)]
fn gesture_input(
    buttons: Res<ButtonInput<MouseButton>>,
    keys: Res<ButtonInput<KeyCode>>,
    hovered: Res<HoveredSquare>,
    cursor: Res<CursorWorld>,
    egui: Res<EguiWantsInput>,
    presentation: Res<Presentation>,
    mut focus: MessageReader<WindowFocused>,
    mut session: ResMut<Session>,
    mut was_pressed: Local<bool>,
    mut last_world: Local<Vec3>,
    mut last_presentation: Local<Option<Presentation>>,
) {
    let over_ui = egui.wants_pointer_input() || egui.is_pointer_over_area();

    // Cancelamentos primeiro: nunca geram Lance (INT-03/UX-09).
    if focus.read().any(|f| !f.focused) {
        session.gesture(GestureEvent::FocusLost);
    }
    if keys.just_pressed(KeyCode::Escape) {
        session.gesture(GestureEvent::Escape);
    }
    // `is_changed` não serve: o egui toma `&mut` do recurso todo quadro. Compara o valor.
    if last_presentation.is_some_and(|p| p != *presentation) {
        session.gesture(GestureEvent::PanelInteraction);
    }
    *last_presentation = Some(*presentation);
    if !egui.wants_keyboard_input() && keys.any_just_pressed(VIEW_KEYS) {
        session.gesture(GestureEvent::OrbitStarted);
    }
    for (button, mapped) in [
        (MouseButton::Middle, PointerButton::Middle),
        (MouseButton::Right, PointerButton::Secondary),
    ] {
        if buttons.just_pressed(button) {
            session.gesture(GestureEvent::PointerDown {
                button: mapped,
                square: hovered.0,
                modifiers: Modifiers::default(),
            });
            session.gesture(GestureEvent::OrbitStarted);
        }
    }

    // Botão esquerdo.
    if buttons.just_pressed(MouseButton::Left) {
        if over_ui {
            session.gesture(GestureEvent::PanelInteraction);
        } else {
            *was_pressed = true;
            session.gesture(GestureEvent::PointerDown {
                button: PointerButton::Primary,
                square: hovered.0,
                modifiers: Modifiers {
                    alt: keys.any_pressed([KeyCode::AltLeft, KeyCode::AltRight]),
                    shift: keys.any_pressed([KeyCode::ShiftLeft, KeyCode::ShiftRight]),
                    ctrl: keys.any_pressed([KeyCode::ControlLeft, KeyCode::ControlRight]),
                },
            });
        }
    }
    if *was_pressed && buttons.pressed(MouseButton::Left) {
        // Sobre a interface o cursor não tem ponto no tabuleiro: mantém o último.
        *last_world = cursor.0.unwrap_or(*last_world);
        session.gesture(GestureEvent::PointerMove {
            square: hovered.0,
            world: *last_world,
        });
    }
    if buttons.just_released(MouseButton::Left) && *was_pressed {
        *was_pressed = false;
        // Soltar sobre a interface cancela o arrasto (a casa é `None`).
        session.gesture(GestureEvent::PointerUp {
            square: if over_ui { None } else { hovered.0 },
        });
    }
}

/// Espelha a origem do gesto para os marcadores de seleção.
/// Sem o Auxílio "destaque da peça selecionada" o marcador não aparece (UX-01).
fn mirror_selection(
    session: Res<Session>,
    aids: Res<crate::aids::AidSettings>,
    mut selected: ResMut<crate::picking::SelectedSquare>,
) {
    let origin = session.gesture_visual().origin.filter(|_| aids.selected);
    if selected.0 != origin {
        selected.0 = origin;
    }
}
