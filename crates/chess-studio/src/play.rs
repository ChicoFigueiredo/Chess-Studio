//! Entrada -> intérprete de gestos -> Partida. Traduz mouse, teclado e foco do Bevy em
//! eventos abstratos de `chess_input` e os entrega a `Session::gesture`. Nenhuma regra
//! mora aqui: o ponteiro só diz "pressionei na casa X"; o intérprete decide o Comando e a
//! Partida decide o Lance (ADR-0001).

use crate::camera::CameraSet;
use crate::controls::Controls;
use crate::picking::{CursorWorld, HoveredSquare, PickSet};
use crate::presentation::Presentation;
use crate::session::Session;
use bevy::prelude::*;
use bevy::window::WindowFocused;
use bevy_egui::input::EguiWantsInput;
use chess_input::controls::Action;
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

/// Ações que mudam a Vista; trocar a câmera no meio de um gesto o cancela.
const VIEW_ACTIONS: [Action; 5] = [
    Action::ViewTop,
    Action::ViewOblique,
    Action::ViewStandard,
    Action::ViewInverted,
    Action::ResetCamera,
];

/// Os botões do mouse que o mapa de controles pode usar.
const MOUSE_BUTTONS: [MouseButton; 5] = [
    MouseButton::Left,
    MouseButton::Middle,
    MouseButton::Right,
    MouseButton::Back,
    MouseButton::Forward,
];

#[allow(clippy::too_many_arguments)]
fn gesture_input(
    buttons: Res<ButtonInput<MouseButton>>,
    keys: Res<ButtonInput<KeyCode>>,
    controls: Res<Controls>,
    hovered: Res<HoveredSquare>,
    cursor: Res<CursorWorld>,
    egui: Res<EguiWantsInput>,
    presentation: Res<Presentation>,
    mut focus: MessageReader<WindowFocused>,
    mut session: ResMut<Session>,
    mut held_button: Local<Option<MouseButton>>,
    mut last_world: Local<Vec3>,
    mut last_presentation: Local<Option<Presentation>>,
    editor: Res<crate::pieces::BoardOverride>,
) {
    // O editor de posição (C11) usa o tabuleiro: os gestos não chegam à Partida.
    if editor.is_active() {
        *held_button = None;
        return;
    }
    // Capturando um novo atalho na tela "Controles": o tabuleiro não recebe nada.
    if controls.is_capturing() {
        *held_button = None;
        return;
    }
    let over_ui = egui.wants_pointer_input() || egui.is_pointer_over_area();

    // Cancelamentos primeiro: nunca geram Lance (INT-03/UX-09).
    if focus.read().any(|f| !f.focused) {
        session.gesture(GestureEvent::FocusLost);
    }
    if controls.key_just_pressed(Action::Cancel, &keys) {
        session.gesture(GestureEvent::Escape);
    }
    // `is_changed` não serve: o egui toma `&mut` do recurso todo quadro. Compara o valor.
    if last_presentation.is_some_and(|p| p != *presentation) {
        session.gesture(GestureEvent::PanelInteraction);
    }
    *last_presentation = Some(*presentation);
    if !egui.wants_keyboard_input()
        && VIEW_ACTIONS
            .iter()
            .any(|a| controls.key_just_pressed(*a, &keys))
    {
        session.gesture(GestureEvent::OrbitStarted);
    }
    // Cada botão apertado vira a ação que o mapa de controles diz (DEC-51 por padrão):
    // órbita e pan cancelam o gesto em curso; seleção e Ajuste tocam/arrastam a peça.
    for button in MOUSE_BUTTONS {
        if !buttons.just_pressed(button) {
            continue;
        }
        match controls.mouse_action(button, &keys) {
            Some(action @ (Action::Orbit | Action::Pan)) => {
                session.gesture(GestureEvent::PointerDown {
                    button: if action == Action::Orbit {
                        PointerButton::Middle
                    } else {
                        PointerButton::Secondary
                    },
                    square: hovered.0,
                    modifiers: Modifiers::default(),
                });
                session.gesture(GestureEvent::OrbitStarted);
            }
            Some(action @ (Action::Select | Action::Adjust)) => {
                if over_ui {
                    session.gesture(GestureEvent::PanelInteraction);
                } else {
                    *held_button = Some(button);
                    session.gesture(GestureEvent::PointerDown {
                        button: PointerButton::Primary,
                        square: hovered.0,
                        modifiers: Modifiers {
                            // O Ajuste (j'adoube) é o `alt` do intérprete, onde quer que o
                            // usuário o tenha posto.
                            alt: action == Action::Adjust,
                            shift: keys.any_pressed([KeyCode::ShiftLeft, KeyCode::ShiftRight]),
                            ctrl: keys.any_pressed([KeyCode::ControlLeft, KeyCode::ControlRight]),
                        },
                    });
                }
            }
            _ => {}
        }
    }
    if let Some(button) = *held_button
        && buttons.pressed(button)
    {
        // Sobre a interface o cursor não tem ponto no tabuleiro: mantém o último.
        *last_world = cursor.0.unwrap_or(*last_world);
        session.gesture(GestureEvent::PointerMove {
            square: hovered.0,
            world: *last_world,
        });
    }
    if let Some(button) = *held_button
        && buttons.just_released(button)
    {
        *held_button = None;
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
