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
use bevy::window::{PrimaryWindow, WindowFocused};
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

/// Um arrasto só começa depois que o cursor se afasta do ponto do press por mais que isto,
/// em pixels lógicos (já independentes da escala HiDPI). Abaixo disso, press + release é um
/// toque, mesmo que a mão trema e o picking troque de casa numa borda.
pub const DRAG_THRESHOLD_PX: f32 = 6.0;

/// O cursor saiu do raio de tolerância do press?
fn exceeds_drag_threshold(press: Vec2, now: Vec2) -> bool {
    press.distance(now) > DRAG_THRESHOLD_PX
}

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
    windows: Query<&Window, With<PrimaryWindow>>,
    mut press_at: Local<Option<Vec2>>,
    mut drag_started: Local<bool>,
    (editor, panel, frames): (
        Res<crate::pieces::BoardOverride>,
        Option<Res<crate::board2d_panel::Board2dPanel>>,
        Res<bevy::diagnostic::FrameCount>,
    ),
) {
    let panel_playing = panel.is_some_and(|p| p.pointer_over_playable(frames.0));
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
                    // Sobre o painel "Tabuleiro 2D" jogável o gesto é dele: cancelar aqui
                    // desfaria a seleção do primeiro clique.
                    if !panel_playing {
                        session.gesture(GestureEvent::PanelInteraction);
                    }
                } else {
                    *held_button = Some(button);
                    *press_at = windows.single().ok().and_then(Window::cursor_position);
                    *drag_started = false;
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
        let now = windows.single().ok().and_then(Window::cursor_position);
        if !*drag_started {
            *drag_started = match (*press_at, now) {
                (Some(press), Some(now)) => exceeds_drag_threshold(press, now),
                // Sem posição lógica (sem cursor): não há como medir, vale o picking.
                _ => true,
            };
        }
        if *drag_started {
            session.gesture(GestureEvent::PointerMove {
                square: hovered.0,
                world: *last_world,
            });
        }
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::pieces::BoardOverride;
    use chess_core::Square;
    use chess_input::MoveMethod;

    fn sq(name: &str) -> Square {
        name.parse().unwrap()
    }

    fn app(method: MoveMethod) -> App {
        let mut app = App::new();
        app.add_plugins(MinimalPlugins)
            .init_resource::<ButtonInput<MouseButton>>()
            .init_resource::<ButtonInput<KeyCode>>()
            .init_resource::<Controls>()
            .init_resource::<HoveredSquare>()
            .init_resource::<CursorWorld>()
            .init_resource::<EguiWantsInput>()
            .init_resource::<Presentation>()
            .init_resource::<BoardOverride>()
            .init_resource::<Session>()
            .add_message::<WindowFocused>()
            .add_systems(Update, gesture_input);
        app.world_mut().spawn((Window::default(), PrimaryWindow));
        app.world_mut()
            .resource_mut::<Session>()
            .set_move_method(method);
        app
    }

    /// Um clique de verdade: o botão desce num quadro e sobe em outro, com o cursor parado.
    fn click(app: &mut App, at: &str) {
        app.world_mut().resource_mut::<HoveredSquare>().0 = Some(sq(at));
        let mut buttons = app.world_mut().resource_mut::<ButtonInput<MouseButton>>();
        buttons.press(MouseButton::Left);
        app.update();
        let mut buttons = app.world_mut().resource_mut::<ButtonInput<MouseButton>>();
        buttons.clear();
        buttons.release(MouseButton::Left);
        app.update();
        app.world_mut()
            .resource_mut::<ButtonInput<MouseButton>>()
            .clear();
        app.update();
    }

    fn played(app: &App) -> usize {
        app.world().resource::<Session>().snapshot().history.len()
    }

    #[test]
    fn two_taps_play_the_move_in_every_method_that_admits_them() {
        for method in [MoveMethod::Both, MoveMethod::TwoTaps] {
            let mut app = app(method);
            click(&mut app, "e2");
            assert_eq!(played(&app), 0);
            click(&mut app, "e4");
            assert_eq!(played(&app), 1, "{method:?}");
        }
    }

    fn cursor_to(app: &mut App, x: f32, y: f32) {
        let mut q = app
            .world_mut()
            .query_filtered::<&mut Window, With<PrimaryWindow>>();
        let mut window = q.single_mut(app.world_mut()).unwrap();
        window
            .set_physical_cursor_position(Some(bevy::math::DVec2::new(f64::from(x), f64::from(y))));
    }

    /// Press na peça, move `dx` px (e o picking passa para `moved_to`), release.
    fn press_move_release(app: &mut App, dx: f32, moved_to: &str) {
        cursor_to(app, 100.0, 100.0);
        app.world_mut().resource_mut::<HoveredSquare>().0 = Some(sq("e2"));
        app.world_mut()
            .resource_mut::<ButtonInput<MouseButton>>()
            .press(MouseButton::Left);
        app.update();
        app.world_mut()
            .resource_mut::<ButtonInput<MouseButton>>()
            .clear();
        cursor_to(app, 100.0 + dx, 100.0);
        app.world_mut().resource_mut::<HoveredSquare>().0 = Some(sq(moved_to));
        app.update();
        let mut buttons = app.world_mut().resource_mut::<ButtonInput<MouseButton>>();
        buttons.clear();
        buttons.release(MouseButton::Left);
        app.update();
        app.world_mut()
            .resource_mut::<ButtonInput<MouseButton>>()
            .clear();
    }

    #[test]
    fn a_3px_wobble_across_a_square_edge_is_still_a_tap_that_selects() {
        let mut app = app(MoveMethod::Both);
        press_move_release(&mut app, 3.0, "e3");
        assert_eq!(played(&app), 0);
        let origin = app.world().resource::<Session>().gesture_visual().origin;
        assert_eq!(origin, Some(sq("e2")), "toque seleciona, não cancela");
        click(&mut app, "e4");
        assert_eq!(played(&app), 1);
    }

    #[test]
    fn a_20px_move_is_a_normal_drag() {
        let mut app = app(MoveMethod::Both);
        press_move_release(&mut app, 20.0, "e4");
        assert_eq!(played(&app), 1);
    }
}
