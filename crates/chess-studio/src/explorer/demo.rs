//! Cenas de demonstração para as capturas de evidência: `chess-studio --screenshot <dir> --panel
//! --explorer-demo <cena> [--moves ...] [--theme dark|light]`. Só existe com `--screenshot`;
//! nenhum pedido sai e os nomes de jogadores são fictícios.
//!
//! Cenas: `library`, `masters` (como ficará com a Conexão autorizada), `signin`, `off`,
//! `limited`, `stale`, `nodata`, `locked`, `endgame`, `endgame-off`, `toomany` e `cloud`.

use super::ExplorerUi;
use super::driver::{CloudRow, EndgamesView, OpeningsView};
use super::model::{
    Age, CloudView, EndgameMove, EndgameTable, How, MoveRow, Notice, Nuance, OpeningsTable,
    OriginNote, Outlook, ReferenceRow, Source, Tab, Verdict,
};
use crate::capture::{CaptureMode, option_value};
use crate::dock::{Dock, PanelRegistry, ids};
use crate::net_service::NetService;
use crate::session::Session;
use bevy::prelude::*;
use chess_core::Color;
use chess_net::{ProviderId, Timestamp};
use chess_providers::Score;
use chess_store::Store;
use std::sync::Mutex;

/// Dados prontos de uma cena (no lugar do que viria da rede).
#[derive(Default)]
pub struct Scene {
    openings: Option<OpeningsView>,
    endgames: Option<EndgamesView>,
    cloud: Option<CloudRow>,
}

impl Scene {
    pub fn openings_view(&self) -> Option<OpeningsView> {
        self.openings.clone()
    }

    pub fn endgames_view(&self) -> Option<EndgamesView> {
        self.endgames.clone()
    }

    pub fn cloud_row(&self) -> Option<CloudRow> {
        self.cloud.clone()
    }
}

fn now_ms() -> i64 {
    NetService::now().as_millis()
}

fn lichess_origin(how: How) -> OriginNote {
    OriginNote {
        provider: "Lichess".into(),
        how,
        license: "CC0".into(),
    }
}

fn row(san: &str, white: u64, draws: u64, black: u64, rating: Option<u32>) -> MoveRow {
    MoveRow {
        san: san.into(),
        games: white + draws + black,
        white,
        draws,
        black,
        rating,
    }
}

/// Estatística de mestres fictícia para a Posição depois de 1.e4 e5 2.Cf3.
fn masters_table(how: How) -> OpeningsTable {
    let rows = vec![
        row("Nc6", 61_204, 88_911, 38_420, Some(2531)),
        row("Nf6", 9_870, 17_332, 7_015, Some(2518)),
        row("d6", 2_105, 3_480, 2_044, Some(2490)),
        row("Bc5", 640, 802, 391, Some(2475)),
        row("f5", 188, 150, 143, Some(2460)),
    ];
    let sum = |f: fn(&MoveRow) -> u64| rows.iter().map(f).sum::<u64>();
    OpeningsTable {
        opening: Some(("C40".into(), "King's Knight Opening".into())),
        total: sum(|r| r.games),
        white: sum(|r| r.white),
        draws: sum(|r| r.draws),
        black: sum(|r| r.black),
        rows,
        references: vec![
            ReferenceRow {
                id: "AbCdEfGh".into(),
                white: "Beltrano, B".into(),
                white_rating: Some(2750),
                black: "Fulano, A".into(),
                black_rating: Some(2700),
                winner: Some(Color::White),
                year: Some(2019),
            },
            ReferenceRow {
                id: "IjKlMnOp".into(),
                white: "Ciclano, C".into(),
                white_rating: Some(2690),
                black: "Dowling, D".into(),
                black_rating: Some(2680),
                winner: None,
                year: Some(2020),
            },
        ],
        origin: lichess_origin(how),
        still_loading: false,
    }
}

/// Torre e peão contra torre: o final de torre que a evidência mostra.
fn rook_endgame() -> EndgameTable {
    let mv = |san: &str, outlook, nuance, dtz, dtm| EndgameMove {
        san: san.into(),
        verdict: Some(Verdict { outlook, nuance }),
        dtz,
        dtm,
    };
    EndgameTable {
        side: Color::White,
        verdict: Some(Verdict {
            outlook: Outlook::Win,
            nuance: None,
        }),
        dtz: Some(23),
        dtm: Some(41),
        checkmate: false,
        stalemate: false,
        insufficient_material: false,
        moves: vec![
            mv("Rg6+", Outlook::Win, None, Some(23), Some(41)),
            mv("Rg4", Outlook::Win, None, Some(25), Some(45)),
            mv(
                "Kf4",
                Outlook::Win,
                Some(Nuance::CursedWin),
                Some(98),
                Some(0),
            ),
            mv("Rb2", Outlook::Draw, None, Some(0), None),
            mv("Kd3", Outlook::Draw, None, Some(0), None),
            mv("Rg7", Outlook::Loss, None, Some(-4), Some(-9)),
        ],
        origin: lichess_origin(How::Queried(Age::Minutes(3))),
    }
}

fn cloud_view() -> CloudView {
    CloudView {
        depth: 62,
        score_white: Score::Centipawns(24),
        san: ["Nf6", "Nxe5", "d6", "Nf3", "Nxe4", "d4", "d5", "Bd3"]
            .map(str::to_owned)
            .to_vec(),
        first_number: 2,
        white_to_move: false,
        origin: lichess_origin(How::Cached(Timestamp::from_millis(
            now_ms().saturating_sub(2 * 86_400_000),
        ))),
    }
}

fn scene(name: &str) -> Option<Scene> {
    let table = |t: OpeningsTable| OpeningsView::Table(t);
    let notice = |n: Notice| OpeningsView::Notice(n);
    Some(match name {
        "masters" => Scene {
            openings: Some(table(masters_table(How::Queried(Age::Minutes(3))))),
            ..Scene::default()
        },
        "stale" => Scene {
            openings: Some(table(masters_table(How::Stale(Timestamp::from_millis(
                now_ms().saturating_sub(3 * 86_400_000),
            ))))),
            ..Scene::default()
        },
        "limited" => Scene {
            openings: Some(notice(Notice::Limited {
                until: Timestamp::from_millis(now_ms() + 3 * 60_000),
            })),
            ..Scene::default()
        },
        "nodata" => Scene {
            openings: Some(notice(Notice::NoData)),
            ..Scene::default()
        },
        "endgame" => Scene {
            endgames: Some(EndgamesView::Table(rook_endgame())),
            ..Scene::default()
        },
        "cloud" => Scene {
            cloud: Some(CloudRow::Ready(cloud_view())),
            ..Scene::default()
        },
        "library" | "signin" | "off" | "locked" | "endgame-off" | "toomany" | "cloud-off" => {
            Scene::default()
        }
        _ => return None,
    })
}

/// Partidas fictícias da Biblioteca que passam pela Posição 1.e4 e5 2.Cf3.
fn seed_library(store: &mut Store) {
    const WHITE: [&str; 6] = [
        "Ana Álvares",
        "Beto Lima",
        "Carla Dias",
        "Davi Rocha",
        "Eva Prado",
        "Caio Neves",
    ];
    // (lances depois de 1.e4 e5 2.Nf3, resultado, quantas vezes)
    let lines: [(&str, &str, usize); 9] = [
        ("Nc6 3. Bb5 a6 4. Ba4 Nf6 5. O-O Be7", "1-0", 7),
        ("Nc6 3. Bb5 a6 4. Ba4 Nf6 5. O-O Be7", "1/2-1/2", 4),
        ("Nc6 3. Bb5 Nf6 4. O-O Nxe4 5. d4 Nd6", "0-1", 3),
        ("Nc6 3. Bc4 Bc5 4. c3 Nf6 5. d3 d6", "1-0", 5),
        ("Nc6 3. Bc4 Nf6 4. d3 Bc5 5. O-O d6", "1/2-1/2", 3),
        ("Nc6 3. d4 exd4 4. Nxd4 Nf6 5. Nxc6 bxc6", "1-0", 4),
        ("Nf6 3. Nxe5 d6 4. Nf3 Nxe4 5. d4 d5", "1/2-1/2", 4),
        ("Nf6 3. Nxe5 d6 4. Nf3 Nxe4 5. d4 d5", "0-1", 2),
        ("d6 3. d4 Bg4 4. dxe5 Bxf3 5. Qxf3 dxe5", "1-0", 2),
    ];
    let mut pgn = String::new();
    let mut n = 0;
    for (moves, result, times) in lines {
        for _ in 0..times {
            let white = WHITE[n % WHITE.len()];
            let black = WHITE[(n + 3) % WHITE.len()];
            pgn.push_str(&format!(
                "[Event \"Treino {n}\"]\n[White \"{white}\"]\n[Black \"{black}\"]\n\
                 [Date \"2026.{:02}.{:02}\"]\n[Result \"{result}\"]\n\n\
                 1. e4 e5 2. Nf3 {moves} {result}\n\n",
                1 + n % 11,
                1 + (n * 3) % 27,
            ));
            n += 1;
        }
    }
    let _ = store.import_pgn(&pgn, Some("treinos.pgn"));
}

/// Aplica a cena pedida em `--explorer-demo`. Só roda em captura.
#[allow(clippy::too_many_arguments)] // parâmetros de sistema do Bevy
pub fn apply_demo(
    mut state: ResMut<ExplorerUi>,
    mut dock: ResMut<Dock>,
    registry: Res<PanelRegistry>,
    mut session: ResMut<Session>,
    net: Option<Res<NetService>>,
    mut library: Option<ResMut<crate::library_ui::LibraryUi>>,
    capture: Option<Res<CaptureMode>>,
    mut frame: Local<u32>,
) {
    if capture.is_none() {
        return;
    }
    let Some(name) = option_value("--explorer-demo") else {
        return;
    };
    *frame += 1;
    if *frame == 1 {
        // Evidência: só o Painel da cena no dock, para ele ocupar a altura toda.
        let only = match name.as_str() {
            "cloud" | "cloud-off" => ids::ANALYSIS,
            "import" | "import-done" => ids::LIBRARY,
            _ => ids::EXPLORER,
        };
        for def in registry.iter() {
            dock.layout.set_visible(def.id, def.id == only);
        }
    }
    if *frame != 3 {
        return;
    }
    if matches!(name.as_str(), "import" | "import-done") {
        if let Some(library) = library.as_mut() {
            library.demo_online(name == "import");
        }
        if let Some(net) = net {
            let _ = net.set_enabled(ProviderId::Lichess, true);
        }
        return;
    }
    let Some(scene) = scene(&name) else {
        warn!("--explorer-demo: cena desconhecida `{name}`");
        return;
    };
    match name.as_str() {
        "library" => {
            if let Ok(mut store) = Store::open_in_memory() {
                seed_library(&mut store);
                state.local = Some(Mutex::new(store));
            }
        }
        "masters" | "stale" | "limited" | "nodata" | "signin" | "off" => {
            state.explorer.source = Source::Masters;
        }
        "endgame" | "endgame-off" | "toomany" => {
            state.explorer.tab = Tab::Endgames;
            if name != "toomany" {
                let _ = session.study_from_fen("8/8/4k3/8/8/4KP2/6R1/7r w - - 0 1");
            }
        }
        _ => {}
    }
    if matches!(
        name.as_str(),
        "endgame" | "cloud" | "masters" | "stale" | "signin" | "toomany" | "nodata" | "limited"
    ) {
        // A cena usa dados prontos; o Provedor aparece ligado como na Central.
        if let Some(net) = net {
            let _ = net.set_enabled(ProviderId::Lichess, true);
        }
    }
    if name == "cloud" || name == "cloud-off" {
        let _ = session
            .study_from_fen("r1bqkbnr/pppp1ppp/2n5/4p3/4P3/5N2/PPPP1PPP/RNBQKB1R b KQkq - 2 2");
    }
    state.demo = Some(scene);
}
