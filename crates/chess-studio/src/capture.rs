//! Modo de evidência: `chess-studio --screenshot <dir>` percorre as Apresentações e Vistas,
//! salva um PNG de cada (captura do próprio Bevy, independente do compositor), registra o
//! tempo de quadro médio em `<dir>/desempenho.txt` e encerra o aplicativo.

use crate::camera::CameraRig;
use crate::presentation::Presentation;
use crate::session::Session;
use bevy::prelude::*;
use bevy::render::view::screenshot::{Screenshot, save_to_disk};
use chess_core::Square;
use chess_game::Command;
use chess_input::{CameraView, GestureEvent, Modifiers, PointerButton};
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

/// Roteiro opcional da captura: Lances jogados e peça selecionada antes das fotos.
#[derive(Debug, Default, Clone)]
pub struct CaptureScript {
    /// Lances em coordenadas (`e2e4`, `a7a8q`), jogados como Comandos da Partida.
    pub moves: Vec<String>,
    /// Peça a selecionar por clique depois dos Lances (mostra elevação e destinos).
    pub select: Option<Square>,
    /// Mantém o painel egui visível.
    pub show_panel: bool,
    /// Perfil de auxílios da captura: `physical`, `training` (padrão) ou `ghosts`
    /// (Treinamento com peças fantasmas).
    pub aids: Option<String>,
    /// Entra na revisão neste meio-lance antes das fotos.
    pub review: Option<usize>,
}

/// Presença deste recurso ativa o modo de captura (o painel egui fica oculto por padrão).
#[derive(Resource, Debug)]
pub struct CaptureMode {
    pub dir: PathBuf,
    pub show_panel: bool,
    script: CaptureScript,
    scripted: bool,
    step: usize,
    frame: u32,
    frame_time_sum: f32,
    report: String,
}

impl CaptureMode {
    pub fn new(dir: PathBuf, script: CaptureScript) -> Self {
        Self {
            dir,
            show_panel: script.show_panel,
            script,
            scripted: false,
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

#[allow(clippy::too_many_arguments)]
fn run_capture(
    mut mode: ResMut<CaptureMode>,
    time: Res<Time>,
    mut presentation: ResMut<Presentation>,
    mut rig: ResMut<CameraRig>,
    mut session: ResMut<Session>,
    mut aids: ResMut<crate::aids::AidSettings>,
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
    if !mode.scripted {
        mode.scripted = true;
        play_script(&mut session, &mode.script);
        *aids = match mode.script.aids.as_deref() {
            Some("physical") => crate::aids::AidSettings::physical(),
            Some("ghosts") => crate::aids::AidSettings {
                ghosts: true,
                ..crate::aids::AidSettings::training()
            },
            _ => crate::aids::AidSettings::training(),
        };
        if let Some(ply) = mode.script.review {
            session.set_review_ply(Some(ply));
        }
    }
    if mode.frame == 0 {
        *presentation = step.presentation;
        rig.pose = step.view.pose();
        rig.view = Some(step.view);
    }
    mode.frame += 1;
    if mode.frame == 10 {
        select_script(&mut session, &mode.script);
    }
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

/// Joga os Lances do roteiro e seleciona a peça pedida (só para gerar evidências).
fn play_script(session: &mut Session, script: &CaptureScript) {
    for text in &script.moves {
        let parse =
            |range: std::ops::Range<usize>| text.get(range).and_then(|t| t.parse::<Square>().ok());
        let (Some(from), Some(to)) = (parse(0..2), parse(2..4)) else {
            warn!("lance de roteiro inválido: {text}");
            continue;
        };
        session.command(Command::Release { from, to });
        if let Some(piece) = text.get(4..5).and_then(promotion_piece) {
            session.command(Command::ChoosePromotion { piece });
        }
    }
}

/// Seleciona a peça do roteiro (refeito a cada passo: trocar de Apresentação cancela o gesto).
fn select_script(session: &mut Session, script: &CaptureScript) {
    if let Some(square) = script.select
        && session.gesture_visual().origin.is_none()
    {
        session.gesture(GestureEvent::PointerDown {
            button: PointerButton::Primary,
            square: Some(square),
            modifiers: Modifiers::default(),
        });
        session.gesture(GestureEvent::PointerUp {
            square: Some(square),
        });
    }
}

fn promotion_piece(letter: &str) -> Option<chess_core::Piece> {
    match letter {
        "q" => Some(chess_core::Piece::Queen),
        "r" => Some(chess_core::Piece::Rook),
        "b" => Some(chess_core::Piece::Bishop),
        "n" => Some(chess_core::Piece::Knight),
        _ => None,
    }
}
