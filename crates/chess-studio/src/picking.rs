//! Picking das 64 casas. A fonte de verdade é `chess_input::ray_to_square` (geometria
//! pura), não o picking de mesh do Bevy. Aqui só se descobre a casa (e o ponto do plano)
//! sob o cursor; quem interpreta cliques e arrastos é o intérprete de gestos (`play`).

use crate::board2d::{cursor_to_world, to_board_plane};
use crate::camera::{CameraSet, MainCamera};
use crate::presentation::Presentation;
use bevy::prelude::*;
use bevy::window::PrimaryWindow;
use bevy_egui::input::EguiWantsInput;
use chess_core::Square;
use chess_input::{point_to_square, ray_to_board_plane, square_center};

/// Casa sob o cursor (se houver).
#[derive(Resource, Default, Debug)]
pub struct HoveredSquare(pub Option<Square>);

/// Ponto do plano do tabuleiro sob o cursor (mesmo fora das 64 casas); `None` sobre a
/// interface ou sem cursor. Serve para desenhar a peça arrastada.
#[derive(Resource, Default, Debug)]
pub struct CursorWorld(pub Option<Vec3>);

/// Origem do gesto em curso (peça selecionada/arrastada), espelhada de `Session`.
#[derive(Resource, Default, Debug)]
pub struct SelectedSquare(pub Option<Square>);

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
            .init_resource::<CursorWorld>()
            .init_resource::<SelectedSquare>()
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

fn pick(
    window: Single<&Window, With<PrimaryWindow>>,
    camera: Single<(&Camera, &Transform), With<MainCamera>>,
    presentation: Res<Presentation>,
    egui: Res<EguiWantsInput>,
    dock: Res<crate::dock::Dock>,
    mut hovered: ResMut<HoveredSquare>,
    mut world: ResMut<CursorWorld>,
) {
    let (camera, transform) = *camera;
    let over_ui = egui.wants_pointer_input() || egui.is_pointer_over_area();
    let point = if over_ui {
        None
    } else {
        window
            .cursor_position()
            .filter(|cursor| dock.pointer_in_board(*cursor))
            .and_then(|cursor| {
                // Em todas as Apresentações o ponto vem da geometria pura de `chess_input`.
                if presentation.is_3d() {
                    let ray = camera
                        .viewport_to_world(&GlobalTransform::from(*transform), cursor)
                        .ok()?;
                    ray_to_board_plane(ray.origin, *ray.direction)
                } else {
                    // 2D técnico: raio vertical sobre o ponto do tabuleiro sob o cursor.
                    Some(to_board_plane(cursor_to_world(cursor, window.size())))
                }
            })
    };
    let square = point.and_then(point_to_square);
    if hovered.0 != square {
        hovered.0 = square;
    }
    if world.0 != point {
        world.0 = point;
    }
}

#[allow(clippy::type_complexity)]
fn update_markers(
    hovered: Res<HoveredSquare>,
    aids: Res<crate::aids::AidSettings>,
    selected: Res<SelectedSquare>,
    mut markers: ParamSet<(
        Single<(&mut Transform, &mut Visibility), With<HoverMarker>>,
        Single<(&mut Transform, &mut Visibility), With<ClickMarker>>,
    )>,
) {
    let (mut t, mut v) = markers.p0().into_inner();
    place(&mut t, &mut v, hovered.0.filter(|_| aids.selected), 0.011);
    let (mut t, mut v) = markers.p1().into_inner();
    place(&mut t, &mut v, selected.0, 0.012);
}

fn place(transform: &mut Transform, visibility: &mut Visibility, square: Option<Square>, y: f32) {
    match square {
        Some(sq) => {
            let c = square_center(sq);
            transform.translation = Vec3::new(c.x, y, c.z);
            *visibility = Visibility::Visible;
        }
        None => *visibility = Visibility::Hidden,
    }
}
