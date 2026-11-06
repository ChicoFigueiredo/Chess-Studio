//! Auxílio visual do Assistido: marcadores nos destinos legais da peça selecionada ou
//! arrastada (`Snapshot::legal_destinations`), em 3D e no 2D técnico. Só leitura da Partida.

use crate::board2d::{UNIT, square_pos};
use crate::play::PlaySet;
use crate::presentation::VisibleIn;
use crate::session::Session;
use bevy::prelude::*;
use chess_core::Square;
use chess_input::square_center;

/// Máximo de destinos legais de uma peça (dama no centro: 27).
const MAX_DESTINATIONS: usize = 28;

#[derive(Component)]
struct Dest3d(usize);
#[derive(Component)]
struct Dest2d(usize);

pub struct HighlightPlugin;

impl Plugin for HighlightPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Startup, spawn_markers)
            .add_systems(Update, update_markers.after(PlaySet));
    }
}

fn spawn_markers(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    let mesh = meshes.add(Cylinder::new(0.17, 0.02));
    let material = materials.add(StandardMaterial {
        base_color: Color::srgba(0.15, 0.75, 0.35, 0.7),
        unlit: true,
        alpha_mode: AlphaMode::Blend,
        ..default()
    });
    for i in 0..MAX_DESTINATIONS {
        commands.spawn((
            Dest3d(i),
            Mesh3d(mesh.clone()),
            MeshMaterial3d(material.clone()),
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
}

#[allow(clippy::type_complexity)]
fn update_markers(
    session: Res<Session>,
    mut cache: Local<(Option<Square>, Option<u64>, Vec<Square>)>,
    mut markers: ParamSet<(
        Query<(&Dest3d, &mut Transform, &mut Visibility)>,
        Query<(&Dest2d, &mut Transform, &mut Visibility), Without<Dest3d>>,
    )>,
) {
    let origin = session.gesture_visual().origin;
    let key = (origin, Some(session.revision()));
    if (cache.0, cache.1) != key {
        cache.2 = origin.map_or_else(Vec::new, |o| session.snapshot().legal_destinations(o));
        (cache.0, cache.1) = key;
    }
    let dests = &cache.2;
    for (marker, mut transform, mut visibility) in &mut markers.p0() {
        match dests.get(marker.0) {
            Some(sq) => {
                let c = square_center(*sq);
                transform.translation = Vec3::new(c.x, 0.015, c.z);
                *visibility = Visibility::Visible;
            }
            None => *visibility = Visibility::Hidden,
        }
    }
    for (marker, mut transform, mut visibility) in &mut markers.p1() {
        match dests.get(marker.0) {
            Some(sq) => {
                let p = square_pos(*sq);
                transform.translation = Vec3::new(p.x, p.y, 0.6);
                *visibility = Visibility::Visible;
            }
            None => *visibility = Visibility::Hidden,
        }
    }
}
