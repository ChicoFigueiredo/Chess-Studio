//! Trajetória de soltura e encaixe da peça (INT-05).
//!
//! O destino é sempre uma casa **canônica** (a que a Partida decidiu); o ponto de partida
//! é só onde a peça estava na tela (cursor ao soltar, ou centro da origem). A animação
//! nunca decide para onde a peça vai: `t >= 1` devolve exatamente o centro da casa.

use crate::board::square_center;
use chess_core::Square;
use glam::Vec3;

/// Altura da peça levantada/arrastada acima do tabuleiro.
pub const DRAG_HEIGHT: f32 = 0.7;
/// Duração do encaixe, em segundos (curta: nunca atrasa o controle).
pub const SETTLE_SECS: f32 = 0.18;
/// Altura do arco extra de um lance sem arrasto (a peça "voa" até o destino).
const ARC_HEIGHT: f32 = 0.35;

fn ease_out_cubic(t: f32) -> f32 {
    1.0 - (1.0 - t).powi(3)
}

/// Posição da peça em `t` (0..1 do tempo decorrido sobre [`SETTLE_SECS`]) indo de `start`
/// ao centro de `target`. Em `t >= 1` (ou NaN tratado como fim) é exatamente o centro.
pub fn settle_position(start: Vec3, target: Square, t: f32) -> Vec3 {
    let end = square_center(target);
    if t.is_nan() || t >= 1.0 || !start.is_finite() {
        return end;
    }
    let t = t.max(0.0);
    let e = ease_out_cubic(t);
    let flat = Vec3::new(start.x, 0.0, start.z).distance(end);
    let arc = if flat > 0.05 {
        ARC_HEIGHT * (std::f32::consts::PI * t).sin()
    } else {
        0.0
    };
    Vec3::new(
        start.x + (end.x - start.x) * e,
        (start.y * (1.0 - e) + arc).max(0.0),
        start.z + (end.z - start.z) * e,
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::board::all_squares;

    #[test]
    fn ends_exactly_on_the_canonical_square_from_any_start() {
        let starts = [
            Vec3::new(0.3, DRAG_HEIGHT, -2.2),
            Vec3::new(-9.0, DRAG_HEIGHT, 7.0), // arrastada para fora do tabuleiro
            Vec3::ZERO,
        ];
        for target in all_squares() {
            for start in starts {
                assert_eq!(settle_position(start, target, 1.0), square_center(target));
                assert_eq!(settle_position(start, target, 5.0), square_center(target));
                assert_eq!(
                    settle_position(start, target, f32::NAN),
                    square_center(target)
                );
            }
        }
    }

    #[test]
    fn starts_where_the_piece_was() {
        let start = Vec3::new(1.2, DRAG_HEIGHT, 0.4);
        let target: Square = "e4".parse().unwrap();
        assert_eq!(settle_position(start, target, 0.0), start);
    }

    #[test]
    fn approaches_the_target_monotonically_and_stays_above_the_board() {
        let start = Vec3::new(-3.0, DRAG_HEIGHT, 3.0);
        let target: Square = "g7".parse().unwrap();
        let end = square_center(target);
        let mut last = f32::MAX;
        for step in 0..=100 {
            let p = settle_position(start, target, step as f32 / 100.0);
            let d = Vec3::new(p.x - end.x, 0.0, p.z - end.z).length();
            assert!(d <= last + 1e-5, "afastou-se do destino em {step}");
            assert!(p.y >= 0.0);
            last = d;
        }
        assert_eq!(last, 0.0);
    }
}
