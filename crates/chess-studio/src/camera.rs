//! Câmera orbital própria. Toda a matemática (órbita/pan/zoom/limites/Vistas) vive em
//! `chess_input::view`; aqui só se traduzem eventos de entrada em chamadas a ela.
//!
//! Mapeamento (DEC-51): botão do meio + mover = órbita; Shift+meio ou direito + mover =
//! pan; roda = zoom; 1/2/3/4 = Vistas; 0 = reset; trackpad: dois dedos = órbita,
//! Shift + dois dedos = pan, pinça = zoom.
//!
//! Trackpad: macOS/iOS entregam `PanGesture`/`PinchGesture` (winit). Em Linux/Windows o
//! winit não os emite; dois dedos chegam como `MouseWheel` com unidade `Pixel` (tratado
//! como órbita/pan) e a roda do mouse, com unidade `Line`, é zoom.

use crate::presentation::Presentation;
use bevy::input::gestures::{PanGesture, PinchGesture};
use bevy::input::mouse::{AccumulatedMouseMotion, MouseScrollUnit, MouseWheel};
use bevy::prelude::*;
use bevy_egui::input::EguiWantsInput;
use chess_input::{CameraPose, CameraView};

const ORBIT_RAD_PER_PX: f32 = 0.005;
const PAN_PER_PX_PER_DISTANCE: f32 = 0.0012;
const WHEEL_LINE_ZOOM: f32 = 0.9;
const PINCH_ZOOM: f32 = 2.0;

/// Câmera atual (pose pura) e a Vista ativa; `None` = livre (o usuário moveu a câmera).
#[derive(Resource, Debug)]
pub struct CameraRig {
    pub pose: CameraPose,
    pub view: Option<CameraView>,
}

impl Default for CameraRig {
    fn default() -> Self {
        Self {
            pose: CameraView::Standard.pose(),
            view: Some(CameraView::Standard),
        }
    }
}

#[derive(Component)]
pub struct MainCamera;

/// Sistemas que atualizam a câmera; o picking roda depois deste conjunto.
#[derive(SystemSet, Debug, Clone, PartialEq, Eq, Hash)]
pub struct CameraSet;

pub struct CameraPlugin;

impl Plugin for CameraPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<CameraRig>()
            .add_systems(Startup, spawn_camera)
            .add_systems(
                Update,
                (view_keys, pointer_controls, apply_pose)
                    .chain()
                    .in_set(CameraSet)
                    // O 2D técnico tem câmera fixa de cima: sem órbita, pan nem Vistas.
                    .run_if(|p: Res<Presentation>| p.is_3d()),
            );
    }
}

fn spawn_camera(mut commands: Commands, rig: Res<CameraRig>) {
    commands.spawn((
        Camera3d::default(),
        MainCamera,
        pose_transform(&rig.pose),
        // Qualidade (MSAA/sombras) é aplicada por `presentation::apply_quality`.
    ));
}

fn pose_transform(pose: &CameraPose) -> Transform {
    Transform::from_translation(pose.position()).looking_at(pose.target, Vec3::Y)
}

fn view_keys(
    keys: Res<ButtonInput<KeyCode>>,
    egui: Res<EguiWantsInput>,
    mut rig: ResMut<CameraRig>,
) {
    if egui.wants_keyboard_input() {
        return;
    }
    let bindings = [
        (KeyCode::Digit1, CameraView::Top),
        (KeyCode::Digit2, CameraView::Oblique),
        (KeyCode::Digit3, CameraView::Standard),
        (KeyCode::Digit4, CameraView::Inverted),
        (KeyCode::Digit0, CameraView::Standard), // reset = Vista padrão
        (KeyCode::Numpad1, CameraView::Top),
        (KeyCode::Numpad2, CameraView::Oblique),
        (KeyCode::Numpad3, CameraView::Standard),
        (KeyCode::Numpad4, CameraView::Inverted),
        (KeyCode::Numpad0, CameraView::Standard),
    ];
    for (key, view) in bindings {
        if keys.just_pressed(key) {
            rig.pose = view.pose();
            rig.view = Some(view);
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn pointer_controls(
    buttons: Res<ButtonInput<MouseButton>>,
    keys: Res<ButtonInput<KeyCode>>,
    motion: Res<AccumulatedMouseMotion>,
    egui: Res<EguiWantsInput>,
    mut wheel: MessageReader<MouseWheel>,
    mut pinch: MessageReader<PinchGesture>,
    mut pan_gesture: MessageReader<PanGesture>,
    mut rig: ResMut<CameraRig>,
) {
    // Esvazia os eventos sempre, mesmo se o egui estiver com o ponteiro.
    let wheel: Vec<MouseWheel> = wheel.read().cloned().collect();
    let pinch: Vec<f32> = pinch.read().map(|p| p.0).collect();
    let pans: Vec<Vec2> = pan_gesture.read().map(|p| p.0).collect();
    let dragging = buttons.any_pressed([MouseButton::Middle, MouseButton::Right]);
    if egui.wants_pointer_input() && !dragging {
        return;
    }

    let shift = keys.any_pressed([KeyCode::ShiftLeft, KeyCode::ShiftRight]);
    let before = rig.pose;
    let mut pose = before;

    let delta = motion.delta;
    let middle = buttons.pressed(MouseButton::Middle);
    let right = buttons.pressed(MouseButton::Right);
    if delta != Vec2::ZERO {
        if (middle && shift) || right {
            pose = pan(pose, delta);
        } else if middle {
            pose = orbit(pose, delta);
        }
    }

    for ev in wheel {
        match ev.unit {
            MouseScrollUnit::Line => pose = pose.zoom(WHEEL_LINE_ZOOM.powf(ev.y)),
            // Trackpad de dois dedos (Linux/Windows): arrastar o conteúdo, como no mouse.
            MouseScrollUnit::Pixel => {
                let d = Vec2::new(ev.x, ev.y);
                pose = if shift { pan(pose, d) } else { orbit(pose, d) };
            }
        }
    }
    for p in pans {
        pose = if shift { pan(pose, p) } else { orbit(pose, p) };
    }
    for p in pinch {
        pose = pose.zoom((-p * PINCH_ZOOM).exp());
    }

    if pose != before {
        rig.pose = pose;
        rig.view = None;
    }
}

fn orbit(pose: CameraPose, px: Vec2) -> CameraPose {
    pose.orbit(-px.x * ORBIT_RAD_PER_PX, px.y * ORBIT_RAD_PER_PX)
}

fn pan(pose: CameraPose, px: Vec2) -> CameraPose {
    let k = PAN_PER_PX_PER_DISTANCE * pose.distance;
    pose.pan(-px.x * k, px.y * k)
}

fn apply_pose(rig: Res<CameraRig>, mut cameras: Query<&mut Transform, With<MainCamera>>) {
    if !rig.is_changed() {
        return;
    }
    for mut t in &mut cameras {
        *t = pose_transform(&rig.pose);
    }
}
