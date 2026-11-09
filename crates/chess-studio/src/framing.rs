//! Enquadramento das Vistas: o tabuleiro (com moldura e peças) cabe inteiro na área útil, que
//! muda com o dock aberto, sem dock e no modo Foco, e o alvo da câmera é sempre o centro do
//! tabuleiro (#135). O relógio 3D fica ao lado e não entra na conta: ele só ajusta a distância
//! se estivesse no enquadramento, e deslocaria o centro. A matemática é
//! `chess_input::fit_distance` (pura e testada); aqui se montam os pontos da cena e se
//! reaplica o enquadramento quando a Vista ou a largura do viewport mudam.

use crate::camera::CameraRig;
use crate::dock::Dock;
use crate::session::Session;
use bevy::prelude::*;
use bevy::window::PrimaryWindow;
use chess_core::Color as Side;
use chess_input::{BOARD_SIZE, CameraPose, CameraView, fit_distance};

/// Meia-largura da moldura do tabuleiro (tabuleiro 8 + borda), igual à de `board`.
const FRAME_HALF: f32 = BOARD_SIZE / 2.0 + 0.6;
/// Altura que as peças alcançam acima do tabuleiro (o rei passa de 1,7).
const PIECES_TOP: f32 = 2.0;
/// Base da mesa, abaixo da moldura.
const TABLE_LEVEL: f32 = -0.42;
/// Fração do campo de visão que a cena pode ocupar; o resto é respiro.
const FILL: f32 = 0.94;

/// Pontos que a câmera precisa enxergar: a caixa do tabuleiro com moldura e peças.
pub fn scene_points() -> Vec<Vec3> {
    let mut points = Vec::with_capacity(16);
    for (x, z) in [
        (-FRAME_HALF, -FRAME_HALF),
        (FRAME_HALF, -FRAME_HALF),
        (FRAME_HALF, FRAME_HALF),
        (-FRAME_HALF, FRAME_HALF),
    ] {
        points.push(Vec3::new(x, TABLE_LEVEL, z));
        points.push(Vec3::new(x, PIECES_TOP, z));
    }
    points
}

/// Pose da Vista enquadrada para a área útil de proporção `aspect`.
pub fn framed_pose(view: CameraView, _user_side: Side, aspect: f32) -> CameraPose {
    fit_distance(
        view.pose(),
        &scene_points(),
        aspect,
        PerspectiveProjection::default().fov,
        FILL,
    )
}

/// Mantém a pose da Vista ativa enquadrada. Câmera livre (o usuário mexeu) não é tocada.
pub(crate) fn fit_active_view(
    mut rig: ResMut<CameraRig>,
    dock: Res<Dock>,
    session: Res<Session>,
    window: Single<&Window, With<PrimaryWindow>>,
) {
    let Some(view) = rig.view else { return };
    let area = dock.board_size(window.size());
    if area.x < 1.0 || area.y < 1.0 {
        return;
    }
    let pose = framed_pose(view, session.user_side(), area.x / area.y);
    if pose != rig.pose {
        rig.pose = pose;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Projeção em coordenadas normalizadas; `None` atrás da câmera.
    fn project(pose: &CameraPose, p: Vec3, aspect: f32) -> Option<(f32, f32)> {
        let forward = (pose.target - pose.position()).normalize();
        let right = pose.right();
        let up = right.cross(forward);
        let rel = p - pose.position();
        let depth = rel.dot(forward);
        let t = (PerspectiveProjection::default().fov / 2.0).tan();
        (depth > 0.0).then(|| {
            (
                rel.dot(right) / (depth * t * aspect),
                rel.dot(up) / (depth * t),
            )
        })
    }

    #[test]
    fn board_fits_in_every_view_with_and_without_dock() {
        // Sem dock (1280), com dock de 340 pt (940), dock largo (640) e tela cheia (1920).
        for width in [1280.0_f32, 940.0, 640.0, 1920.0] {
            let aspect = width / 720.0;
            for view in CameraView::ALL {
                let pose = framed_pose(view, Side::White, aspect);
                for p in scene_points() {
                    let (x, y) = project(&pose, p, aspect).unwrap();
                    assert!(
                        x.abs() <= 1.0 && y.abs() <= 1.0,
                        "{view:?} {width}: ({x}, {y})"
                    );
                }
            }
        }
    }

    #[test]
    fn every_view_targets_the_board_center_and_the_center_is_on_screen_center() {
        for width in [1280.0_f32, 940.0, 640.0, 1920.0] {
            let aspect = width / 720.0;
            for side in [Side::White, Side::Black] {
                for view in CameraView::ALL {
                    let pose = framed_pose(view, side, aspect);
                    assert_eq!(pose.target, Vec3::ZERO, "{view:?} {side:?} {width}");
                    let (x, y) = project(&pose, Vec3::ZERO, aspect).unwrap();
                    assert!(x.abs() < 1e-4 && y.abs() < 1e-4, "({x}, {y})");
                }
            }
        }
    }

    #[test]
    fn the_clock_does_not_enter_the_framing() {
        assert!(
            scene_points().iter().all(|p| p.x.abs() <= FRAME_HALF),
            "só o tabuleiro (com a moldura) define o enquadramento"
        );
    }

    #[test]
    fn a_narrower_area_pulls_the_camera_back() {
        let wide = framed_pose(CameraView::Standard, Side::White, 1280.0 / 720.0);
        let narrow = framed_pose(CameraView::Standard, Side::White, 940.0 / 720.0);
        assert!(narrow.distance > wide.distance);
    }
}
