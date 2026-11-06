//! Tabuleiro 8x8 e peças provisórias (primitivas, sem assets), luz e sombra.
//! Só geometria visual: nenhuma regra de jogo.

use bevy::prelude::*;
use chess_input::{BoardSquare, SQUARE_SIZE, square_center};

pub struct BoardPlugin;

impl Plugin for BoardPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Startup, spawn_scene);
    }
}

/// Peça provisória (posição inicial apenas visual).
#[derive(Component, Debug)]
#[allow(dead_code)] // lido pelos marcos seguintes
pub struct ProvisionalPiece {
    pub white: bool,
}

#[derive(Clone, Copy)]
enum Kind {
    Pawn,
    Rook,
    Knight,
    Bishop,
    Queen,
    King,
}

const BACK_RANK: [Kind; 8] = [
    Kind::Rook,
    Kind::Knight,
    Kind::Bishop,
    Kind::Queen,
    Kind::King,
    Kind::Bishop,
    Kind::Knight,
    Kind::Rook,
];

/// Espessura das casas; o topo fica em y = 0.
const SQUARE_THICKNESS: f32 = 0.2;

fn spawn_scene(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    let light_mat = materials.add(Color::srgb(0.90, 0.80, 0.62));
    let dark_mat = materials.add(Color::srgb(0.42, 0.27, 0.16));
    let frame_mat = materials.add(Color::srgb(0.25, 0.15, 0.09));
    let table_mat = materials.add(Color::srgb(0.16, 0.20, 0.17));

    let square_mesh = meshes.add(Cuboid::new(SQUARE_SIZE, SQUARE_THICKNESS, SQUARE_SIZE));
    for sq in BoardSquare::all() {
        // a1 é escura: file + rank par = escura.
        let dark = (sq.file() + sq.rank()) % 2 == 0;
        let c = square_center(sq);
        commands.spawn((
            Mesh3d(square_mesh.clone()),
            MeshMaterial3d(if dark {
                dark_mat.clone()
            } else {
                light_mat.clone()
            }),
            Transform::from_xyz(c.x, -SQUARE_THICKNESS / 2.0, c.z),
        ));
    }

    // Moldura e mesa.
    commands.spawn((
        Mesh3d(meshes.add(Cuboid::new(9.0, 0.3, 9.0))),
        MeshMaterial3d(frame_mat),
        Transform::from_xyz(0.0, -SQUARE_THICKNESS - 0.15 + 0.001, 0.0),
    ));
    commands.spawn((
        Mesh3d(meshes.add(Plane3d::default().mesh().size(60.0, 60.0))),
        MeshMaterial3d(table_mat),
        Transform::from_xyz(0.0, -0.5, 0.0),
    ));

    // Peças.
    let white_mat = materials.add(Color::srgb(0.95, 0.94, 0.90));
    let black_mat = materials.add(Color::srgb(0.12, 0.11, 0.12));
    for file in 0..8u8 {
        for (rank, white, kind) in [
            (0, true, BACK_RANK[usize::from(file)]),
            (1, true, Kind::Pawn),
            (6, false, Kind::Pawn),
            (7, false, BACK_RANK[usize::from(file)]),
        ] {
            let Some(sq) = BoardSquare::new(file, rank) else {
                continue;
            };
            let c = square_center(sq);
            let (mesh, height) = piece_mesh(kind, &mut meshes);
            commands.spawn((
                ProvisionalPiece { white },
                Mesh3d(mesh),
                MeshMaterial3d(if white {
                    white_mat.clone()
                } else {
                    black_mat.clone()
                }),
                Transform::from_xyz(c.x, height / 2.0, c.z),
            ));
        }
    }

    // Luz e sombra.
    commands.insert_resource(GlobalAmbientLight {
        color: Color::WHITE,
        brightness: 350.0,
        ..default()
    });
    commands.spawn((
        DirectionalLight {
            shadow_maps_enabled: true,
            illuminance: 9_000.0,
            ..default()
        },
        Transform::from_xyz(-5.0, 10.0, 6.0).looking_at(Vec3::ZERO, Vec3::Y),
    ));
}

/// Forma (cilindro ou cone) e altura por tipo.
fn piece_mesh(kind: Kind, meshes: &mut Assets<Mesh>) -> (Handle<Mesh>, f32) {
    match kind {
        Kind::Pawn => (meshes.add(Cylinder::new(0.22, 0.6)), 0.6),
        Kind::Rook => (meshes.add(Cylinder::new(0.32, 0.8)), 0.8),
        Kind::Knight => (meshes.add(Cone::new(0.32, 0.95)), 0.95),
        Kind::Bishop => (meshes.add(Cone::new(0.30, 1.1)), 1.1),
        Kind::Queen => (meshes.add(Cone::new(0.36, 1.3)), 1.3),
        Kind::King => (meshes.add(Cylinder::new(0.34, 1.5)), 1.5),
    }
}
