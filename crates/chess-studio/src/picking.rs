//! Picking das 64 casas. A fonte de verdade é `chess_input::ray_to_square` (geometria
//! pura), não o picking de mesh do Bevy. Só o botão esquerdo destaca: arrastar com o
//! botão do meio (órbita) nunca seleciona nem move peça (CAM-01).

use crate::board2d::{cursor_to_world, to_board_plane};
use crate::camera::{CameraSet, MainCamera};
use crate::presentation::Presentation;
use bevy::prelude::*;
use bevy::window::PrimaryWindow;
use bevy_egui::input::EguiWantsInput;
use chess_core::Square;
use chess_input::{ray_to_square, square_center};

/// Casa sob o cursor (se houver).
#[derive(Resource, Default, Debug)]
pub struct HoveredSquare(pub Option<Square>);

/// Casa destacada por clique esquerdo (apenas visual; não é Seleção de jogo).
#[derive(Resource, Default, Debug)]
pub struct ClickedSquare(pub Option<Square>);

#[derive(Component)]
struct HoverMarker;
#[derive(Component)]
struct ClickMarker;

/// Sistemas de picking (o 2D técnico atualiza seus marcadores depois deste conjunto).
#[derive(SystemSet, Debug, Clone, PartialEq, Eq, Hash)]
pub struct PickSet;

pub struct PickingPlugin;

impl Plugin for PickingPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<HoveredSquare>()
            .init_resource::<ClickedSquare>()
            .add_systems(Startup, spawn_markers)
            .add_systems(
                Update,
                (pick, update_markers)
                    .chain()
                    .in_set(PickSet)
                    .after(CameraSet),
            );
    }
}

fn marker_material(
    color: Color,
    materials: &mut Assets<StandardMaterial>,
) -> Handle<StandardMaterial> {
    materials.add(StandardMaterial {
        base_color: color,
        unlit: true,
        alpha_mode: AlphaMode::Blend,
        ..default()
    })
}

fn spawn_markers(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    let mesh = meshes.add(Cuboid::new(0.96, 0.02, 0.96));
    commands.spawn((
        HoverMarker,
        Mesh3d(mesh.clone()),
        MeshMaterial3d(marker_material(
            Color::srgba(0.2, 0.6, 1.0, 0.45),
            &mut materials,
        )),
        Transform::default(),
        Visibility::Hidden,
    ));
    commands.spawn((
        ClickMarker,
        Mesh3d(mesh),
        MeshMaterial3d(marker_material(
            Color::srgba(1.0, 0.8, 0.1, 0.55),
            &mut materials,
        )),
        Transform::default(),
        Visibility::Hidden,
    ));
}

#[allow(clippy::too_many_arguments)]
fn pick(
    window: Single<&Window, With<PrimaryWindow>>,
    camera: Single<(&Camera, &Transform), With<MainCamera>>,
    presentation: Res<Presentation>,
    buttons: Res<ButtonInput<MouseButton>>,
    egui: Res<EguiWantsInput>,
    mut hovered: ResMut<HoveredSquare>,
    mut clicked: ResMut<ClickedSquare>,
) {
    let (camera, transform) = *camera;
    let over_ui = egui.wants_pointer_input() || egui.is_pointer_over_area();
    let square = if over_ui {
        None
    } else {
        window.cursor_position().and_then(|cursor| {
            // Em todas as Apresentações a casa vem de `ray_to_square` (geometria pura).
            if presentation.is_3d() {
                let ray = camera
                    .viewport_to_world(&GlobalTransform::from(*transform), cursor)
                    .ok()?;
                ray_to_square(ray.origin, *ray.direction)
            } else {
                // 2D técnico: raio vertical sobre o ponto do tabuleiro sob o cursor.
                let world = cursor_to_world(cursor, window.size());
                ray_to_square(to_board_plane(world) + Vec3::Y, -Vec3::Y)
            }
        })
    };
    if hovered.0 != square {
        hovered.0 = square;
    }

    // Só o botão esquerdo destaca. O meio (órbita) e o direito (pan) nunca selecionam.
    if buttons.just_pressed(MouseButton::Left) && !over_ui {
        clicked.0 = square;
    }
}

#[allow(clippy::type_complexity)]
fn update_markers(
    hovered: Res<HoveredSquare>,
    clicked: Res<ClickedSquare>,
    mut markers: ParamSet<(
        Single<(&mut Transform, &mut Visibility), With<HoverMarker>>,
        Single<(&mut Transform, &mut Visibility), With<ClickMarker>>,
    )>,
) {
    let (mut t, mut v) = markers.p0().into_inner();
    place(&mut t, &mut v, hovered.0, 0.011);
    let (mut t, mut v) = markers.p1().into_inner();
    place(&mut t, &mut v, clicked.0, 0.012);
}

fn place(
    transform: &mut Transform,
    visibility: &mut Visibility,
    square: Option<Square>,
    y: f32,
) {
    match square {
        Some(sq) => {
            let c = square_center(sq);
            transform.translation = Vec3::new(c.x, y, c.z);
            *visibility = Visibility::Visible;
        }
        None => *visibility = Visibility::Hidden,
    }
}
