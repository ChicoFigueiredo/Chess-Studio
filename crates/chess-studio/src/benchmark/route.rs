//! Roteiro determinístico da câmera: função pura do tempo decorrido, então duas
//! execuções percorrem exatamente as mesmas poses independentemente do FPS.
//!
//! Roteiro: órbita contínua (uma volta a cada `ORBIT_PERIOD_S`) em cima da pose da Vista
//! ativa; a Vista troca a cada `VIEW_PERIOD_S`, ciclando Padrão, Oblíqua, Superior, Invertida.

use chess_input::{CameraPose, CameraView};

pub const ORBIT_PERIOD_S: f32 = 20.0;
pub const VIEW_PERIOD_S: f32 = 15.0;
const VIEW_CYCLE: [CameraView; 4] = [
    CameraView::Standard,
    CameraView::Oblique,
    CameraView::Top,
    CameraView::Inverted,
];

/// Vista ativa em `t` segundos.
pub fn view_at(t: f32) -> CameraView {
    let slot = (t.max(0.0) / VIEW_PERIOD_S) as usize;
    VIEW_CYCLE[slot % VIEW_CYCLE.len()]
}

/// Pose da câmera em `t` segundos.
pub fn pose_at(t: f32) -> CameraPose {
    let t = t.max(0.0);
    let turn = (t % ORBIT_PERIOD_S) / ORBIT_PERIOD_S;
    view_at(t).pose().orbit(turn * std::f32::consts::TAU, 0.0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn route_is_a_pure_function_of_time() {
        for t in [0.0, 1.5, 14.99, 15.0, 61.0, 299.9] {
            assert_eq!(pose_at(t), pose_at(t));
            assert_eq!(view_at(t), view_at(t));
        }
    }

    #[test]
    fn view_changes_periodically_and_cycles() {
        assert_eq!(view_at(0.0), CameraView::Standard);
        assert_eq!(view_at(VIEW_PERIOD_S - 0.01), CameraView::Standard);
        assert_eq!(view_at(VIEW_PERIOD_S), CameraView::Oblique);
        assert_eq!(view_at(2.0 * VIEW_PERIOD_S), CameraView::Top);
        assert_eq!(view_at(3.0 * VIEW_PERIOD_S), CameraView::Inverted);
        assert_eq!(view_at(4.0 * VIEW_PERIOD_S), CameraView::Standard);
    }

    #[test]
    fn camera_orbits_continuously_within_a_view() {
        let a = pose_at(1.0);
        let b = pose_at(2.0);
        assert_ne!(a.position(), b.position());
        assert_eq!(a.distance, b.distance);
    }

    #[test]
    fn negative_time_is_treated_as_start() {
        assert_eq!(pose_at(-3.0), pose_at(0.0));
    }
}
