//! Modo de evidência: `chess-studio --screenshot <dir>` percorre as Apresentações e Vistas,
//! salva um PNG de cada (captura do próprio Bevy, independente do compositor), registra o
//! tempo de quadro médio em `<dir>/desempenho.txt` e encerra o aplicativo.

use crate::camera::CameraRig;
use crate::presentation::Presentation;
use bevy::prelude::*;
use bevy::render::view::screenshot::{Screenshot, save_to_disk};
use chess_input::CameraView;
use std::fmt::Write as _;
use std::path::PathBuf;

/// Quadros de espera depois de trocar a cena (carregar assets, sombras, mipmaps).
const SETTLE_FRAMES: u32 = 150;
/// Quadros usados para medir o tempo médio antes de capturar.
const MEASURE_FRAMES: u32 = 120;
/// Quadros após o pedido de captura, para o PNG chegar ao disco.
const SAVE_FRAMES: u32 = 40;

struct Step {
    file: &'static str,
    presentation: Presentation,
    view: CameraView,
}

const STEPS: [Step; 7] = [
    Step {
        file: "3d-profissional-padrao",
        presentation: Presentation::Professional3d,
        view: CameraView::Standard,
    },
    Step {
        file: "3d-profissional-obliqua",
        presentation: Presentation::Professional3d,
        view: CameraView::Oblique,
    },
    Step {
        file: "3d-profissional-superior",
        presentation: Presentation::Professional3d,
        view: CameraView::Top,
    },
    Step {
        file: "mesa-imersiva-padrao",
        presentation: Presentation::ImmersiveTable,
        view: CameraView::Standard,
    },
    Step {
        file: "mesa-imersiva-obliqua",
        presentation: Presentation::ImmersiveTable,
        view: CameraView::Oblique,
    },
    Step {
        file: "mesa-imersiva-superior",
        presentation: Presentation::ImmersiveTable,
        view: CameraView::Top,
    },
    Step {
        file: "2d-tecnico",
        presentation: Presentation::Technical2d,
        view: CameraView::Top,
    },
];

/// Presença deste recurso ativa o modo de captura (o painel egui fica oculto).
#[derive(Resource, Debug)]
pub struct CaptureMode {
    pub dir: PathBuf,
    step: usize,
    frame: u32,
    frame_time_sum: f32,
    report: String,
}

impl CaptureMode {
    pub fn new(dir: PathBuf) -> Self {
        Self {
            dir,
            step: 0,
            frame: 0,
            frame_time_sum: 0.0,
            report: String::new(),
        }
    }
}

pub struct CapturePlugin;

impl Plugin for CapturePlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Update, run_capture.run_if(resource_exists::<CaptureMode>));
    }
}

fn run_capture(
    mut mode: ResMut<CaptureMode>,
    time: Res<Time>,
    mut presentation: ResMut<Presentation>,
    mut rig: ResMut<CameraRig>,
    mut commands: Commands,
    mut exit: MessageWriter<AppExit>,
) {
    let Some(step) = STEPS.get(mode.step) else {
        let path = mode.dir.join("desempenho.txt");
        if let Err(err) = std::fs::write(&path, &mode.report) {
            error!("não foi possível salvar {}: {err}", path.display());
        }
        exit.write(AppExit::Success);
        return;
    };
    if mode.frame == 0 {
        *presentation = step.presentation;
        rig.pose = step.view.pose();
        rig.view = Some(step.view);
    }
    mode.frame += 1;
    if mode.frame > SETTLE_FRAMES - MEASURE_FRAMES && mode.frame <= SETTLE_FRAMES {
        mode.frame_time_sum += time.delta_secs();
    }
    if mode.frame == SETTLE_FRAMES {
        let avg_ms = mode.frame_time_sum / MEASURE_FRAMES as f32 * 1000.0;
        let line = format!(
            "{}: {:.2} ms/quadro ({:.0} quadros/s)\n",
            step.file,
            avg_ms,
            1000.0 / avg_ms.max(f32::EPSILON)
        );
        info!("{}", line.trim_end());
        let _ = write!(mode.report, "{line}");
        let path = mode.dir.join(format!("{}.png", step.file));
        commands
            .spawn(Screenshot::primary_window())
            .observe(save_to_disk(path));
    }
    if mode.frame >= SETTLE_FRAMES + SAVE_FRAMES {
        mode.step += 1;
        mode.frame = 0;
        mode.frame_time_sum = 0.0;
    }
}
