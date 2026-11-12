//! Modo de evidência: `chess-studio --screenshot <dir>` percorre as Apresentações e Vistas,
//! salva um PNG de cada (captura do próprio Bevy, independente do compositor), registra o
//! tempo de quadro médio em `<dir>/desempenho.txt` e encerra o aplicativo.

use crate::camera::{CameraMemory, CameraRig};
use crate::new_game::{NewGameConfig, NewGameDialog};
use crate::presentation::Presentation;
use crate::session::Session;
use bevy::prelude::*;
use bevy::render::view::screenshot::{Screenshot, save_to_disk};
use chess_core::Square;
use chess_game::{Command, PolicyKind, TimeControl};
use chess_input::{CameraView, GestureEvent, Modifiers, PointerButton};
use std::fmt::Write as _;
use std::path::PathBuf;

/// Valor de uma opção `--<nome> <valor>` da linha de comando (roteiros de evidência).
pub fn option_value(name: &str) -> Option<String> {
    let mut args = std::env::args().skip(1);
    while let Some(arg) = args.next() {
        if arg == name {
            return args.next();
        }
    }
    None
}

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
    /// `--hover <casa>`: força o realce de hover nesta casa (sem cursor real na captura).
    pub hover: Option<Square>,
    /// Mantém o painel egui visível.
    pub show_panel: bool,
    /// Perfil de auxílios da captura: `physical`, `training` (padrão) ou `ghosts`
    /// (Treinamento com peças fantasmas).
    pub aids: Option<String>,
    /// Entra na revisão neste meio-lance antes das fotos.
    pub review: Option<usize>,
    /// `--policy`, `--time`, `--side`: configuração da Partida de roteiro (B7/B8).
    pub config: Option<NewGameConfig>,
    /// `--hud off|study|analysis`: perfil de HUD da captura (C5).
    pub hud: Option<String>,
    /// `--inverted`: Apresentações 3D na Vista invertida (do lado das pretas).
    pub inverted: bool,
    /// `--new-game`: mostra a tela de Nova Partida (com a configuração acima preenchida).
    pub show_new_game: bool,
    /// `--profile physical|training|study|competition|minimal`: aplica o Perfil de interface de fábrica e tira
    /// uma única foto (C4).
    pub profile: Option<String>,
    /// `--single <nome>`: tira uma única foto com este nome (telas novas: Controles,
    /// seletor de perfis, comparação de qualidade).
    pub single: Option<&'static str>,
}

impl CaptureScript {
    fn factory_profile(&self) -> Option<crate::profiles::FactoryProfile> {
        use crate::profiles::FactoryProfile;
        match self.profile.as_deref()? {
            "physical" => Some(FactoryProfile::PhysicalGame),
            "training" => Some(FactoryProfile::AssistedTraining),
            "study" => Some(FactoryProfile::StudyAnalysis),
            "competition" => Some(FactoryProfile::Competition),
            "minimal" => Some(FactoryProfile::Minimal),
            _ => None,
        }
    }
}

/// Presença deste recurso ativa o modo de captura (o painel egui fica oculto por padrão).
#[derive(Resource, Debug)]
pub struct CaptureMode {
    pub dir: PathBuf,
    pub show_panel: bool,
    pub show_new_game: bool,
    script: CaptureScript,
    scripted: bool,
    step: usize,
    frame: u32,
    frame_time_sum: f32,
    report: String,
}

impl CaptureMode {
    /// Casa de hover forçada pelo roteiro (`--hover`).
    pub fn forced_hover(&self) -> Option<Square> {
        self.script.hover
    }

    pub fn new(dir: PathBuf, script: CaptureScript) -> Self {
        Self {
            dir,
            show_panel: script.show_panel,
            show_new_game: script.show_new_game,
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
        // O passeio da Partida online (`--online-tour`) tira as próprias fotos e segura esta.
        app.add_systems(
            Update,
            run_capture.run_if(
                |mode: Option<Res<CaptureMode>>,
                 hold: Option<Res<crate::online::demo::CaptureHold>>| {
                    mode.is_some() && hold.is_none()
                },
            ),
        );
    }
}

#[allow(clippy::too_many_arguments)]
fn run_capture(
    mut mode: ResMut<CaptureMode>,
    time: Res<Time>,
    mut presentation: ResMut<Presentation>,
    mut rig: ResMut<CameraRig>,
    mut memory: ResMut<CameraMemory>,
    mut session: ResMut<Session>,
    mut aids: ResMut<crate::aids::AidSettings>,
    mut hud: ResMut<crate::hud::HudSettings>,
    mut dialog: ResMut<NewGameDialog>,
    mut profiles: ResMut<crate::profiles::InterfaceProfiles>,
    mut commands: Commands,
    mut exit: MessageWriter<AppExit>,
    ready: Res<crate::ready::SceneReady>,
) {
    // B22: nada de contar quadros nem fotografar antes da cena pronta.
    if let Some(err) = ready.timeout_error() {
        error!("captura abortada: {err}");
        exit.write(AppExit::from_code(2));
        return;
    }
    if !ready.is_ready() {
        return;
    }
    // Com `--profile` a captura é uma foto só, na Apresentação do perfil.
    let profile_step = mode.script.factory_profile().and_then(|f| {
        (mode.step == 0).then(|| Step {
            file: match f {
                crate::profiles::FactoryProfile::PhysicalGame => "perfil-jogo-fisico",
                crate::profiles::FactoryProfile::AssistedTraining => "perfil-treinamento",
                crate::profiles::FactoryProfile::StudyAnalysis => "perfil-estudo",
                crate::profiles::FactoryProfile::Competition => "perfil-competicao",
                crate::profiles::FactoryProfile::Minimal => "perfil-minimo",
            },
            presentation: f.profile().presentation,
            view: CameraView::Standard,
        })
    });
    let single_step = mode.script.single.and_then(|file| {
        (mode.step == 0).then_some(Step {
            file,
            presentation: Presentation::Professional3d,
            view: CameraView::Standard,
        })
    });
    let step = if mode.script.factory_profile().is_some() {
        profile_step.as_ref()
    } else if mode.script.single.is_some() {
        single_step.as_ref()
    } else {
        STEPS.get(mode.step)
    };
    let Some(step) = step else {
        if let Some(load) = ready.load_time() {
            let _ = writeln!(
                mode.report,
                "carregamento da cena: {:.2} s",
                load.as_secs_f32()
            );
        }
        let path = mode.dir.join("desempenho.txt");
        if let Err(err) = std::fs::write(&path, &mode.report) {
            error!("não foi possível salvar {}: {err}", path.display());
        }
        exit.write(AppExit::Success);
        return;
    };
    if !mode.scripted {
        mode.scripted = true;
        if let Some(config) = &mode.script.config {
            dialog.draft = config.clone();
            dialog.seeded = true;
            session.start(config);
            rig.pose = config.initial_view().pose();
        }
        play_script(&mut session, &mode.script);
        if let Some(f) = mode.script.factory_profile() {
            profiles.request = Some(crate::profiles::ProfileRequest::Apply(
                crate::profiles::ProfileKey::Factory(f),
            ));
        } else {
            *aids = match mode.script.aids.as_deref() {
                Some("physical") => crate::aids::AidSettings::physical(),
                Some("ghosts") => crate::aids::AidSettings {
                    ghosts: true,
                    ..crate::aids::AidSettings::training()
                },
                _ => crate::aids::AidSettings::training(),
            };
            *hud = match mode.script.hud.as_deref() {
                Some("study") => crate::hud::HudSettings::study(),
                Some("analysis") => crate::hud::HudSettings::analysis(),
                _ => crate::hud::HudSettings::off(),
            };
        }
        if let Some(ply) = mode.script.review {
            session.set_review_ply(Some(ply));
        }
    }
    let view = if mode.script.inverted && step.view != CameraView::Top {
        CameraView::Inverted
    } else {
        step.view
    };
    if mode.frame == 0 {
        // A câmera da Apresentação que entra vem da memória por Apresentação (B12).
        if *presentation == step.presentation {
            rig.pose = view.pose();
            rig.view = Some(view);
        } else {
            memory.set(step.presentation, view.pose(), Some(view));
            *presentation = step.presentation;
        }
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

/// `--policy assisted|fide|elimination`, `--time 90+30` (minutos + segundos) e
/// `--side white|black`; sem nenhum deles, o roteiro usa a Partida padrão.
pub fn parse_config(
    policy: Option<&str>,
    time: Option<&str>,
    side: Option<&str>,
) -> Option<NewGameConfig> {
    if policy.is_none() && time.is_none() && side.is_none() {
        return None;
    }
    let policy = match policy {
        Some("fide") => PolicyKind::FideSimulation,
        Some("elimination") => PolicyKind::SuddenElimination,
        _ => PolicyKind::Assisted,
    };
    let mut config = NewGameConfig::for_policy(policy);
    if let Some((base, inc)) = time.and_then(|t| t.split_once('+')) {
        config.time = match (base.parse::<u64>(), inc.parse::<u64>()) {
            (Ok(base), Ok(inc)) => Some(TimeControl {
                base_ms: base * 60_000,
                increment_ms: inc * 1000,
            }),
            _ => config.time,
        };
    }
    if side == Some("black") {
        config.user_side = chess_core::Color::Black;
    }
    Some(config)
}
