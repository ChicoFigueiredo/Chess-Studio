//! Geometria do tabuleiro no mundo: casa -> posição e raio -> casa.
//!
//! Convenção do mundo: Y para cima; a superfície do tabuleiro é o plano `y = 0`,
//! centrado na origem. A coluna `a` fica em -X e `h` em +X; a fila 1 fica em +Z
//! (lado das brancas, perto da câmera da Vista padrão) e a fila 8 em -Z.
//! A convenção é lógica: girar a câmera nunca altera o mapeamento casa <-> mundo (CAM-03).

use crate::square::BoardSquare;
use glam::Vec3;

/// Lado de uma casa, em unidades de mundo.
pub const SQUARE_SIZE: f32 = 1.0;
/// Lado do tabuleiro (8 casas).
pub const BOARD_SIZE: f32 = 8.0 * SQUARE_SIZE;

/// Centro da casa, na superfície do tabuleiro (`y = 0`).
pub fn square_center(square: BoardSquare) -> Vec3 {
    let half = BOARD_SIZE / 2.0;
    Vec3::new(
        (f32::from(square.file()) + 0.5) * SQUARE_SIZE - half,
        0.0,
        half - (f32::from(square.rank()) + 0.5) * SQUARE_SIZE,
    )
}

/// Casa atingida pelo raio contra o plano do tabuleiro; `None` se o raio não o
/// intercepta à frente da origem ou atinge fora das 64 casas.
pub fn ray_to_square(origin: Vec3, dir: Vec3) -> Option<BoardSquare> {
    if !(origin.is_finite() && dir.is_finite()) || dir.y >= 0.0 || origin.y <= 0.0 {
        return None;
    }
    let t = -origin.y / dir.y;
    let hit = origin + dir * t;
    let half = BOARD_SIZE / 2.0;
    let file = ((hit.x + half) / SQUARE_SIZE).floor();
    let rank = ((half - hit.z) / SQUARE_SIZE).floor();
    if !(0.0..8.0).contains(&file) || !(0.0..8.0).contains(&rank) {
        return None;
    }
    BoardSquare::new(file as u8, rank as u8)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sq(name: &str) -> BoardSquare {
        BoardSquare::all().find(|s| s.to_string() == name).unwrap()
    }

    #[test]
    fn a1_and_h8_are_opposite_corners() {
        assert_eq!(square_center(sq("a1")), Vec3::new(-3.5, 0.0, 3.5));
        assert_eq!(square_center(sq("h8")), Vec3::new(3.5, 0.0, -3.5));
    }

    #[test]
    fn vertical_ray_over_each_center_returns_that_square() {
        for s in BoardSquare::all() {
            let c = square_center(s);
            assert_eq!(ray_to_square(c + Vec3::Y * 5.0, -Vec3::Y), Some(s));
        }
    }

    #[test]
    fn rays_outside_the_board_return_none() {
        let down = -Vec3::Y;
        assert_eq!(ray_to_square(Vec3::new(4.01, 5.0, 0.0), down), None);
        assert_eq!(ray_to_square(Vec3::new(-4.01, 5.0, 0.0), down), None);
        assert_eq!(ray_to_square(Vec3::new(0.0, 5.0, 4.01), down), None);
        assert_eq!(ray_to_square(Vec3::new(0.0, 5.0, -4.01), down), None);
        assert_eq!(ray_to_square(Vec3::new(f32::NAN, 5.0, 0.0), down), None);
    }

    #[test]
    fn rays_not_pointing_at_the_plane_return_none() {
        let o = Vec3::new(0.0, 5.0, 0.0);
        assert_eq!(ray_to_square(o, Vec3::Y), None); // para cima
        assert_eq!(ray_to_square(o, Vec3::X), None); // paralelo
        assert_eq!(ray_to_square(Vec3::new(0.0, -1.0, 0.0), -Vec3::Y), None); // origem abaixo
    }
}
