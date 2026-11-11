//! Cenas de demonstração da Partida online para as capturas (`--online-demo <cena>`): dados
//! **fictícios** (apelidos inventados, nenhuma pessoa real), nenhum pedido de rede, nenhum
//! banco. As cenas montam a Partida localmente pelo mesmo retrato que o Lichess mandaria.
//!
//! Cenas: `dialog`, `offline`, `waiting`, `challenges`, `engine`, `list`, `playing`,
//! `reconnecting`, `draw`, `gone`, `toast`, `offer`, `ended`.
//!
//! `--online-tour dialog,playing,...` (com `--screenshot <dir>`) percorre as cenas nos Temas
//! Escuro e Claro numa só execução e grava `online-<cena>-<tema>.png`.

use super::service::{OutgoingState, PlayingRow};
use super::view::{Pace, PanelView};
use super::{OnlineUi, Tab};
use crate::session::Session;
use bevy::prelude::*;
use chess_core::{Color, Move};
use chess_game::{
    Command, EndReason, Game, GameResult, GameSetup, OnlineGame, OnlinePlayer, Opponent, Outcome,
    ProviderClock, ProviderEnd, ProviderSnapshot, TimeControl, Timestamp as GameTime, Variant,
};
use chess_net::Timestamp;
use chess_providers::{Challenge, ChallengePlayer, LinkStatus, OnlineTime, Seek};
use std::time::Duration;

/// Uma cena: o que a tela, o Painel e o aviso mostram.
#[derive(Debug, Clone)]
pub struct Scene {
    pub name: String,
    connected: bool,
    seek: Option<(Seek, Duration, bool)>,
    engine_pending: bool,
    outgoing: Vec<(String, OnlineTime, OutgoingState)>,
    incoming: Vec<Challenge>,
    rows: Vec<PlayingRow>,
    link: Option<LinkStatus>,
    opponent_gone: bool,
    toast: bool,
    offer: Option<usize>,
}

const OPPONENT: &str = "Coruja_Azul";
const CHALLENGER: &str = "TorreDeMadeira";

fn challenge(id: &str, name: &str, rating: u32, time: OnlineTime, rated: bool) -> Challenge {
    Challenge {
        id: id.to_owned(),
        challenger: ChallengePlayer {
            id: name.to_lowercase(),
            name: name.to_owned(),
            title: None,
            rating: Some(rating),
        },
        dest: None,
        variant: "standard".to_owned(),
        rated,
        speed: "blitz".to_owned(),
        time,
        color: "random".to_owned(),
    }
}

impl Scene {
    pub fn named(name: &str) -> Option<Self> {
        let base = Self {
            name: name.to_owned(),
            connected: true,
            seek: None,
            engine_pending: false,
            outgoing: Vec::new(),
            incoming: Vec::new(),
            rows: Vec::new(),
            link: Some(LinkStatus::Live),
            opponent_gone: false,
            toast: false,
            offer: None,
        };
        let blitz = OnlineTime::RealTime {
            base_s: 300,
            increment_s: 3,
        };
        Some(match name {
            "dialog" | "engine" | "playing" | "draw" | "ended" => base,
            "offline" => Self {
                connected: false,
                ..base
            },
            "waiting" => Self {
                seek: Some((
                    Seek {
                        time: OnlineTime::RealTime {
                            base_s: 600,
                            increment_s: 5,
                        },
                        rated: true,
                        color: None,
                    },
                    Duration::from_secs(42),
                    false,
                )),
                ..base
            },
            "challenges" => Self {
                incoming: vec![
                    challenge("Hq3vT8aZ", CHALLENGER, 1720, blitz, true),
                    Challenge {
                        variant: "chess960".to_owned(),
                        ..challenge(
                            "Pw9xK2mB",
                            "Gambito_Verde",
                            1585,
                            OnlineTime::RealTime {
                                base_s: 600,
                                increment_s: 0,
                            },
                            false,
                        )
                    },
                ],
                outgoing: vec![("Peao_Passado".to_owned(), blitz, OutgoingState::Waiting)],
                ..base
            },
            "list" => Self {
                rows: vec![
                    PlayingRow {
                        remote_id: "cR7nQ2wE".into(),
                        opponent: OPPONENT.into(),
                        rating: Some(1648),
                        color: Color::White,
                        my_turn: true,
                        seconds_left: Some(2 * 86_400 + 3600),
                        days: Some(3),
                        stored: true,
                    },
                    PlayingRow {
                        remote_id: "mL4kZ9tY".into(),
                        opponent: "Bispo_Solitario".into(),
                        rating: Some(1702),
                        color: Color::Black,
                        my_turn: true,
                        seconds_left: Some(5 * 3600),
                        days: Some(1),
                        stored: true,
                    },
                    PlayingRow {
                        remote_id: "aD2fG6hJ".into(),
                        opponent: "Gambito_Verde".into(),
                        rating: Some(1585),
                        color: Color::White,
                        my_turn: false,
                        seconds_left: Some(6 * 86_400),
                        days: Some(7),
                        stored: false,
                    },
                ],
                ..base
            },
            "reconnecting" => Self {
                link: Some(LinkStatus::Retrying {
                    attempt: 2,
                    at: crate::net_service::NetService::now().plus(Duration::from_secs(4)),
                }),
                ..base
            },
            "toast" => Self {
                incoming: vec![challenge("Hq3vT8aZ", CHALLENGER, 1720, blitz, true)],
                toast: true,
                ..base
            },
            "offer" => Self {
                offer: Some(2),
                ..base
            },
            "gone" => Self {
                opponent_gone: true,
                ..base
            },
            _ => return None,
        })
    }

    fn dialog_tab(&self) -> Option<Tab> {
        match self.name.as_str() {
            "dialog" | "offline" | "waiting" => Some(Tab::Seek),
            "challenges" => Some(Tab::Challenge),
            "engine" => Some(Tab::Engine),
            "list" => Some(Tab::Playing),
            _ => None,
        }
    }

    pub(super) fn view_for_dialog(&self) -> super::view::DialogView {
        let now = crate::net_service::NetService::now();
        super::view::DialogView {
            connected: self.connected,
            provider_on: self.connected,
            seek: self.seek.map(|(seek, ago, posted)| {
                let ago = i64::try_from(ago.as_millis()).unwrap_or(0);
                (
                    seek,
                    Timestamp::from_millis(now.as_millis().saturating_sub(ago)),
                    posted,
                )
            }),
            engine_pending: self.engine_pending,
            outgoing: self.outgoing.clone(),
            incoming: self.incoming.clone(),
            rows: self.rows.clone(),
            refreshing: false,
            refreshed: (self.name == "list").then_some(now),
            now,
        }
    }

    pub fn panel_view(&self, now: Timestamp) -> PanelView {
        PanelView {
            link: self.link,
            now,
            opponent_gone: self.opponent_gone,
            can_claim: self.opponent_gone,
            incoming: self.incoming.len(),
            stored: 2,
        }
    }

    pub fn toast_challenge(&self) -> Option<Challenge> {
        self.toast.then(|| self.incoming.first().cloned()).flatten()
    }

    pub fn offer(&self) -> Option<usize> {
        self.offer
    }

    /// A Partida no tabuleiro desta cena, se houver.
    fn game(&self) -> Option<Game> {
        let script: (&[&str], Option<&str>, Option<ProviderEnd>, Option<Color>) =
            match self.name.as_str() {
                // Espanhola, e a intenção enviada 4.Ba4 à espera do Lichess.
                "playing" => (&RUY, Some("b5a4"), None, None),
                "reconnecting" | "gone" => (&RUY, None, None, None),
                "draw" => (&RUY_LONG, None, None, Some(Color::Black)),
                "ended" => (
                    &RUY_LONG,
                    None,
                    Some(ProviderEnd::Decided(Outcome {
                        result: GameResult::WhiteWins,
                        reason: EndReason::Resignation,
                    })),
                    None,
                ),
                _ => return None,
            };
        let (moves, sent, end, offer) = script;
        let setup = GameSetup::online(
            OnlineGame {
                provider: "lichess".into(),
                remote_id: "cR7nQ2wE".into(),
                local_color: Color::White,
                opponent: Opponent::Human,
                white: OnlinePlayer {
                    name: "Voce_Demo".into(),
                    rating: Some(1612),
                },
                black: OnlinePlayer {
                    name: OPPONENT.into(),
                    rating: Some(1648),
                },
                rated: true,
                days_per_move: None,
            },
            Variant::Standard,
            None,
        )
        .ok()?
        .with_time_control(TimeControl {
            base_ms: 600_000,
            increment_ms: 5_000,
        });
        let mut game = Game::new(setup);
        let snapshot = ProviderSnapshot {
            moves: moves.iter().map(|m| (*m).to_owned()).collect(),
            clock: Some(ProviderClock {
                white_ms: 512_000,
                black_ms: 547_000,
                running: true,
            }),
            draw_offer: offer,
            end,
        };
        game.handle(Command::Provider(snapshot), GameTime::mono(1))
            .ok()?;
        if let Some(sent) = sent {
            game.send_move(sent.parse::<Move>().ok()?).ok()?;
        }
        Some(game)
    }
}

const RUY: [&str; 6] = ["e2e4", "e7e5", "g1f3", "b8c6", "f1b5", "a7a6"];
const RUY_LONG: [&str; 11] = [
    "e2e4", "e7e5", "g1f3", "b8c6", "f1b5", "a7a6", "b5a4", "g8f6", "e1g1", "f8e7", "f1e1",
];

/// Nas capturas, o Painel Partida online aparece assim que o dock o conhece (o Painel novo
/// entra no layout alguns quadros depois do primeiro).
pub fn show_demo_panel(
    ui: Res<OnlineUi>,
    session: Res<Session>,
    dock: Option<ResMut<crate::dock::Dock>>,
    mut frames: Local<u32>,
) {
    if ui.demo.is_none() || !session.is_online() || *frames > 120 {
        return;
    }
    *frames += 1;
    if let Some(mut dock) = dock {
        super::reveal_panel(&mut dock.layout);
    }
}

/// Põe a cena na tela: o estado da interface, a Partida no tabuleiro e o Painel.
fn enter(
    scene: Scene,
    ui: &mut OnlineUi,
    session: &mut Session,
    dock: Option<&mut crate::dock::Dock>,
) {
    *ui = OnlineUi::default();
    if let Some(tab) = scene.dialog_tab() {
        ui.open_at(tab);
        if scene.name == "challenges" {
            ui.form.name = "Peao_Passado".to_owned();
        }
        if scene.name == "dialog" {
            ui.form.seek_pace = Pace::Rapid10_5;
            ui.form.seek_rated = true;
        }
    }
    match scene.game() {
        Some(game) => {
            session.show_online(game, None, true);
            if let Some(dock) = dock {
                super::reveal_panel(&mut dock.layout);
            }
        }
        None if session.is_online() => {
            let mut fresh = Session::new();
            fresh.supersede(session);
            *session = fresh;
        }
        None => {}
    }
    ui.demo = Some(scene);
}

/// Segura a captura padrão enquanto o passeio tira as fotos (`--online-tour`).
#[derive(Resource, Debug)]
pub struct CaptureHold;

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
    let Some(list) = crate::capture::option_value("--online-tour") else {
        return;
    };
    let mut steps = Vec::new();
    for name in list.split(',').map(str::trim).filter(|n| !n.is_empty()) {
        if Scene::named(name).is_none() {
            warn!("cena desconhecida em --online-tour: {name}");
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
    mut ui: ResMut<OnlineUi>,
    mut session: ResMut<Session>,
    mut dock: Option<ResMut<crate::dock::Dock>>,
    mut appearance: Option<ResMut<crate::ui_theme::Appearance>>,
    mut exit: MessageWriter<AppExit>,
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
            enter(scene, &mut ui, &mut session, dock.as_deref_mut());
        }
    }
    // O Painel novo entra no dock alguns quadros depois: a cena insiste até a foto.
    if tour.frame < TOUR_SETTLE
        && session.is_online()
        && let Some(dock) = dock.as_mut()
    {
        super::reveal_panel(&mut dock.layout);
    }
    tour.frame += 1;
    if tour.frame == TOUR_SETTLE {
        let path = capture.dir.join(format!("online-{name}-{label}.png"));
        info!("captura da Partida online: {}", path.display());
        commands
            .spawn(bevy::render::view::screenshot::Screenshot::primary_window())
            .observe(bevy::render::view::screenshot::save_to_disk(path));
    }
    if tour.frame >= TOUR_SETTLE + TOUR_SAVE {
        tour.index += 1;
        tour.frame = 0;
    }
}

/// Aplica a cena uma vez (capturas). Sem `--online-demo`, nada.
pub fn apply_demo(
    capture: Option<Res<crate::capture::CaptureMode>>,
    mut ui: ResMut<OnlineUi>,
    mut session: ResMut<Session>,
    dock: Option<ResMut<crate::dock::Dock>>,
    mut done: Local<bool>,
) {
    if capture.is_none() || *done {
        return;
    }
    *done = true;
    let Some(name) = crate::capture::option_value("--online-demo") else {
        return;
    };
    let Some(scene) = Scene::named(&name) else {
        warn!("cena desconhecida em --online-demo: {name}");
        return;
    };
    if let Some(tab) = scene.dialog_tab() {
        ui.open_at(tab);
        if scene.name == "challenges" {
            ui.form.name = "Peao_Passado".to_owned();
        }
        if scene.name == "dialog" {
            ui.form.seek_pace = Pace::Rapid10_5;
            ui.form.seek_rated = true;
        }
    }
    if let Some(game) = scene.game() {
        session.show_online(game, None, true);
        if let Some(mut dock) = dock {
            super::reveal_panel(&mut dock.layout);
        }
    }
    ui.demo = Some(scene);
}
