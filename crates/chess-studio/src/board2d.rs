//! 2D técnico: tabuleiro plano visto de cima (câmera ortográfica), coordenadas a–h/1–8 e
//! peças como glifos Unicode de xadrez (Noto Sans Symbols 2, OFL; ver `assets/LICENSES.md`).
//!
//! Mundo 2D: 1 casa = `UNIT` (100) unidades, x para a direita, y para cima. A conversão para
//! o mundo 3D (que `chess_input` usa) é `x3d = x / UNIT`, `z3d = -y / UNIT`.

use crate::picking::{HoveredSquare, SelectedSquare};
use crate::presentation::{TechnicalCamera, VisibleIn};
use bevy::camera::ScalingMode;
use bevy::prelude::*;
use chess_core::Square;
use chess_input::{BOARD_SIZE, all_squares, square_center};

/// Unidades do mundo 2D por casa.
pub const UNIT: f32 = 100.0;

/// Mundo 2D -> ponto no plano do tabuleiro (coordenadas do `chess_input`).
pub fn to_board_plane(p: Vec2) -> Vec3 {
    Vec3::new(p.x / UNIT, 0.0, -p.y / UNIT)
}

pub fn square_pos(sq: Square) -> Vec2 {
    let c = square_center(sq);
    Vec2::new(c.x * UNIT, -c.z * UNIT)
}

// Tons de madeira neutros, coerentes com o tabuleiro 3D (bordo claro e nogueira escura).
pub(crate) const LIGHT: Color = Color::srgb(0.87, 0.77, 0.62);
pub(crate) const DARK: Color = Color::srgb(0.55, 0.40, 0.29);
const LABEL: Color = Color::srgb(0.84, 0.80, 0.74);

#[derive(Component)]
struct HoverSprite;
#[derive(Component)]
struct ClickSprite;

pub struct Board2dPlugin;

impl Plugin for Board2dPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Startup, (spawn_camera, spawn_board))
            .add_systems(Update, update_markers.after(crate::picking::PickSet));
    }
}

/// Lado mínimo visível da câmera 2D (tabuleiro + margem das coordenadas).
const VIEW_SPAN: f32 = (BOARD_SIZE + 2.2) * UNIT;

/// Posição do cursor (pixels, origem no canto superior esquerdo) -> mundo 2D, para a câmera
/// ortográfica `AutoMin` centrada na origem. Evita depender do estado de uma câmera inativa.
pub fn cursor_to_world(cursor: Vec2, window: Vec2) -> Vec2 {
    let scale = VIEW_SPAN / window.min_element().max(1.0);
    Vec2::new(
        (cursor.x - window.x / 2.0) * scale,
        -(cursor.y - window.y / 2.0) * scale,
    )
}

pub(crate) fn spawn_camera(mut commands: Commands) {
    let half = VIEW_SPAN;
    commands.spawn((
        Camera2d,
        TechnicalCamera,
        Camera {
            is_active: false,
            ..default()
        },
        Projection::Orthographic(OrthographicProjection {
            scaling_mode: ScalingMode::AutoMin {
                min_width: half,
                min_height: half,
            },
            ..OrthographicProjection::default_2d()
        }),
        // MSAA igual ao das outras câmeras da janela: `presentation::apply_quality`.
    ));
}

fn spawn_board(mut commands: Commands) {
    let size = Vec2::splat(UNIT);
    for sq in all_squares() {
        let dark = (sq.file() + sq.rank()) % 2 == 0;
        let p = square_pos(sq);
        commands.spawn((
            Sprite::from_color(if dark { DARK } else { LIGHT }, size),
            Transform::from_xyz(p.x, p.y, 0.0),
            VisibleIn::TECHNICAL,
        ));
    }

    // Coordenadas: letras abaixo da fila 1, números à esquerda da coluna a.
    let edge = (BOARD_SIZE / 2.0 + 0.42) * UNIT;
    for i in 0..8u8 {
        let along = (f32::from(i) + 0.5 - BOARD_SIZE / 2.0) * UNIT;
        let file = char::from(b'a' + i).to_string();
        let rank = char::from(b'1' + i).to_string();
        for (text, pos) in [
            (file, Vec2::new(along, -edge)),
            (rank, Vec2::new(-edge, along)),
        ] {
            commands.spawn((
                Text2d::new(text),
                TextFont {
                    font_size: FontSize::Px(38.0),
                    ..default()
                },
                TextColor(LABEL),
                Transform::from_xyz(pos.x, pos.y, 1.0),
                VisibleIn::TECHNICAL,
            ));
        }
    }

    for (is_hover, color, z) in [
        (true, Color::srgba(0.2, 0.6, 1.0, 0.40), 0.4),
        (false, Color::srgba(1.0, 0.8, 0.1, 0.50), 0.5),
    ] {
        let mut e = commands.spawn((
            Sprite::from_color(color, size),
            Transform::from_xyz(0.0, 0.0, z),
            Visibility::Hidden,
            VisibleIn::TECHNICAL,
        ));
        if is_hover {
            e.insert(HoverSprite);
        } else {
            e.insert(ClickSprite);
        }
    }
}

#[allow(clippy::type_complexity)]
fn update_markers(
    hovered: Res<HoveredSquare>,
    aids: Res<crate::aids::AidSettings>,
    clicked: Res<SelectedSquare>,
    mut sprites: ParamSet<(
        Single<(&mut Transform, &mut Visibility), With<HoverSprite>>,
        Single<(&mut Transform, &mut Visibility), With<ClickSprite>>,
    )>,
) {
    let (mut t, mut v) = sprites.p0().into_inner();
    place(&mut t, &mut v, hovered.0.filter(|_| aids.selected));
    let (mut t, mut v) = sprites.p1().into_inner();
    place(&mut t, &mut v, clicked.0);
}

fn place(transform: &mut Transform, visibility: &mut Visibility, square: Option<Square>) {
    match square {
        Some(sq) => {
            let p = square_pos(sq);
            transform.translation.x = p.x;
            transform.translation.y = p.y;
            *visibility = Visibility::Visible;
        }
        None => *visibility = Visibility::Hidden,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chess_input::ray_to_square;

    fn square_at(cursor: Vec2, window: Vec2) -> Option<String> {
        let p = to_board_plane(cursor_to_world(cursor, window));
        ray_to_square(p + Vec3::Y, -Vec3::Y).map(|s| s.to_string())
    }

    #[test]
    fn window_center_is_between_d_and_e_files() {
        let w = Vec2::new(1280.0, 720.0);
        // Centro: cruzamento d4/e4/d5/e5; um pouco à direita e abaixo = e4.
        assert_eq!(square_at(Vec2::new(641.0, 361.0), w).as_deref(), Some("e4"));
        assert_eq!(square_at(Vec2::new(639.0, 359.0), w).as_deref(), Some("d5"));
    }

    #[test]
    fn corners_map_to_a1_and_h8() {
        let w = Vec2::new(800.0, 800.0);
        let step = 800.0 / (BOARD_SIZE + 2.2);
        let edge = (800.0 - 8.0 * step) / 2.0;
        assert_eq!(
            square_at(Vec2::new(edge + 2.0, 800.0 - edge - 2.0), w).as_deref(),
            Some("a1")
        );
        assert_eq!(
            square_at(Vec2::new(800.0 - edge - 2.0, edge + 2.0), w).as_deref(),
            Some("h8")
        );
        assert_eq!(square_at(Vec2::new(5.0, 5.0), w), None);
    }
}
