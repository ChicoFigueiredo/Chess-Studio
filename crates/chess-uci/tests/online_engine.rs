#![allow(clippy::unwrap_used)]
//! Engine online de ponta a ponta (#234): a Engine UCI **falsa** deste crate (`fake_uci`,
//! ADR-0003: nunca uma Engine real) joga pela Bot API contra o servidor e o relógio falsos do
//! `chess-net`. Respostas do Lichess **sintéticas**, no formato da documentação oficial.
//! O adaptador está em `chess-providers`; o teste mora aqui porque só aqui existe o binário
//! `fake_uci` (`CARGO_BIN_EXE_fake_uci`).

use chess_core::{CanonicalPosition, Color};
use chess_net::testing::{FakeResponse, FakeServer, LiveFeed};
use chess_net::{
    ConnectionId, FakeClock, LocalProfileId, Net, NetConfig, ProviderId, Secret, Timestamp,
};
use chess_providers::{
    Backoff, ConnectionRole, EnginePolicy, GameInfo, LichessBot, OnlineGameSession, SessionNotice,
};
use chess_uci::{EngineConfig, EngineController, Strength, Timeouts};
use chess_vault::testing::MemoryKeyring;
use chess_vault::{SecretVault, VaultConfig};
use serde_json::{Value, json};
use std::sync::Arc;
use std::time::{Duration, Instant};

const FAKE: &str = env!("CARGO_BIN_EXE_fake_uci");
const GAME: &str = "Fa7eUci1";
const WAIT: Duration = Duration::from_secs(20);
const ENGINE_CONNECTION: ConnectionId = ConnectionId(2);
const TOKEN: &str = "lip_TOKEN_DA_ENGINE_FALSA";

fn ms(n: u64) -> Duration {
    Duration::from_millis(n)
}

fn eventually(mut cond: impl FnMut() -> bool) -> bool {
    let start = Instant::now();
    while start.elapsed() < WAIT {
        if cond() {
            return true;
        }
        std::thread::sleep(ms(10));
    }
    cond()
}

fn engine(mode: &str) -> Box<EngineController> {
    let config = EngineConfig::new(FAKE)
        .arg("--mode")
        .arg(mode)
        .timeouts(Timeouts {
            handshake: ms(3000),
            ready: ms(3000),
            search_slack: ms(200),
            search_default: ms(1000),
            stop_grace: ms(300),
            quit: ms(300),
        });
    Box::new(EngineController::start(config, Strength::Full))
}

struct World {
    server: FakeServer,
    bot: LichessBot,
    net: Net,
    dir: std::path::PathBuf,
}

impl Drop for World {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.dir);
    }
}

fn world() -> World {
    let clock = Arc::new(FakeClock::new(Timestamp::from_millis(1_794_398_400_000)));
    let server = FakeServer::with_clock(clock.clone());
    let dir = std::path::Path::new(env!("CARGO_TARGET_TMPDIR")).join(format!(
        "chess-studio-online-engine-{}-{:?}",
        std::process::id(),
        std::thread::current().id()
    ));
    let _ = std::fs::remove_dir_all(&dir);
    let keyring = Arc::new(MemoryKeyring::new());
    let vault = SecretVault::open(
        VaultConfig::new(dir.join("cofre"))
            .keyring(keyring)
            .clock(clock.clone()),
    )
    .unwrap();
    vault
        .store(
            LocalProfileId(1),
            ProviderId::Lichess,
            ENGINE_CONNECTION,
            Secret::new(TOKEN),
        )
        .unwrap();
    let config = NetConfig::in_memory()
        .clock(clock.clone())
        .jitter_seed(7)
        .vault(Arc::new(vault))
        .route("lichess.org", &server.base_url());
    let net = Net::open(config).unwrap();
    net.set_enabled(ProviderId::Lichess, true).unwrap();
    let bot = LichessBot::new(
        net.clone(),
        clock,
        ENGINE_CONNECTION,
        ConnectionRole::Engine,
    )
    .unwrap()
    .backoff(Backoff {
        base: ms(2000),
        max: ms(16000),
    });
    World {
        server,
        bot,
        net,
        dir,
    }
}

fn state(moves: &str) -> Value {
    json!({ "type": "gameState", "moves": moves, "wtime": 300_000, "btime": 300_000,
            "winc": 2000, "binc": 2000, "status": "started" })
}

fn full(moves: &str) -> Value {
    json!({
        "type": "gameFull", "id": GAME,
        "variant": { "key": "standard", "name": "Standard" },
        "speed": "blitz", "rated": false,
        "white": { "id": "motor", "name": "Motor", "title": "BOT", "rating": 2500 },
        "black": { "id": "salma", "name": "Salma", "rating": 1662 },
        "initialFen": "startpos",
        "clock": { "initial": 300_000, "increment": 2_000 },
        "state": state(moves),
    })
}

fn game_info() -> GameInfo {
    serde_json::from_value(json!({
        "fullId": format!("{GAME}91cB"), "gameId": GAME,
        "fen": "rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 0 1",
        "color": "white", "lastMove": "", "source": "friend",
        "status": { "id": 20, "name": "started" },
        "variant": { "key": "standard", "name": "Standard" },
        "speed": "blitz", "perf": "blitz", "rated": false, "hasMoved": false,
        "opponent": { "id": "salma", "username": "Salma", "rating": 1662 },
        "isMyTurn": true, "secondsLeft": 300, "id": GAME,
    }))
    .unwrap()
}

/// O que a `fake_uci` joga: o primeiro Lance legal em ordem alfabética.
fn first_alphabetical(moves: &[&str]) -> String {
    let mut position = CanonicalPosition::start();
    for uci in moves {
        position = position.play(uci.parse().unwrap()).unwrap();
    }
    assert_eq!(position.side_to_move(), Color::White);
    let mut legal: Vec<String> = position
        .legal_moves()
        .iter()
        .map(ToString::to_string)
        .collect();
    legal.sort();
    legal.remove(0)
}

fn move_posts(world: &World) -> Vec<String> {
    world
        .server
        .requests()
        .iter()
        .filter(|r| r.method == "POST" && r.path.contains("/move/"))
        .map(|r| r.path.rsplit('/').next().unwrap().to_owned())
        .collect()
}

struct Run {
    session: OnlineGameSession,
    notices: Vec<SessionNotice>,
}

impl Run {
    fn pump(&mut self) {
        self.notices.extend(self.session.poll().notices);
    }
}

fn start(world: &World, mode: &str) -> (Run, LiveFeed) {
    let feed = LiveFeed::new();
    world.server.route(
        &format!("/api/bot/game/stream/{GAME}"),
        FakeResponse::live(&feed),
    );
    let session = world
        .bot
        .open_game(&game_info(), engine(mode), EnginePolicy::default());
    assert!(feed.wait_connected(WAIT));
    (
        Run {
            session,
            notices: Vec::new(),
        },
        feed,
    )
}

#[test]
fn the_fake_uci_engine_plays_a_simulated_game_and_every_move_goes_out_once() {
    let world = world();
    let (mut run, feed) = start(&world, "normal");
    feed.push(&full("").to_string());

    // Três Lances da Engine (brancas), com a resposta do adversário (pretas) entre eles.
    let replies = ["a7a6", "b7b6"];
    let mut played: Vec<String> = Vec::new();
    for round in 0..3 {
        let expected = first_alphabetical(&played.iter().map(String::as_str).collect::<Vec<_>>());
        world.server.route(
            &format!("/api/bot/game/{GAME}/move/{expected}"),
            FakeResponse::json(r#"{"ok":true}"#),
        );
        assert!(
            eventually(|| {
                run.pump();
                move_posts(&world).len() == round + 1
            }),
            "a Engine falsa enviou o Lance {round}"
        );
        assert_eq!(move_posts(&world)[round], expected);
        played.push(expected);
        feed.push(&state(&played.join(" ")).to_string());
        if round < replies.len() {
            played.push(replies[round].to_owned());
            feed.push(&state(&played.join(" ")).to_string());
            assert!(eventually(|| {
                run.pump();
                run.session
                    .game()
                    .is_some_and(|g| g.snapshot().history.len() == played.len())
            }));
        }
    }
    assert_eq!(move_posts(&world).len(), 3, "um envio por Lance");
    assert!(!world.net.fair_play_guard().is_engaged());
    assert!(
        run.notices.iter().all(|n| !matches!(
            n,
            SessionNotice::EngineMoveRejected { .. } | SessionNotice::EngineFailed { .. }
        )),
        "{:?}",
        run.notices
    );
}

#[test]
fn an_engine_that_proposes_an_illegal_move_sends_nothing_and_the_game_is_resigned() {
    let world = world();
    world.server.route(
        &format!("/api/bot/game/{GAME}/resign"),
        FakeResponse::json(r#"{"ok":true}"#),
    );
    let (mut run, feed) = start(&world, "illegal");
    feed.push(&full("").to_string());
    assert!(eventually(|| {
        run.pump();
        world
            .server
            .requests()
            .iter()
            .any(|r| r.method == "POST" && r.path.ends_with("/resign"))
    }));
    assert!(move_posts(&world).is_empty(), "nenhum Lance ilegal saiu");
    let rejected = run
        .notices
        .iter()
        .filter(|n| matches!(n, SessionNotice::EngineMoveRejected { uci } if uci == "a1a1"))
        .count();
    assert_eq!(rejected, 2);
    assert!(run.session.engine_failed());
}
