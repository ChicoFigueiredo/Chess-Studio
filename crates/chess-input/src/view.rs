//! Vistas (enquadramentos de câmera) e operações puras de órbita/pan/zoom com limites.

use crate::board::BOARD_SIZE;
use glam::Vec3;

/// Vista nomeada (teclas 1..4). Ver GLOSSARY.md.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum CameraView {
    /// Vista superior: de cima, brancas embaixo na tela.
    Top,
    /// Vista oblíqua: do lado das brancas, girada e inclinada.
    Oblique,
    /// Vista padrão: do lado das brancas, inclinação média.
    Standard,
    /// Vista invertida: do lado das pretas.
    Inverted,
}

impl CameraView {
    pub const ALL: [CameraView; 4] = [Self::Top, Self::Oblique, Self::Standard, Self::Inverted];

    /// Nome em pt-BR para a interface.
    pub fn label(self) -> &'static str {
        match self {
            Self::Top => "Superior",
            Self::Oblique => "Oblíqua",
            Self::Standard => "Padrão",
            Self::Inverted => "Invertida",
        }
    }

    /// Pose de câmera da Vista.
    pub fn pose(self) -> CameraPose {
        let (yaw, pitch, distance) = match self {
            Self::Top => (0.0, CameraPose::MAX_PITCH, 13.0),
            Self::Oblique => (35f32.to_radians(), 38f32.to_radians(), 12.0),
            Self::Standard => (0.0, 55f32.to_radians(), 12.0),
            Self::Inverted => (180f32.to_radians(), 55f32.to_radians(), 12.0),
        };
        CameraPose {
            target: Vec3::ZERO,
            yaw,
            pitch,
            distance,
        }
    }
}

/// Câmera orbital: ponto-alvo sobre a mesa, guinada, inclinação e distância.
///
/// `yaw = 0` põe a câmera em +Z (lado das brancas); `yaw = PI` em -Z.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct CameraPose {
    pub target: Vec3,
    /// Rotação em torno de Y, radianos (livre: 360°).
    pub yaw: f32,
    /// Elevação sobre a mesa, radianos, em `MIN_PITCH..=MAX_PITCH`.
    pub pitch: f32,
    pub distance: f32,
}

impl CameraPose {
    /// Elevação mínima: a câmera nunca desce até a mesa.
    pub const MIN_PITCH: f32 = 0.14; // ~8°
    /// Elevação máxima: quase vertical, sem degenerar o vetor "para cima".
    pub const MAX_PITCH: f32 = 1.55; // ~88,8°
    pub const MIN_DISTANCE: f32 = 4.0;
    pub const MAX_DISTANCE: f32 = 30.0;
    /// O alvo nunca sai desta distância do centro, em X e Z (sempre vê o tabuleiro).
    pub const MAX_TARGET_OFFSET: f32 = BOARD_SIZE / 2.0;

    /// Posição da câmera no mundo.
    pub fn position(&self) -> Vec3 {
        let (sy, cy) = self.yaw.sin_cos();
        let (sp, cp) = self.pitch.sin_cos();
        self.target + Vec3::new(sy * cp, sp, cy * cp) * self.distance
    }

    /// Direção horizontal "para a direita" na tela.
    pub fn right(&self) -> Vec3 {
        Vec3::new(self.yaw.cos(), 0.0, -self.yaw.sin())
    }

    /// Direção horizontal "para frente" (da câmera em direção ao alvo, no plano da mesa).
    pub fn forward(&self) -> Vec3 {
        Vec3::new(-self.yaw.sin(), 0.0, -self.yaw.cos())
    }

    /// Aplica os limites (pitch, distância e alvo sobre a mesa).
    pub fn clamped(mut self) -> Self {
        self.pitch = self.pitch.clamp(Self::MIN_PITCH, Self::MAX_PITCH);
        self.distance = self.distance.clamp(Self::MIN_DISTANCE, Self::MAX_DISTANCE);
        self.target.y = 0.0;
        self.target.x = self
            .target
            .x
            .clamp(-Self::MAX_TARGET_OFFSET, Self::MAX_TARGET_OFFSET);
        self.target.z = self
            .target
            .z
            .clamp(-Self::MAX_TARGET_OFFSET, Self::MAX_TARGET_OFFSET);
        self
    }

    /// Órbita: yaw sem limite (360°), pitch limitado.
    pub fn orbit(self, d_yaw: f32, d_pitch: f32) -> Self {
        Self {
            yaw: (self.yaw + d_yaw).rem_euclid(std::f32::consts::TAU),
            pitch: self.pitch + d_pitch,
            ..self
        }
        .clamped()
    }

    /// Pan no plano da mesa, relativo à tela
    /// (`right` positivo move o alvo para a direita; `up`, para longe da câmera).
    pub fn pan(self, right: f32, up: f32) -> Self {
        Self {
            target: self.target + self.right() * right + self.forward() * up,
            ..self
        }
        .clamped()
    }

    /// Zoom multiplicativo: `factor < 1` aproxima, `> 1` afasta.
    pub fn zoom(self, factor: f32) -> Self {
        Self {
            distance: self.distance * factor,
            ..self
        }
        .clamped()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::board::{ray_to_square, square_center};
    use crate::square::BoardSquare;

    /// Raio da câmera da pose até um ponto da mesa.
    fn ray_to(pose: &CameraPose, point: Vec3) -> Option<BoardSquare> {
        let o = pose.position();
        ray_to_square(o, point - o)
    }

    #[test]
    fn every_square_is_pickable_in_every_view() {
        for view in CameraView::ALL {
            let pose = view.pose();
            for s in BoardSquare::all() {
                assert_eq!(ray_to(&pose, square_center(s)), Some(s), "{view:?} {s}");
                // Dentro da casa, longe do centro, ainda é a mesma casa.
                for (dx, dz) in [(0.4, 0.4), (-0.4, 0.4), (0.4, -0.4), (-0.4, -0.4)] {
                    let p = square_center(s) + Vec3::new(dx, 0.0, dz);
                    assert_eq!(ray_to(&pose, p), Some(s), "{view:?} {s} off-center");
                }
            }
        }
    }

    #[test]
    fn every_square_is_pickable_after_any_rotation() {
        for step in 0..72 {
            for d_pitch in [-0.7, -0.3, 0.0, 0.5] {
                let pose = CameraView::Standard
                    .pose()
                    .orbit(step as f32 * 5f32.to_radians(), d_pitch);
                for s in BoardSquare::all() {
                    assert_eq!(ray_to(&pose, square_center(s)), Some(s));
                }
            }
        }
    }

    #[test]
    fn inverting_the_view_does_not_change_logical_interpretation() {
        // CAM-03: a casa sob um ponto do mundo é a mesma, qualquer que seja a Vista.
        let e4 = square_center(BoardSquare::new(4, 3).unwrap());
        for view in CameraView::ALL {
            assert_eq!(ray_to(&view.pose(), e4).unwrap().to_string(), "e4");
        }
    }

    #[test]
    fn standard_view_looks_from_white_side_and_inverted_from_black_side() {
        assert!(CameraView::Standard.pose().position().z > 0.0);
        assert!(CameraView::Inverted.pose().position().z < 0.0);
        assert!(CameraView::Top.pose().position().y > 10.0);
    }

    #[test]
    fn orbit_wraps_yaw_and_clamps_pitch() {
        let p = CameraView::Standard.pose().orbit(7.0, 10.0);
        assert!((0.0..std::f32::consts::TAU).contains(&p.yaw));
        assert_eq!(p.pitch, CameraPose::MAX_PITCH);
        let p = p.orbit(0.0, -10.0);
        assert_eq!(p.pitch, CameraPose::MIN_PITCH);
        assert!(p.position().y > 0.0);
    }

    #[test]
    fn zoom_is_limited() {
        let p = CameraView::Standard.pose();
        assert_eq!(p.zoom(0.0001).distance, CameraPose::MIN_DISTANCE);
        assert_eq!(p.zoom(1000.0).distance, CameraPose::MAX_DISTANCE);
    }

    #[test]
    fn pan_moves_with_screen_axes_and_never_loses_the_board() {
        let p = CameraView::Standard.pose().pan(1.0, 0.0);
        assert!((p.target - Vec3::X).length() < 1e-5);
        let p = CameraView::Standard.pose().pan(0.0, 1.0);
        assert!((p.target - Vec3::NEG_Z).length() < 1e-5); // para longe da câmera
        let far = CameraView::Inverted.pose().pan(500.0, 500.0);
        assert!(far.target.x.abs() <= CameraPose::MAX_TARGET_OFFSET);
        assert!(far.target.z.abs() <= CameraPose::MAX_TARGET_OFFSET);
        assert!(far.position().y > 0.0);
    }
}
