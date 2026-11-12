//! Cenas de demonstração para as capturas (`--engine-online-tour cena,cena`, com
//! `--screenshot <dir>`): dados **fictícios** (apelidos inventados), nenhum pedido de rede,
//! nenhum banco. Cada cena é fotografada nos Temas Escuro e Claro numa só execução e grava
//! `engine-online-<cena>-<tema>.png`.
//!
//! Cenas da Engine online: `off`, `ready`, `strength`, `resumed`, `exit`, `waiting`, `playing`, `fairplay-stopped`, `stopping`, `lost`,
//! `convert`. Modo espectador (#302): `watch-confirm` (a pergunta, com a Partida do usuário em
//! andamento), `spectator` e `spectator-2d` (a Partida da Engine no tabuleiro, 3D e 2D técnico)
//! e `spectator-ended`. Cenas da Engine remota (Painel Análise): `remote-warning`,
//! `remote-active`, `remote-disabled`, `remote-limit`, `remote-fairplay`.

use super::spectator::SpectatorUi;
use super::view::{ConversionView, Form, ViewModel};
use super::{EngineOnlineUi, service};
use crate::analysis::{Analysis, Lock, Shown, Status};
use crate::dock::ids;
use crate::presentation::Presentation;
use crate::remote_engine::RemoteFlow;
use crate::session::Session;
use bevy::prelude::*;
use chess_core::CanonicalPosition;
use chess_net::ProviderId;
use chess_providers::{DeclineReason, ProviderError};
use chess_uci::{AnalysisLine, AnalysisUpdate, Score};
use service::{Activity, EngineSpec, GameRow, RunState, StopReason};
use std::path::PathBuf;

/// Uma cena: o que a tela da Engine online mostra (ou, nas remotas, o que o Painel mostra).
#[derive(Debug, Clone)]
pub struct Scene {
    pub model: ViewModel,
    pub remote: Option<RemoteScene>,
    /// A pergunta de saída aberta sobre a cena.
    pub exit: bool,
    /// O que o tabuleiro mostra (modo espectador, #302).
    pub board: Board,
    form: fn(&mut Form),
}

/// O tabuleiro de uma cena.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Board {
    /// O tabuleiro como está (as cenas da tela).
    Keep,
    /// A Partida do usuário em andamento, com relógio, e a pergunta antes de trocar a vista.
    Confirm,
    /// O modo espectador: a Partida fictícia da Engine no tabuleiro (terminada ou não, no 3D ou
    /// no 2D técnico).
    Spectator { ended: bool, two_d: bool },
}

/// A Partida fictícia da Engine que as cenas do modo espectador mostram.
const WATCHED: &str = "Hq3vT8aZ";
/// Giuoco Piano, 22 meios-lances: a vez é da Engine (brancas).
const GIUOCO: [&str; 22] = [
    "e2e4", "e7e5", "g1f3", "b8c6", "f1c4", "f8c5", "c2c3", "g8f6", "d2d3", "d7d6", "e1g1", "e8g8",
    "a2a4", "a7a6", "f1e1", "c5a7", "b1d2", "c8e6", "c4e6", "f7e6", "d2f1", "d8e8",
];

/// A Partida da Engine online como o Lichess a mandaria (apelidos inventados).
pub(super) fn watched_game(ended: bool) -> Option<chess_game::Game> {
    use chess_game::{
        Command, EndReason, Game, GameResult, GameSetup, OnlineGame, OnlinePlayer, Opponent,
        Outcome, ProviderClock, ProviderEnd, ProviderSnapshot, TimeControl, Timestamp, Variant,
    };
    let setup = GameSetup::online(
        OnlineGame {
            provider: service::BOT_KEY.into(),
            remote_id: WATCHED.into(),
            local_color: chess_core::Color::White,
            opponent: Opponent::Human,
            white: OnlinePlayer {
                name: "Motor_Demo".into(),
                rating: Some(2310),
            },
            black: OnlinePlayer {
                name: "TorreDeMadeira".into(),
                rating: Some(1720),
            },
            rated: true,
            days_per_move: None,
        },
        Variant::Standard,
        None,
    )
    .ok()?
    .with_time_control(TimeControl {
        base_ms: 180_000,
        increment_ms: 2_000,
    });
    let mut game = Game::new(setup);
    let snapshot = ProviderSnapshot {
        moves: GIUOCO.iter().map(|m| (*m).to_owned()).collect(),
        clock: Some(ProviderClock {
            white_ms: 151_000,
            black_ms: 128_000,
            running: !ended,
        }),
        draw_offer: None,
        end: ended.then_some(ProviderEnd::Decided(Outcome {
            result: GameResult::WhiteWins,
            reason: EndReason::Resignation,
        })),
    };
    game.handle(Command::Provider(snapshot), Timestamp::mono(1))
        .ok()?;
    Some(game)
}

/// A Partida do usuário em andamento na cena da pergunta: local, com relógio.
fn own_game() -> Session {
    let mut session = Session::new();
    session.start(&crate::new_game::NewGameConfig {
        time: Some(chess_game::TimeControl {
            base_ms: 600_000,
            increment_ms: 5_000,
        }),
        ..crate::new_game::NewGameConfig::default()
    });
    for (from, to) in [("e2", "e4"), ("c7", "c5"), ("g1", "f3")] {
        if let (Ok(from), Ok(to)) = (from.parse(), to.parse()) {
            session.command(chess_game::Command::Release { from, to });
        }
    }
    session
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RemoteScene {
    Warning,
    Active,
    Disabled,
    Limit,
    FairPlay,
}

fn engines() -> Vec<EngineSpec> {
    ["Stockfish 17", "Lc0 0.31", "Berserk 13"]
        .into_iter()
        .map(|name| EngineSpec {
            name: name.to_owned(),
            path: PathBuf::from(format!(
                "/engines/{}",
                name.to_lowercase().replace(' ', "-")
            )),
            options: Vec::new(),
        })
        .collect()
}

fn base() -> ViewModel {
    ViewModel {
        connected: true,
        provider_on: true,
        engines: engines(),
        state: RunState::Stopped,
        stop_reason: None,
        running_engine: None,
        rows: Vec::new(),
        activity: Vec::new(),
        counts: (0, 0, 0),
        conversion: None,
        watching: None,
    }
}

fn row(
    id: &str,
    opponent: &str,
    rating: u32,
    white: bool,
    plies: usize,
    thinking: bool,
) -> GameRow {
    GameRow {
        id: id.to_owned(),
        opponent: opponent.to_owned(),
        rating: Some(rating),
        white,
        plies,
        rated: true,
        speed: "blitz".to_owned(),
        engine_thinking: thinking,
        link_lost: false,
        over: false,
    }
}

fn history() -> Vec<Activity> {
    vec![
        Activity::Accepted {
            name: "TorreDeMadeira".to_owned(),
        },
        Activity::GameStarted {
            opponent: "TorreDeMadeira".to_owned(),
        },
        Activity::Declined {
            name: "Gambito_Verde".to_owned(),
            reason: DeclineReason::TimeControl,
        },
        Activity::Declined {
            name: "Coruja_Azul".to_owned(),
            reason: DeclineReason::NoBot,
        },
    ]
}

impl Scene {
    pub fn named(name: &str) -> Option<Self> {
        let scene = |model: ViewModel, form: fn(&mut Form)| Self {
            model,
            remote: None,
            exit: false,
            board: Board::Keep,
            form,
        };
        let remote = |kind| Self {
            model: base(),
            remote: Some(kind),
            exit: false,
            board: Board::Keep,
            form: |_| {},
        };
        let spectator = |ended, two_d| {
            Some(Self {
                board: Board::Spectator { ended, two_d },
                ..Self::named("playing")?
            })
        };
        Some(match name {
            "off" => scene(
                ViewModel {
                    connected: false,
                    ..base()
                },
                |_| {},
            ),
            "ready" => scene(base(), |_| {}),
            "strength" => scene(base(), |form| form.settings.elo = Some(1800)),
            "resumed" => scene(
                ViewModel {
                    state: RunState::Playing,
                    running_engine: Some("Stockfish 17".to_owned()),
                    counts: (0, 0, 0),
                    rows: vec![row("Hq3vT8aZ", "TorreDeMadeira", 1720, true, 23, false)],
                    activity: vec![Activity::GameResumed {
                        opponent: "TorreDeMadeira".to_owned(),
                    }],
                    ..base()
                },
                |_| {},
            ),
            "exit" => Self {
                exit: true,
                ..Self::named("playing")?
            },
            "watch-confirm" => Self {
                board: Board::Confirm,
                ..Self::named("playing")?
            },
            "spectator" => spectator(false, false)?,
            "spectator-2d" => spectator(false, true)?,
            "spectator-ended" => spectator(true, false)?,
            "waiting" => scene(
                ViewModel {
                    state: RunState::Waiting,
                    running_engine: Some("Stockfish 17".to_owned()),
                    counts: (1, 2, 1),
                    activity: vec![
                        Activity::Accepted {
                            name: "Peao_Passado".to_owned(),
                        },
                        Activity::GameStarted {
                            opponent: "Peao_Passado".to_owned(),
                        },
                        Activity::GameEnded {
                            opponent: "Peao_Passado".to_owned(),
                            result: Some("1-0".to_owned()),
                        },
                        Activity::Declined {
                            name: "Gambito_Verde".to_owned(),
                            reason: DeclineReason::TimeControl,
                        },
                    ],
                    ..base()
                },
                |_| {},
            ),
            "playing" => scene(
                ViewModel {
                    state: RunState::Playing,
                    running_engine: Some("Stockfish 17".to_owned()),
                    counts: (2, 2, 0),
                    rows: vec![
                        row(WATCHED, "TorreDeMadeira", 1720, true, 22, true),
                        row("Pw9xK2mB", "Coruja_Azul", 1648, false, 14, false),
                    ],
                    activity: history(),
                    ..base()
                },
                |form| form.settings.policy.max_concurrent = 2,
            ),
            "fairplay-stopped" => scene(
                ViewModel {
                    state: RunState::Playing,
                    running_engine: Some("Stockfish 17".to_owned()),
                    counts: (2, 2, 0),
                    rows: vec![row("Hq3vT8aZ", "TorreDeMadeira", 1720, true, 23, true)],
                    activity: {
                        let mut items = history();
                        items.push(Activity::FairPlayStopped {
                            scope: crate::fair_play::LockScope::Everything,
                        });
                        items
                    },
                    ..base()
                },
                |_| {},
            ),
            "stopping" => scene(
                ViewModel {
                    state: RunState::Stopping,
                    stop_reason: Some(StopReason::User),
                    running_engine: Some("Stockfish 17".to_owned()),
                    counts: (2, 2, 1),
                    rows: vec![row("Hq3vT8aZ", "TorreDeMadeira", 1720, true, 41, false)],
                    activity: history(),
                    ..base()
                },
                |_| {},
            ),
            "lost" => scene(
                ViewModel {
                    state: RunState::Stopping,
                    stop_reason: Some(StopReason::ConnectionLost(
                        ProviderError::ConnectionRejected {
                            provider: ProviderId::Lichess,
                            status: 401,
                        },
                    )),
                    running_engine: Some("Stockfish 17".to_owned()),
                    counts: (1, 0, 0),
                    rows: vec![row("Hq3vT8aZ", "TorreDeMadeira", 1720, true, 18, false)],
                    activity: vec![Activity::Accepted {
                        name: "TorreDeMadeira".to_owned(),
                    }],
                    ..base()
                },
                |_| {},
            ),
            "convert" => scene(
                ViewModel {
                    conversion: None,
                    ..base()
                },
                |form| form.upgrade_open = true,
            ),
            "convert-done" => scene(
                ViewModel {
                    conversion: Some(ConversionView::Done),
                    ..base()
                },
                |form| form.upgrade_open = true,
            ),
            "remote-warning" => remote(RemoteScene::Warning),
            "remote-active" => remote(RemoteScene::Active),
            "remote-disabled" => remote(RemoteScene::Disabled),
            "remote-limit" => remote(RemoteScene::Limit),
            "remote-fairplay" => remote(RemoteScene::FairPlay),
            _ => return None,
        })
    }
}

/// A resposta fictícia do Stockfish remoto para a Posição inicial.
fn remote_shown() -> Option<Shown> {
    let base = CanonicalPosition::start();
    let mut position = base.clone();
    let mut pv = Vec::new();
    let mut san = Vec::new();
    for text in ["d2d4", "g8f6", "c2c4", "e7e6", "b1c3", "f8b4"] {
        let mv: chess_core::Move = text.parse().ok()?;
        san.push(position.san(mv).ok()?);
        position = position.play(mv).ok()?;
        pv.push(mv);
    }
    Some(Shown {
        update: AnalysisUpdate {
            id: 1,
            lines: vec![AnalysisLine {
                multipv: 1,
                depth: 12,
                seldepth: None,
                score: Score::Cp(35),
                nodes: None,
                nps: None,
                time_ms: Some(180),
                pv,
                san,
            }],
        },
        base,
        base_ply: 0,
        total: 0,
    })
}

/// O tabuleiro da cena: a Partida do usuário (com a pergunta) ou o modo espectador.
fn enter_board(
    scene: &Scene,
    ui: &mut EngineOnlineUi,
    session: &mut Session,
    spectator: &mut SpectatorUi,
    presentation: &mut Presentation,
    dock: Option<&mut crate::dock::Dock>,
) {
    // Só toca no que muda: a cena 3D se refaz quando a Sessão muda.
    if session.is_spectator() {
        session.leave_spectator();
    }
    if spectator.confirm.is_some() || spectator.demo {
        *spectator = SpectatorUi::default();
    }
    let wanted = if matches!(scene.board, Board::Spectator { two_d: true, .. }) {
        Presentation::Technical2d
    } else {
        Presentation::Professional3d
    };
    if *presentation != wanted {
        *presentation = wanted;
    }
    match scene.board {
        Board::Keep => {}
        Board::Confirm => {
            let mut fresh = own_game();
            fresh.supersede(session);
            *session = fresh;
            spectator.confirm = Some(WATCHED.to_owned());
            spectator.demo = true;
        }
        Board::Spectator { ended, .. } => {
            if let Some(game) = watched_game(ended) {
                session.watch(&game);
            }
            // O tabuleiro é o que importa: a tela sai da frente e o Painel Partida online
            // mostra a Partida vista.
            ui.open = false;
            if let Some(dock) = dock {
                crate::online::reveal_panel(&mut dock.layout);
            }
        }
    }
}

/// Põe a cena na tela.
fn enter(
    scene: Scene,
    ui: &mut EngineOnlineUi,
    analysis: &mut Analysis,
    flow: &mut RemoteFlow,
    dock: Option<&mut crate::dock::Dock>,
) {
    *ui = EngineOnlineUi::default();
    flow.cancel();
    if let Some(kind) = scene.remote {
        if let Some(dock) = dock {
            // Só o Painel Análise, para a captura mostrar a lista de Engines inteira.
            for id in [
                ids::CLOCK,
                ids::CAPTURED,
                ids::BOARD2D,
                ids::NOTATION,
                ids::STUDY,
                ids::AIDS,
                ids::REVIEW,
                ids::ARBITRATION,
                ids::TRAINING,
                ids::LIBRARY,
                ids::OPENINGS,
                ids::EXPLORER,
                ids::SCORESHEET,
                crate::online::PANEL_ID,
                "Engines",
            ] {
                dock.layout.set_visible(id, false);
            }
            dock.layout.set_visible(ids::ANALYSIS, true);
            dock.layout.activate(ids::ANALYSIS);
        }
        let now_problem = |error| Some(error);
        match kind {
            RemoteScene::Warning => {
                analysis.demo_remote(Status::Off, None, None);
                flow.request();
            }
            RemoteScene::Active => {
                analysis.demo_remote(Status::Running, remote_shown(), None);
            }
            RemoteScene::Disabled => analysis.demo_remote(
                Status::Failed("desligado".to_owned()),
                None,
                now_problem(ProviderError::Disabled {
                    provider: ProviderId::ChessApi,
                }),
            ),
            RemoteScene::Limit => analysis.demo_remote(
                Status::Failed("limite".to_owned()),
                None,
                now_problem(ProviderError::RateLimited {
                    provider: ProviderId::ChessApi,
                    until: chess_net::Timestamp::from_millis(
                        crate::net_service::NetService::now().as_millis() + 42_000,
                    ),
                }),
            ),
            RemoteScene::FairPlay => {
                analysis.demo_remote(Status::Locked(Lock::FairPlay), None, None);
            }
        }
        ui.demo = Some(scene);
        return;
    }
    ui.open = true;
    (scene.form)(&mut ui.form);
    ui.form.engine = scene.model.engines.first().map(|e| e.path.clone());
    ui.demo = Some(scene);
}

/// Segura a captura padrão enquanto o passeio tira as fotos.
pub use crate::online::demo::CaptureHold;

/// O passeio pelas cenas: (cena, Tema) na ordem.
#[derive(Resource, Debug)]
pub struct Tour {
    steps: Vec<(String, crate::ui_theme::ThemeMode, &'static str)>,
    index: usize,
    frame: u32,
}

/// Quadros até a cena assentar (layout, Tema, dock) e até a foto ir ao disco.
const TOUR_SETTLE: u32 = 50;
const TOUR_SAVE: u32 = 20;

pub fn start_tour(
    capture: Option<Res<crate::capture::CaptureMode>>,
    mut commands: Commands,
    mut done: Local<bool>,
) {
    if capture.is_none() || *done {
        return;
    }
    *done = true;
    let Some(list) = crate::capture::option_value("--engine-online-tour") else {
        return;
    };
    let mut steps = Vec::new();
    for name in list.split(',').map(str::trim).filter(|n| !n.is_empty()) {
        if Scene::named(name).is_none() {
            warn!("cena desconhecida em --engine-online-tour: {name}");
            continue;
        }
        steps.push((name.to_owned(), crate::ui_theme::ThemeMode::DARK, "dark"));
        steps.push((name.to_owned(), crate::ui_theme::ThemeMode::LIGHT, "light"));
    }
    commands.insert_resource(Tour {
        steps,
        index: 0,
        frame: 0,
    });
    commands.insert_resource(CaptureHold);
}

#[allow(clippy::too_many_arguments)]
pub fn run_tour(
    mut commands: Commands,
    tour: Option<ResMut<Tour>>,
    capture: Option<Res<crate::capture::CaptureMode>>,
    ready: Option<Res<crate::ready::SceneReady>>,
    mut ui: ResMut<EngineOnlineUi>,
    mut analysis: ResMut<Analysis>,
    mut flow: ResMut<RemoteFlow>,
    mut dock: Option<ResMut<crate::dock::Dock>>,
    mut appearance: Option<ResMut<crate::ui_theme::Appearance>>,
    mut exit: MessageWriter<AppExit>,
    mut board: (ResMut<Session>, ResMut<SpectatorUi>, ResMut<Presentation>),
) {
    let (Some(mut tour), Some(capture)) = (tour, capture) else {
        return;
    };
    if !ready.is_some_and(|r| r.is_ready()) {
        return;
    }
    let Some((name, theme, label)) = tour.steps.get(tour.index).cloned() else {
        exit.write(AppExit::Success);
        return;
    };
    if tour.frame == 0 {
        if let Some(appearance) = appearance.as_mut()
            && appearance.theme != theme
        {
            appearance.theme = theme;
        }
        if let Some(scene) = Scene::named(&name) {
            let (session, spectator, presentation) = &mut board;
            let shown = scene.clone();
            enter(
                scene,
                &mut ui,
                &mut analysis,
                &mut flow,
                dock.as_deref_mut(),
            );
            enter_board(
                &shown,
                &mut ui,
                session,
                spectator,
                presentation,
                dock.as_deref_mut(),
            );
        }
    }
    // O Painel novo entra no dock alguns quadros depois: a cena insiste até a foto.
    if tour.frame < TOUR_SETTLE
        && board.0.is_spectator()
        && let Some(dock) = dock.as_mut()
    {
        crate::online::reveal_panel(&mut dock.layout);
    }
    tour.frame += 1;
    if tour.frame == TOUR_SETTLE {
        let path = capture
            .dir
            .join(format!("engine-online-{name}-{label}.png"));
        info!("captura da Engine online: {}", path.display());
        commands
            .spawn(bevy::render::view::screenshot::Screenshot::primary_window())
            .observe(bevy::render::view::screenshot::save_to_disk(path));
    }
    if tour.frame >= TOUR_SETTLE + TOUR_SAVE {
        tour.index += 1;
        tour.frame = 0;
    }
}
