//! Desenho dos Auxílios visuais no tabuleiro (3D e 2D técnico): pontos nos destinos
//! legais, peças fantasmas, último Lance e rei em xeque. O que desenhar vem pronto de
//! [`Marks`] (`aids`); este módulo só lê e nunca toca a Partida. Com todos os Auxílios
//! desligados nada é desenhado (UX-01).

use crate::aids::{Marks, MarksSet};
use crate::board::PIECE_SCALE;
use crate::board2d::{UNIT, square_pos};
use crate::layout::glyph;
use crate::pieces::{PieceFont, PieceTemplates};
use crate::play::PlaySet;
use crate::presentation::VisibleIn;
use bevy::prelude::*;
use chess_core::{Color as Side, Square};
use chess_input::square_center;

/// Máximo de destinos legais de uma peça (dama no centro: 27).
const MAX_DESTINATIONS: usize = 28;

const LAST_MOVE_COLOR: Color = Color::srgba(0.95, 0.80, 0.15, 0.30);
const CHECK_COLOR: Color = Color::srgba(0.90, 0.10, 0.10, 0.55);

#[derive(Component)]
struct Dest3d(usize);
#[derive(Component)]
struct Dest2d(usize);
#[derive(Component)]
struct LastMove3d(usize);
#[derive(Component)]
struct LastMove2d(usize);
#[derive(Component)]
struct Check3d;
#[derive(Component)]
struct Check2d;
/// Peça fantasma (3D ou 2D); recriada quando o conjunto de fantasmas muda.
#[derive(Component)]
struct GhostPiece;

pub struct HighlightPlugin;

impl Plugin for HighlightPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Startup, spawn_markers).add_systems(
            Update,
            (update_markers, update_ghosts)
                .after(PlaySet)
                .after(MarksSet),
        );
    }
}

fn flat_marker(color: Color) -> StandardMaterial {
    StandardMaterial {
        base_color: color,
        unlit: true,
        alpha_mode: AlphaMode::Blend,
        ..default()
    }
}

fn spawn_markers(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    let dot = meshes.add(Cylinder::new(0.17, 0.02));
    let tile = meshes.add(Cuboid::new(0.96, 0.02, 0.96));
    let dot_material = materials.add(flat_marker(Color::srgba(0.15, 0.75, 0.35, 0.7)));
    let last_material = materials.add(flat_marker(LAST_MOVE_COLOR));
    let check_material = materials.add(flat_marker(CHECK_COLOR));
    let size = Vec2::splat(UNIT);
    for i in 0..MAX_DESTINATIONS {
        commands.spawn((
            Dest3d(i),
            Mesh3d(dot.clone()),
            MeshMaterial3d(dot_material.clone()),
            Transform::default(),
            Visibility::Hidden,
        ));
        commands.spawn((
            Dest2d(i),
            Sprite::from_color(Color::srgba(0.1, 0.55, 0.25, 0.75), Vec2::splat(UNIT * 0.3)),
            Transform::from_xyz(0.0, 0.0, 0.6),
            Visibility::Hidden,
            VisibleIn::TECHNICAL,
        ));
    }
    for i in 0..2 {
        commands.spawn((
            LastMove3d(i),
            Mesh3d(tile.clone()),
            MeshMaterial3d(last_material.clone()),
            Transform::default(),
            Visibility::Hidden,
        ));
        commands.spawn((
            LastMove2d(i),
            Sprite::from_color(LAST_MOVE_COLOR, size),
            Transform::from_xyz(0.0, 0.0, 0.3),
            Visibility::Hidden,
            VisibleIn::TECHNICAL,
        ));
    }
    commands.spawn((
        Check3d,
        Mesh3d(tile),
        MeshMaterial3d(check_material),
        Transform::default(),
        Visibility::Hidden,
    ));
    commands.spawn((
        Check2d,
        Sprite::from_color(CHECK_COLOR, size),
        Transform::from_xyz(0.0, 0.0, 0.35),
        Visibility::Hidden,
        VisibleIn::TECHNICAL,
    ));
}

fn place3d(transform: &mut Transform, visibility: &mut Visibility, square: Option<Square>, y: f32) {
    match square {
        Some(sq) => {
            let c = square_center(sq);
            transform.translation = Vec3::new(c.x, y, c.z);
            *visibility = Visibility::Visible;
        }
        None => *visibility = Visibility::Hidden,
    }
}

fn place2d(transform: &mut Transform, visibility: &mut Visibility, square: Option<Square>, z: f32) {
    match square {
        Some(sq) => {
            let p = square_pos(sq);
            transform.translation = Vec3::new(p.x, p.y, z);
            *visibility = Visibility::Visible;
        }
        None => *visibility = Visibility::Hidden,
    }
}

#[allow(clippy::type_complexity)]
fn update_markers(
    marks: Res<Marks>,
    mut q: ParamSet<(
        Query<(&Dest3d, &mut Transform, &mut Visibility)>,
        Query<(&Dest2d, &mut Transform, &mut Visibility)>,
        Query<(&LastMove3d, &mut Transform, &mut Visibility)>,
        Query<(&LastMove2d, &mut Transform, &mut Visibility)>,
        Single<(&mut Transform, &mut Visibility), With<Check3d>>,
        Single<(&mut Transform, &mut Visibility), With<Check2d>>,
    )>,
) {
    // Todo quadro: a troca de Apresentação reexibe as entidades do 2D técnico
    // (`VisibleIn`), então a visibilidade precisa ser reafirmada.
    for (marker, mut t, mut v) in &mut q.p0() {
        place3d(
            &mut t,
            &mut v,
            marks.destinations.get(marker.0).copied(),
            0.015,
        );
    }
    for (marker, mut t, mut v) in &mut q.p1() {
        place2d(
            &mut t,
            &mut v,
            marks.destinations.get(marker.0).copied(),
            0.6,
        );
    }
    let last_square = |i: usize| {
        marks
            .last_move
            .map(|(from, to)| if i == 0 { from } else { to })
    };
    for (marker, mut t, mut v) in &mut q.p2() {
        place3d(&mut t, &mut v, last_square(marker.0), 0.010);
    }
    for (marker, mut t, mut v) in &mut q.p3() {
        place2d(&mut t, &mut v, last_square(marker.0), 0.3);
    }
    let (mut t, mut v) = q.p4().into_inner();
    place3d(&mut t, &mut v, marks.check, 0.013);
    let (mut t, mut v) = q.p5().into_inner();
    place2d(&mut t, &mut v, marks.check, 0.35);
}

/// Peças fantasmas: recriadas só quando o conjunto muda.
fn update_ghosts(
    marks: Res<Marks>,
    templates: Res<PieceTemplates>,
    font: Option<Res<PieceFont>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    existing: Query<Entity, With<GhostPiece>>,
    mut commands: Commands,
) {
    if !marks.is_changed() {
        return;
    }
    for entity in &existing {
        commands.entity(entity).despawn();
    }
    let Some(font) = font else {
        return;
    };
    let material = materials.add(StandardMaterial {
        base_color: Color::srgba(0.85, 0.95, 1.0, 0.38),
        unlit: true,
        alpha_mode: AlphaMode::Blend,
        ..default()
    });
    for ghost in &marks.ghosts {
        let facing = if ghost.side == Side::White {
            std::f32::consts::PI
        } else {
            0.0
        };
        commands
            .spawn((
                GhostPiece,
                Name::new(format!("ghost3d_{}", ghost.square)),
                Transform::from_translation(square_center(ghost.square))
                    .with_rotation(Quat::from_rotation_y(facing))
                    .with_scale(Vec3::splat(PIECE_SCALE)),
                Visibility::default(),
                VisibleIn::BOTH_3D,
            ))
            .with_children(|parent| {
                for (mesh, transform) in templates.ghost_parts(ghost.side, ghost.piece) {
                    parent.spawn((Mesh3d(mesh), MeshMaterial3d(material.clone()), transform));
                }
            });
        let color = if ghost.side == Side::White {
            Color::srgba(1.0, 1.0, 1.0, 0.55)
        } else {
            Color::srgba(0.0, 0.0, 0.0, 0.5)
        };
        let p = square_pos(ghost.square);
        commands.spawn((
            GhostPiece,
            Name::new(format!("ghost2d_{}", ghost.square)),
            Text2d::new(glyph(ghost.piece).to_string()),
            TextFont {
                font: font.0.clone().into(),
                font_size: FontSize::Px(84.0),
                ..default()
            },
            TextColor(color),
            Transform::from_xyz(p.x, p.y - 4.0, 1.5),
            Visibility::default(),
            VisibleIn::TECHNICAL,
        ));
    }
}
