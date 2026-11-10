//! Câmera orbital própria. Toda a matemática (órbita/pan/zoom/limites/Vistas) vive em
//! `chess_input::view`; aqui só se traduzem eventos de entrada em chamadas a ela.
//!
//! Mapeamento (DEC-51, remapeável em `controls`): botão do meio + mover = órbita; Shift+meio ou direito + mover =
//! pan; roda = zoom; 1/2/3/4 = Vistas; 0 = reset; trackpad: dois dedos = órbita,
//! Shift + dois dedos = pan, pinça = zoom.
//!
//! Trackpad: macOS/iOS entregam `PanGesture`/`PinchGesture` (winit). Em Linux/Windows o
//! winit não os emite; dois dedos chegam como `MouseWheel` com unidade `Pixel` (tratado
//! como órbita/pan) e a roda do mouse, com unidade `Line`, é zoom.

use crate::controls::Controls;
use crate::presentation::Presentation;
use bevy::input::gestures::{PanGesture, PinchGesture};
use bevy::input::mouse::{AccumulatedMouseMotion, MouseScrollUnit, MouseWheel};
use bevy::prelude::*;
use bevy_egui::input::EguiWantsInput;
use chess_input::controls::{Action, GestureKind, Input};
use chess_input::{CameraPose, CameraView};
use std::collections::HashMap;

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

/// Câmera lembrada por Apresentação 3D (órbita, zoom, pan e Vista). Ao trocar de
/// Apresentação, a câmera de quem sai é guardada e a de quem entra é restaurada; a que
/// nunca foi visitada começa na Vista padrão. O 2D técnico tem câmera fixa e não entra aqui.
#[derive(Resource, Debug, Default)]
pub struct CameraMemory {
    saved: HashMap<Presentation, (CameraPose, Option<CameraView>)>,
}

impl CameraMemory {
    /// Define a câmera com que a Apresentação será aberta na próxima troca.
    pub fn set(&mut self, presentation: Presentation, pose: CameraPose, view: Option<CameraView>) {
        self.saved.insert(presentation, (pose, view));
    }

    /// Guarda a câmera de `from` e carrega a de `to` em `rig`.
    pub fn switch(&mut self, rig: &mut CameraRig, from: Option<Presentation>, to: Presentation) {
        if let Some(from) = from.filter(|p| p.is_3d()) {
            self.saved.insert(from, (rig.pose, rig.view));
        }
        if to.is_3d() {
            let (pose, view) = self
                .saved
                .get(&to)
                .copied()
                .unwrap_or_else(|| (CameraView::Standard.pose(), Some(CameraView::Standard)));
            rig.pose = pose;
            rig.view = view;
        }
    }
}

/// Troca a câmera ativa quando a Apresentação muda (roda no início do `CameraSet`).
pub(crate) fn switch_camera(
    presentation: Res<Presentation>,
    mut memory: ResMut<CameraMemory>,
    mut rig: ResMut<CameraRig>,
    mut last: Local<Option<Presentation>>,
) {
    if *last == Some(*presentation) {
        return;
    }
    // Como o conjunto só roda em 3D, `last` continua sendo a última Apresentação 3D vista,
    // dona da pose atual do `rig`, mesmo depois de uma passagem pelo 2D.
    memory.switch(&mut rig, *last, *presentation);
    *last = Some(*presentation);
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
            .init_resource::<CameraMemory>()
            .add_systems(Startup, spawn_camera)
            .add_systems(
                Update,
                (
                    switch_camera,
                    crate::framing::fit_active_view,
                    pointer_controls,
                    apply_pose,
                )
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

#[allow(clippy::too_many_arguments)]
fn pointer_controls(
    buttons: Res<ButtonInput<MouseButton>>,
    keys: Res<ButtonInput<KeyCode>>,
    controls: Res<Controls>,
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
    // Quais botões apertados agora órbitam ou fazem pan, segundo o mapa de controles
    // (com os modificadores de agora: Shift+meio é pan, meio sozinho é órbita).
    let mut orbit_drag = false;
    let mut pan_drag = false;
    for button in [
        MouseButton::Left,
        MouseButton::Middle,
        MouseButton::Right,
        MouseButton::Back,
        MouseButton::Forward,
    ] {
        if !buttons.pressed(button) {
            continue;
        }
        match controls.mouse_action(button, &keys) {
            Some(Action::Orbit) => orbit_drag = true,
            Some(Action::Pan) => pan_drag = true,
            _ => {}
        }
    }
    if egui.wants_pointer_input() && !(orbit_drag || pan_drag) {
        return;
    }

    let before = rig.pose;
    let mut pose = before;

    let delta = motion.delta;
    if delta != Vec2::ZERO {
        if pan_drag {
            pose = pan(pose, delta);
        } else if orbit_drag {
            pose = orbit(pose, delta);
        }
    }

    // Dois dedos (roda em pixels no Linux/Windows, `PanGesture` no macOS): órbita ou pan.
    let two_finger = controls.input_action(&Input::Gesture(GestureKind::TwoFinger), &keys);
    let two_finger_move = |pose: CameraPose, d: Vec2| match two_finger {
        Some(Action::Orbit) => orbit(pose, d),
        Some(Action::Pan) => pan(pose, d),
        _ => pose,
    };
    let wheel_zoom = controls.input_action(&Input::Wheel, &keys) == Some(Action::Zoom);
    for ev in wheel {
        match ev.unit {
            MouseScrollUnit::Line if wheel_zoom => {
                pose = pose.zoom(WHEEL_LINE_ZOOM.powf(ev.y));
            }
            MouseScrollUnit::Line => {}
            // Trackpad de dois dedos (Linux/Windows): arrastar o conteúdo, como no mouse.
            MouseScrollUnit::Pixel => pose = two_finger_move(pose, Vec2::new(ev.x, ev.y)),
        }
    }
    for p in pans {
        pose = two_finger_move(pose, p);
    }
    let pinch_zoom =
        controls.input_action(&Input::Gesture(GestureKind::Pinch), &keys) == Some(Action::Zoom);
    for p in pinch {
        if pinch_zoom {
            pose = pose.zoom((-p * PINCH_ZOOM).exp());
        }
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
