//! Geometria do tabuleiro no mundo: casa -> posição e raio -> casa.
//!
//! Convenção do mundo: Y para cima; a superfície do tabuleiro é o plano `y = 0`,
//! centrado na origem. A coluna `a` fica em -X e `h` em +X; a fila 1 fica em +Z
//! (lado das brancas, perto da câmera da Vista padrão) e a fila 8 em -Z.
//! A convenção é lógica: girar a câmera nunca altera o mapeamento casa <-> mundo (CAM-03).

use chess_core::{Piece, Square};
use glam::Vec3;

/// Lado de uma casa, em unidades de mundo.
pub const SQUARE_SIZE: f32 = 1.0;
/// Lado do tabuleiro (8 casas).
pub const BOARD_SIZE: f32 = 8.0 * SQUARE_SIZE;

/// As 64 casas, de a1 a h8 (fila a fila).
pub fn all_squares() -> impl Iterator<Item = Square> {
    (0..8u8).flat_map(|rank| (0..8u8).filter_map(move |file| Square::new(file, rank)))
}

/// Centro da casa, na superfície do tabuleiro (`y = 0`).
pub fn square_center(square: Square) -> Vec3 {
    let half = BOARD_SIZE / 2.0;
    Vec3::new(
        (f32::from(square.file()) + 0.5) * SQUARE_SIZE - half,
        0.0,
        half - (f32::from(square.rank()) + 0.5) * SQUARE_SIZE,
    )
}

/// Ponto em que o raio encontra o plano do tabuleiro (`y = 0`), mesmo fora das 64 casas
/// (serve para desenhar a peça arrastada para além da borda). `None` se o raio não o
/// intercepta à frente da origem.
pub fn ray_to_board_plane(origin: Vec3, dir: Vec3) -> Option<Vec3> {
    if !(origin.is_finite() && dir.is_finite()) || dir.y >= 0.0 || origin.y <= 0.0 {
        return None;
    }
    let t = -origin.y / dir.y;
    let hit = origin + dir * t;
    Some(Vec3::new(hit.x, 0.0, hit.z))
}

/// Casa de um ponto do plano do tabuleiro; `None` fora das 64 casas.
pub fn point_to_square(point: Vec3) -> Option<Square> {
    let half = BOARD_SIZE / 2.0;
    let file = ((point.x + half) / SQUARE_SIZE).floor();
    let rank = ((half - point.z) / SQUARE_SIZE).floor();
    if !(0.0..8.0).contains(&file) || !(0.0..8.0).contains(&rank) {
        return None;
    }
    Square::new(file as u8, rank as u8)
}

/// Casa atingida pelo raio contra o plano do tabuleiro; `None` se o raio não o
/// intercepta à frente da origem ou atinge fora das 64 casas.
pub fn ray_to_square(origin: Vec3, dir: Vec3) -> Option<Square> {
    ray_to_board_plane(origin, dir).and_then(point_to_square)
}

/// Raio (em unidades de casa) do cilindro que representa o corpo de uma peça no picking.
pub const PIECE_PICK_RADIUS: f32 = 0.35;

/// Altura aproximada do corpo da peça (unidades de casa), para o picking.
pub fn piece_pick_height(piece: Piece) -> f32 {
    match piece {
        Piece::Pawn => 0.8,
        Piece::Rook => 0.95,
        Piece::Knight => 1.0,
        Piece::Bishop => 1.15,
        Piece::Queen => 1.35,
        Piece::King => 1.5,
    }
}

/// Distância ao longo do raio até o corpo da peça (cilindro vertical sobre o centro da
/// casa, de `y = 0` à altura da peça); `None` se o raio não o atinge.
fn ray_to_piece_body(origin: Vec3, dir: Vec3, square: Square, piece: Piece) -> Option<f32> {
    let center = square_center(square);
    let height = piece_pick_height(piece);
    let (dx, dz) = (dir.x, dir.z);
    let (ox, oz) = (origin.x - center.x, origin.z - center.z);
    let inside = |t: f32| {
        let (x, z) = (ox + dx * t, oz + dz * t);
        x * x + z * z <= PIECE_PICK_RADIUS * PIECE_PICK_RADIUS
    };
    let mut best: Option<f32> = None;
    let mut consider = |t: f32| {
        if t >= 0.0 && best.is_none_or(|b| t < b) {
            best = Some(t);
        }
    };
    // Tampa de cima.
    if dir.y != 0.0 {
        let t = (height - origin.y) / dir.y;
        if inside(t) {
            consider(t);
        }
    }
    // Lateral.
    let a = dx * dx + dz * dz;
    if a > 1e-9 {
        let b = 2.0 * (ox * dx + oz * dz);
        let c = ox * ox + oz * oz - PIECE_PICK_RADIUS * PIECE_PICK_RADIUS;
        let disc = b * b - 4.0 * a * c;
        if disc >= 0.0 {
            let root = disc.sqrt();
            for t in [(-b - root) / (2.0 * a), (-b + root) / (2.0 * a)] {
                let y = origin.y + dir.y * t;
                if (0.0..=height).contains(&y) {
                    consider(t);
                }
            }
        }
    }
    best
}

/// Casa sob o raio **levando em conta o corpo das peças**: uma peça alta vista de lado
/// cobre casas atrás dela, e quem clica no corpo da peça quer a casa dela, não a do chão
/// atrás. Vale a peça mais próxima atingida; sem peça no caminho, a casa do plano.
pub fn ray_to_square_with_pieces(
    origin: Vec3,
    dir: Vec3,
    pieces: impl IntoIterator<Item = (Square, Piece)>,
) -> Option<Square> {
    if !(origin.is_finite() && dir.is_finite()) {
        return None;
    }
    let floor = ray_to_board_plane(origin, dir);
    let floor_t = floor.map(|p| (p - origin).length());
    pieces
        .into_iter()
        .filter_map(|(sq, piece)| Some((sq, ray_to_piece_body(origin, dir, sq, piece)?)))
        .filter(|(_, t)| {
            let len = dir.length();
            floor_t.is_none_or(|ft| t * len <= ft)
        })
        .min_by(|a, b| a.1.total_cmp(&b.1))
        .map(|(sq, _)| sq)
        .or_else(|| floor.and_then(point_to_square))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sq(name: &str) -> Square {
        name.parse().unwrap()
    }

    #[test]
    fn all_squares_cover_a1_to_h8() {
        let names: Vec<String> = all_squares().map(|s| s.to_string()).collect();
        assert_eq!(names.len(), 64);
        assert_eq!(
            (names[0].as_str(), names[7].as_str(), names[63].as_str()),
            ("a1", "h1", "h8")
        );
    }

    #[test]
    fn a1_and_h8_are_opposite_corners() {
        assert_eq!(square_center(sq("a1")), Vec3::new(-3.5, 0.0, 3.5));
        assert_eq!(square_center(sq("h8")), Vec3::new(3.5, 0.0, -3.5));
    }

    #[test]
    fn vertical_ray_over_each_center_returns_that_square() {
        for s in all_squares() {
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

    #[test]
    fn plane_hit_exists_outside_the_board_but_has_no_square() {
        let hit = ray_to_board_plane(Vec3::new(9.0, 5.0, 2.0), -Vec3::Y).unwrap();
        assert_eq!(hit, Vec3::new(9.0, 0.0, 2.0));
        assert_eq!(point_to_square(hit), None);
        assert_eq!(point_to_square(Vec3::new(-3.9, 0.0, 3.9)), Some(sq("a1")));
    }

    /// Câmera oblíqua: olha para o centro do tabuleiro a partir do lado das brancas.
    fn oblique_ray_through(target: Vec3) -> (Vec3, Vec3) {
        let eye = Vec3::new(0.0, 7.0, 9.0);
        (eye, (target - eye).normalize())
    }

    #[test]
    fn clicking_the_upper_body_of_a_tall_piece_picks_its_square_not_the_floor_behind() {
        // Peão em e2; o ponto 0,7 acima do centro dele, visto da câmera oblíqua, cai no
        // plano em e3 ou além: o picking só pelo plano erraria a casa.
        let target = square_center(sq("e2")) + Vec3::Y * 0.7;
        let (origin, dir) = oblique_ray_through(target);
        assert_ne!(ray_to_square(origin, dir), Some(sq("e2")));
        let pieces = [(sq("e2"), Piece::Pawn)];
        assert_eq!(
            ray_to_square_with_pieces(origin, dir, pieces),
            Some(sq("e2"))
        );
    }

    #[test]
    fn without_a_piece_in_the_way_the_floor_square_is_picked() {
        let (origin, dir) = oblique_ray_through(square_center(sq("e4")));
        let pieces = [(sq("e2"), Piece::Pawn), (sq("a1"), Piece::Rook)];
        assert_eq!(
            ray_to_square_with_pieces(origin, dir, pieces),
            Some(sq("e4"))
        );
    }

    #[test]
    fn the_nearest_piece_wins_when_two_overlap_on_the_ray() {
        let (origin, dir) = oblique_ray_through(square_center(sq("e2")) + Vec3::Y * 0.7);
        let pieces = [(sq("e2"), Piece::Pawn), (sq("e3"), Piece::King)];
        // Pela câmera, e2 está mais perto que e3 e é atingida primeiro.
        assert_eq!(
            ray_to_square_with_pieces(origin, dir, pieces),
            Some(sq("e2"))
        );
    }

    #[test]
    fn a_ray_over_a_piece_that_hits_the_floor_in_front_of_it_picks_the_floor() {
        // Para a câmera das brancas, e3 fica atrás de e2: o raio toca o chão em e2 antes
        // de alcançar o corpo da torre em e3.
        let (origin, dir) = oblique_ray_through(square_center(sq("e2")));
        let pieces = [(sq("e3"), Piece::Rook)];
        assert_eq!(
            ray_to_square_with_pieces(origin, dir, pieces),
            Some(sq("e2"))
        );
    }
}
