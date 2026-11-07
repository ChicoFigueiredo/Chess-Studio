//! Geometria pura das setas do HUD: o contorno de uma seta entre duas casas, no plano do
//! tabuleiro (`y = 0`). Como depende só das casas, a seta fica presa a elas em qualquer
//! órbita, inversão ou zoom: a câmera nunca entra na conta (UI-06).

use crate::board::square_center;
use chess_core::Square;
use glam::Vec3;

/// Meia-largura da haste, em casas.
pub const SHAFT_HALF_WIDTH: f32 = 0.08;
/// Meia-largura da cabeça.
pub const HEAD_HALF_WIDTH: f32 = 0.22;
/// Comprimento da cabeça.
pub const HEAD_LENGTH: f32 = 0.34;
/// A seta começa e termina a esta distância dos centros, para não cobrir a peça inteira.
pub const END_MARGIN: f32 = 0.12;

/// Triângulos (índices em [`arrow_outline`]) que preenchem a seta: haste e cabeça.
pub const ARROW_TRIANGLES: [[u32; 3]; 3] = [[0, 1, 5], [0, 5, 6], [2, 3, 4]];

/// Contorno da seta de `from` para `to`: sete pontos (cauda esquerda, fim da haste à
/// esquerda, base da cabeça à esquerda, ponta, base da cabeça à direita, fim da haste à
/// direita, cauda direita). Vazio se as casas coincidem ou o trecho é curto demais.
pub fn arrow_outline(from: Square, to: Square) -> Vec<Vec3> {
    let (a, b) = (square_center(from), square_center(to));
    let delta = b - a;
    let length = delta.length();
    if length <= 2.0 * END_MARGIN + HEAD_LENGTH {
        return Vec::new();
    }
    let dir = delta / length;
    // Perpendicular no plano do tabuleiro (gira 90 graus em torno de Y).
    let side = Vec3::new(dir.z, 0.0, -dir.x);
    let tail = a + dir * END_MARGIN;
    let tip = b - dir * END_MARGIN;
    let neck = tip - dir * HEAD_LENGTH;
    vec![
        tail + side * SHAFT_HALF_WIDTH,
        neck + side * SHAFT_HALF_WIDTH,
        neck + side * HEAD_HALF_WIDTH,
        tip,
        neck - side * HEAD_HALF_WIDTH,
        neck - side * SHAFT_HALF_WIDTH,
        tail - side * SHAFT_HALF_WIDTH,
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sq(name: &str) -> Square {
        name.parse().unwrap()
    }

    fn area(points: &[Vec3]) -> f32 {
        ARROW_TRIANGLES
            .iter()
            .map(|t| {
                let [p, q, r] = t.map(|i| points[i as usize]);
                (q - p).cross(r - p).length() / 2.0
            })
            .sum()
    }

    #[test]
    fn tip_lies_just_before_the_target_center_on_the_line() {
        let o = arrow_outline(sq("a1"), sq("a8"));
        assert_eq!(o.len(), 7);
        let tip = o[3];
        let target = square_center(sq("a8"));
        assert!((tip.x - target.x).abs() < 1e-5 && tip.y == 0.0);
        assert!((tip.z - (target.z + END_MARGIN)).abs() < 1e-5);
    }

    #[test]
    fn outline_is_symmetric_around_the_axis() {
        let o = arrow_outline(sq("c3"), sq("f6"));
        let (a, b) = (square_center(sq("c3")), square_center(sq("f6")));
        let axis_tail = a + (b - a).normalize() * END_MARGIN;
        assert!(((o[0] + o[6]) / 2.0 - axis_tail).length() < 1e-5);
        assert!(((o[2] + o[4]) / 2.0 - (o[1] + o[5]) / 2.0).length() < 1e-5);
    }

    #[test]
    fn fills_shaft_and_head_area() {
        let o = arrow_outline(sq("a1"), sq("a8"));
        let length = (square_center(sq("a1")) - square_center(sq("a8"))).length();
        let shaft_len = length - 2.0 * END_MARGIN - HEAD_LENGTH;
        let expected = shaft_len * 2.0 * SHAFT_HALF_WIDTH + HEAD_LENGTH * HEAD_HALF_WIDTH;
        assert!(
            (area(&o) - expected).abs() < 1e-4,
            "{} vs {expected}",
            area(&o)
        );
    }

    #[test]
    fn same_square_or_too_short_has_no_arrow() {
        assert!(arrow_outline(sq("e4"), sq("e4")).is_empty());
    }

    #[test]
    fn knight_move_and_reversed_direction_work() {
        assert_eq!(arrow_outline(sq("g1"), sq("f3")).len(), 7);
        let back = arrow_outline(sq("f3"), sq("g1"));
        assert_eq!(back.len(), 7);
        assert!((back[3] - square_center(sq("g1"))).length() < 0.2);
    }
}
