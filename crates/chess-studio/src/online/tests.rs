//! A Partida online pela camada do app (#233): o serviço, o tabuleiro (`Session`), o banco e a
//! Trava de fair play juntos, como a interface faz a cada quadro. Servidor falso, relógio falso
//! e cofre falso: nenhuma chamada ao Lichess, nenhuma autorização real (CX-17). As respostas
//! são **sintéticas**, no formato documentado da Board API (as mesmas dos testes do
//! `chess-providers`), com apelidos inventados.

#![allow(clippy::unwrap_used)]

use super::service::{OnlineNotice, OnlinePlay, OutgoingState, StoreAccess, Unavailable};
use super::view::Pace;
use super::{Tab, after_frame};
use crate::analysis::{Analysis, Lock, Status};
use crate::explorer::{CloudDriver, CloudRow};
use crate::fair_play::{Computation, FairPlay, LockScope, Subject};
use crate::net_service::NetService;
use crate::session::{OnlineIntent, Session};
use chess_core::{Color, Square};
use chess_game::{EndReason, GameResult, PolicyKind};
use chess_input::{GestureEvent, Modifiers, PointerButton};
use chess_net::testing::{FakeResponse, FakeServer, LiveFeed};
use chess_net::{Clock, ConnectionId, FakeClock, Net, NetConfig, ProviderId, Request, Timestamp};
use chess_providers::{ChallengeSpec, DeclineReason, LinkStatus, OnlineTime, Seek};
use chess_store::{ConnectionRole, GameQuery, LibraryQuery, ProfileId, Store};
use serde_json::{Value, json};
use std::sync::Arc;
use std::time::{Duration, Instant};

/// 2026-11-11T12:00:00Z.
const T0: Timestamp = Timestamp::from_millis(1_794_398_400_000);
const GAME: &str = "qP8XaFtN";
const WAIT: Duration = Duration::from_secs(10);
const PLAYER: ConnectionId = ConnectionId(1);

/// Cofre falso: um token de teste para a Conexão registrada (nunca o cofre real).
#[derive(Debug)]
struct FakeVault;

impl chess_net::AuthorizationSource for FakeVault {
    fn authorization(
        &self,
        _profile: chess_net::LocalProfileId,
        provider: ProviderId,
        _connection: ConnectionId,
    ) -> Option<chess_net::Authorization> {
        Some(chess_net::Authorization::bearer(
            provider,
            &chess_net::Secret::new("lip_TOKEN_DE_TESTE_DA_PARTIDA_ONLINE"),
        ))
    }
}

struct Env {
    clock: Arc<FakeClock>,
    server: FakeServer,
    service: NetService,
    fair: FairPlay,
    play: OnlinePlay,
    session: Session,
    store: Store,
    profile: ProfileId,
    notices: Vec<OnlineNotice>,
    events: LiveFeed,
}

fn env() -> Env {
    env_with(true)
}

fn env_with(authorized: bool) -> Env {
    let clock = Arc::new(FakeClock::new(T0));
    let server = FakeServer::with_clock(clock.clone());
    env_on(clock, server, authorized)
}

/// O app sobre um relógio e um servidor já existentes (o app que fecha e reabre).
fn env_on(clock: Arc<FakeClock>, server: FakeServer, authorized: bool) -> Env {
    let mut config = NetConfig::in_memory()
        .clock(clock.clone())
        .jitter_seed(7)
        .vault(Arc::new(FakeVault));
    for host in [
        "lichess.org",
        "explorer.lichess.org",
        "tablebase.lichess.ovh",
    ] {
        config = config.route(host, &server.base_url());
    }
    let net = Net::open(config).unwrap();
    net.set_enabled(ProviderId::Lichess, true).unwrap();
    let service = NetService::from_net(net, clock.clone(), false);
    if authorized {
        service.set_authorized_connection(
            ProviderId::Lichess,
            ConnectionRole::Player,
            Some(PLAYER),
        );
    }
    let events = LiveFeed::new();
    server.route("/api/stream/event", FakeResponse::live(&events));
    let store = Store::open_in_memory().unwrap();
    let profile = store.default_profile().unwrap();
    Env {
        fair: service.fair_play().clone(),
        clock,
        server,
        service,
        play: OnlinePlay::new(),
        session: Session::new(),
        store,
        profile,
        notices: Vec::new(),
        events,
    }
}

impl Env {
    /// Um quadro do app, como `drive_online` faz.
    fn frame(&mut self) {
        self.play.follow_connection(&self.service);
        if let Some(remote) = self.session.online_remote_id().map(str::to_owned) {
            for intent in self.session.take_online_intents() {
                self.play.apply(&remote, intent);
            }
        }
        let frame = self.play.frame(Some(StoreAccess {
            store: &mut self.store,
            profile: self.profile,
        }));
        after_frame(&mut self.play, &mut self.session, frame, None);
        self.fair.follow_games(self.play.locked_games());
        self.notices.extend(self.play.take_notices());
    }

    fn until(&mut self, mut cond: impl FnMut(&Self) -> bool) -> bool {
        let start = Instant::now();
        while start.elapsed() < WAIT {
            self.frame();
            if cond(self) {
                return true;
            }
            std::thread::sleep(Duration::from_millis(5));
        }
        false
    }

    fn posts_to(&self, path: &str) -> usize {
        self.server
            .requests()
            .iter()
            .filter(|r| r.method == "POST" && r.path == path)
            .count()
    }

    fn plies(&self) -> usize {
        self.session.snapshot().history.len()
    }

    fn sans(&self) -> Vec<String> {
        self.session
            .snapshot()
            .history
            .iter()
            .map(|h| h.san.clone())
            .collect()
    }

    /// Clique na origem e no destino (dois toques), como no tabuleiro 3D ou 2D.
    fn tap(&mut self, from: &str, to: &str) {
        for at in [from, to] {
            let square: Square = at.parse().unwrap();
            self.session.gesture(GestureEvent::PointerDown {
                button: PointerButton::Primary,
                square: Some(square),
                modifiers: Modifiers::default(),
            });
            self.session.gesture(GestureEvent::PointerUp {
                square: Some(square),
            });
        }
    }

    fn stored_events(&self) -> usize {
        let id = self.session.online_store_id().expect("Partida no banco");
        self.store.load_game(id).unwrap().events().len()
    }
}

fn stream_path() -> String {
    format!("/api/board/game/stream/{GAME}")
}

fn move_path(uci: &str) -> String {
    format!("/api/board/game/{GAME}/move/{uci}")
}

fn ok() -> FakeResponse {
    FakeResponse::json(r#"{"ok":true}"#)
}

fn state(moves: &str, wtime: u64, btime: u64, status: &str) -> Value {
    json!({
        "type": "gameState",
        "moves": moves,
        "wtime": wtime,
        "btime": btime,
        "winc": 5000,
        "binc": 5000,
        "status": status,
    })
}

fn with(mut value: Value, key: &str, extra: Value) -> Value {
    value[key] = extra;
    value
}

fn player(id: &str, name: &str, rating: u32) -> Value {
    json!({ "id": id, "name": name, "title": null, "rating": rating })
}

/// `gameFull` sintético: rápida 10+5, o jogador local de brancas contra "Coruja_Azul".
fn full(state: Value) -> Value {
    json!({
        "type": "gameFull",
        "id": GAME,
        "variant": { "key": "standard", "name": "Standard", "short": "Std" },
        "speed": "rapid",
        "perf": { "name": "Rapid" },
        "rated": false,
        "createdAt": 1_794_398_400_000_i64,
        "white": player("voce_demo", "Voce_Demo", 1566),
        "black": player("coruja_azul", "Coruja_Azul", 1662),
        "initialFen": "startpos",
        "clock": { "initial": 600_000, "increment": 5_000 },
        "state": state,
    })
}

fn correspondence(state: Value) -> Value {
    let mut value = full(state);
    value.as_object_mut().unwrap().remove("clock");
    value["daysPerTurn"] = json!(3);
    value["speed"] = json!("correspondence");
    value
}

fn info(color: &str, opponent: Value, speed: &str) -> Value {
    json!({
        "fullId": format!("{GAME}91cB"),
        "gameId": GAME,
        "fen": "rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 0 1",
        "color": color,
        "lastMove": "",
        "source": "lobby",
        "status": { "id": 20, "name": "started" },
        "variant": { "key": "standard", "name": "Standard" },
        "speed": speed,
        "perf": speed,
        "rated": false,
        "hasMoved": false,
        "opponent": opponent,
        "isMyTurn": color == "white",
        "secondsLeft": 600,
        "compat": { "bot": false, "board": true },
        "id": GAME,
    })
}

fn human() -> Value {
    json!({ "id": "coruja_azul", "username": "Coruja_Azul", "rating": 1662 })
}

fn game_start(speed: &str) -> String {
    json!({ "type": "gameStart", "game": info("white", human(), speed) }).to_string()
}

/// Busca de adversário em tempo real, pareada; a Partida já no tabuleiro.
fn seek_and_start(env: &mut Env) -> LiveFeed {
    let seek = LiveFeed::new();
    env.server
        .route("/api/board/seek", FakeResponse::live(&seek));
    let feed = LiveFeed::new();
    env.server.route(&stream_path(), FakeResponse::live(&feed));
    env.frame();
    env.play
        .seek(&env.service, Seek::real_time(600, 5))
        .unwrap();
    assert!(
        env.events.wait_connected(WAIT),
        "o fluxo de eventos abre junto com a busca"
    );
    assert!(seek.wait_connected(WAIT));
    assert!(env.play.seek_run().is_some(), "estado de espera");
    env.events.push(&game_start("rapid"));
    seek.close();
    assert!(feed.wait_connected_after(|| env.frame(), WAIT));
    feed.push(&full(state("", 600_000, 600_000, "started")).to_string());
    assert!(
        env.until(|e| e.session.is_online()),
        "a Partida vai ao tabuleiro"
    );
    feed
}

trait WaitAfter {
    fn wait_connected_after(&self, pump: impl FnMut(), timeout: Duration) -> bool;
}

impl WaitAfter for LiveFeed {
    fn wait_connected_after(&self, mut pump: impl FnMut(), timeout: Duration) -> bool {
        let start = Instant::now();
        while start.elapsed() < timeout {
            pump();
            if self.is_connected() {
                return true;
            }
            std::thread::sleep(Duration::from_millis(5));
        }
        false
    }
}

/// Lance local pelo tabuleiro, confirmado pelo Lichess, e a resposta do adversário.
fn exchange(env: &mut Env, feed: &LiveFeed, mine: &str, so_far: &mut String, reply: &str) {
    env.server.route(&move_path(mine), ok());
    let plies = env.plies();
    env.tap(&mine[..2], &mine[2..4]);
    assert!(
        env.until(|e| e.posts_to(&move_path(mine)) == 1),
        "o Lance sai uma vez"
    );
    assert!(
        env.session.snapshot().made_move.is_some(),
        "a intenção enviada aparece no tabuleiro, fora do log"
    );
    assert_eq!(env.plies(), plies, "nada no histórico antes da confirmação");
    for uci in [mine, reply] {
        if !so_far.is_empty() {
            so_far.push(' ');
        }
        so_far.push_str(uci);
        feed.push(&state(so_far, 590_000, 595_000, "started").to_string());
        let want = so_far.split(' ').count();
        assert!(
            env.until(|e| e.plies() == want),
            "esperava {want}: {:?}",
            env.sans()
        );
    }
    assert!(env.session.snapshot().made_move.is_none());
}

fn analysis_with_fake_engine() -> Analysis {
    let mut analysis = Analysis::default();
    analysis.set_default_engine(Some(std::path::PathBuf::from("/nao/existe/engine")));
    analysis.set_enabled(true);
    analysis
}

// ---- Partida completa pelo app ---------------------------------------------------------------

#[test]
fn a_full_game_from_seek_to_checkmate_through_the_board_and_into_the_library() {
    let mut env = env();
    let feed = seek_and_start(&mut env);
    assert_eq!(env.session.snapshot().policy, PolicyKind::Online);
    assert_eq!(env.session.user_side(), Color::White);
    assert!(env.service.net().fair_play_guard().is_engaged());
    assert!(
        env.session.competitive_in_progress(),
        "Painéis que calculam fechados"
    );
    let mut so_far = String::new();
    exchange(&mut env, &feed, "e2e4", &mut so_far, "e7e5");
    exchange(&mut env, &feed, "f1c4", &mut so_far, "b8c6");
    exchange(&mut env, &feed, "d1h5", &mut so_far, "g8f6");
    // Relógios do Provedor no Painel e no relógio 3D (a mesma leitura da Session).
    let clock = env.session.snapshot().clock.unwrap();
    assert_eq!((clock.white_ms, clock.black_ms), (590_000, 595_000));
    assert!(env.session.clock_reading().is_some());
    // Mate: o Lichess diz o fim.
    env.server.route(&move_path("h5f7"), ok());
    env.tap("h5", "f7");
    assert!(env.until(|e| e.posts_to(&move_path("h5f7")) == 1));
    assert!(
        env.service.net().fair_play_guard().is_engaged(),
        "trava até o fim confirmado"
    );
    let mate = with(
        state(&format!("{so_far} h5f7"), 589_000, 580_000, "mate"),
        "winner",
        json!("white"),
    );
    feed.push(&mate.to_string());
    assert!(env.until(|e| e.session.snapshot().is_over()));
    let outcome = env.session.snapshot().outcome.unwrap();
    assert_eq!(
        (outcome.result, outcome.reason),
        (GameResult::WhiteWins, EndReason::Checkmate)
    );
    assert!(env.until(|e| !e.service.net().fair_play_guard().is_engaged()));
    assert!(!env.fair.any(), "a trava caiu com o fim confirmado");
    assert!(
        !env.session.competitive_in_progress(),
        "só agora a análise fica disponível"
    );
    // O banco tem exatamente o log do controlador, com a Origem.
    let id = env.session.online_store_id().unwrap();
    let stored = env.store.load_game(id).unwrap();
    assert_eq!(stored.events().len(), env.session.game().events().len());
    assert_eq!(stored.snapshot(), env.session.game().snapshot());
    let summary = env
        .store
        .list_games(&GameQuery::default())
        .unwrap()
        .into_iter()
        .find(|g| g.id == id)
        .unwrap();
    assert!(summary.finished);
    assert_eq!(summary.metadata["online"]["remote_id"], GAME);
    assert_eq!(summary.metadata["online"]["provider"], "lichess");
    let page = env
        .store
        .search_library(&LibraryQuery {
            profile: Some(env.profile),
            ..LibraryQuery::default()
        })
        .unwrap();
    let entry = page.entries.iter().find(|e| e.ref_id == id.0).unwrap();
    assert_eq!(entry.white.as_deref(), Some("Voce_Demo"));
    assert_eq!(entry.black.as_deref(), Some("Coruja_Azul"));
    assert_eq!(entry.result, "1-0");
    assert!(
        entry
            .event
            .as_deref()
            .unwrap()
            .contains("lichess.org/qP8XaFtN")
    );
    // Agora a análise sobe.
    let mut analysis = analysis_with_fake_engine();
    analysis.drive(&env.session, false, &env.fair);
    assert!(analysis.has_engine());
}

#[test]
fn the_end_by_time_and_by_resignation_only_comes_from_the_provider() {
    for (status, winner, reason) in [
        ("outoftime", "black", EndReason::Timeout),
        ("resign", "black", EndReason::Resignation),
    ] {
        let mut env = env();
        let feed = seek_and_start(&mut env);
        let mut so_far = String::new();
        exchange(&mut env, &feed, "e2e4", &mut so_far, "e7e5");
        if status == "resign" {
            let resign = format!("/api/board/game/{GAME}/resign");
            env.server.route(&resign, ok());
            env.session.resign_side_to_move();
            assert!(env.until(|e| e.posts_to(&resign) == 1));
            // Pedir abandono não encerra nada: a trava e a Partida seguem até o retrato.
            for _ in 0..10 {
                env.frame();
            }
            assert!(!env.session.snapshot().is_over());
            assert!(env.fair.everything_locked());
        } else {
            // O relógio local nunca dá bandeira na Partida online.
            env.session.tick();
            assert!(!env.session.snapshot().is_over());
        }
        let end = with(state(&so_far, 0, 500_000, status), "winner", json!(winner));
        feed.push(&end.to_string());
        assert!(env.until(|e| e.session.snapshot().is_over()), "{status}");
        let outcome = env.session.snapshot().outcome.unwrap();
        assert_eq!(outcome.result, GameResult::BlackWins);
        assert_eq!(outcome.reason, reason);
        assert!(env.until(|e| !e.fair.any()));
        assert_eq!(env.stored_events(), env.session.game().events().len());
    }
}

#[test]
fn a_refused_move_goes_back_and_never_reaches_the_log() {
    let mut env = env();
    let feed = seek_and_start(&mut env);
    env.server
        .route(&move_path("e2e4"), FakeResponse::status(400));
    let before = env.session.game().events().len();
    env.tap("e2", "e4");
    assert!(env.until(|e| {
        e.notices
            .iter()
            .any(|n| matches!(n, OnlineNotice::MoveRefused { .. }))
    }));
    assert!(env.session.snapshot().made_move.is_none(), "a peça volta");
    assert_eq!(env.session.game().events().len(), before, "nada no log");
    assert_eq!(env.stored_events(), before);
    assert_eq!(env.posts_to(&move_path("e2e4")), 1, "nunca reenviado");
    // O jogador joga de novo.
    env.server.route(&move_path("d2d4"), ok());
    env.tap("d2", "d4");
    assert!(env.until(|e| e.posts_to(&move_path("d2d4")) == 1));
    feed.push(&state("d2d4", 600_000, 600_000, "started").to_string());
    assert!(env.until(|e| e.plies() == 1));
    assert_eq!(env.sans(), ["d4"]);
}

#[test]
fn a_drop_reconnects_without_resending_and_the_result_that_came_meanwhile_shows() {
    let mut env = env();
    let first = LiveFeed::new();
    let second = LiveFeed::new();
    env.server.route_seq(
        &stream_path(),
        vec![FakeResponse::live(&first), FakeResponse::live(&second)],
    );
    let seek = LiveFeed::new();
    env.server
        .route("/api/board/seek", FakeResponse::live(&seek));
    env.frame();
    env.play
        .seek(&env.service, Seek::real_time(600, 5))
        .unwrap();
    assert!(env.events.wait_connected(WAIT));
    env.events.push(&game_start("rapid"));
    assert!(first.wait_connected_after(|| env.frame(), WAIT));
    first.push(&full(state("e2e4 e7e5", 600_000, 600_000, "started")).to_string());
    assert!(env.until(|e| e.plies() == 2));
    // A queda no meio do envio: o Lance não é repetido; o estado é consultado.
    env.server
        .route(&move_path("g1f3"), FakeResponse::drop_connection());
    env.tap("g1", "f3");
    assert!(env.until(|e| e.posts_to(&move_path("g1f3")) == 1));
    first.close();
    assert!(env.until(|e| {
        let live = e.play.live(GAME).map(super::service::Live::link);
        matches!(live, Some(LinkStatus::Retrying { .. })) || second.is_connected()
    }));
    env.clock.advance(Duration::from_secs(30));
    assert!(
        second.wait_connected_after(|| env.frame(), WAIT),
        "reconecta"
    );
    // Enquanto estava sem fluxo, a Partida terminou: o retrato completo traz o fim.
    let ended = with(
        state(
            "e2e4 e7e5 g1f3 b8c6 f1c4 g8f6 f3g5 d8e7 g5f7 e7f7",
            500_000,
            500_000,
            "resign",
        ),
        "winner",
        json!("black"),
    );
    second.push(&full(ended).to_string());
    assert!(env.until(|e| e.session.snapshot().is_over()));
    assert_eq!(env.plies(), 10);
    assert_eq!(
        env.posts_to(&move_path("g1f3")),
        1,
        "nada de reenvio às cegas"
    );
    assert!(env.until(|e| !e.fair.any()));
    assert_eq!(env.stored_events(), env.session.game().events().len());
}

// ---- Correspondência --------------------------------------------------------------------------

#[test]
fn correspondence_survives_closing_and_reopening_the_app_and_continues_by_the_snapshot() {
    let dir = std::env::temp_dir().join(format!("chess-studio-online-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("partidas.db");
    let first = LiveFeed::new();
    let second = LiveFeed::new();
    // Primeira abertura do app.
    let (clock, server) = {
        let mut env = env();
        env.store = Store::open(&path).unwrap();
        env.profile = env.store.default_profile().unwrap();
        env.server.route_seq(
            &stream_path(),
            vec![FakeResponse::live(&first), FakeResponse::live(&second)],
        );
        env.server
            .route("/api/board/seek", FakeResponse::json(r#"{"id":"sk9Tq2"}"#));
        env.frame();
        env.play
            .seek(&env.service, Seek::correspondence(3))
            .unwrap();
        assert!(env.events.wait_connected(WAIT));
        assert!(env.until(|e| e.play.seek_run().is_none_or(|r| r.posted)));
        env.events.push(&game_start("correspondence"));
        assert!(first.wait_connected_after(|| env.frame(), WAIT));
        first.push(&correspondence(state("", 0, 0, "started")).to_string());
        assert!(env.until(|e| e.session.is_online()));
        let mut so_far = String::new();
        exchange(&mut env, &first, "e2e4", &mut so_far, "e7e5");
        // O jogador faz o Lance dele; a vez passa ao adversário.
        env.server.route(&move_path("g1f3"), ok());
        env.tap("g1", "f3");
        assert!(env.until(|e| e.posts_to(&move_path("g1f3")) == 1));
        first.push(&state("e2e4 e7e5 g1f3", 0, 0, "started").to_string());
        assert!(env.until(|e| e.plies() == 3));
        assert!(env.fair.any());
        // O app fecha: sessão, serviço e tabuleiro somem; o banco fica.
        (env.clock, env.server)
    };
    // Dias depois, o app abre: nenhuma rede sem ação do jogador, e a trava já vale pela
    // Partida do banco (só para ela e as Posições dela).
    clock.advance(Duration::from_secs(2 * 86_400));
    let requests_before = server.requests().len();
    let mut env = env_on(clock, server, true);
    env.store = Store::open(&path).unwrap();
    env.profile = env.store.default_profile().unwrap();
    for _ in 0..5 {
        env.frame();
    }
    assert_eq!(
        env.server.requests().len(),
        requests_before,
        "nada sai ao abrir"
    );
    assert_eq!(env.play.stored_in_progress(), 1);
    assert!(env.fair.any() && !env.fair.everything_locked());
    let rows = env.play.playing_rows();
    assert_eq!(rows.len(), 1);
    assert!(!rows[0].my_turn, "a vez é do adversário no log local");
    // Atualizar: a lista do Lichess diz de quem é a vez e quanto falta.
    let playing = json!({ "nowPlaying": [with(
        with(info("white", human(), "correspondence"), "isMyTurn", json!(true)),
        "secondsLeft",
        json!(172_800),
    )] });
    env.server.route(
        "/api/account/playing",
        FakeResponse::json(&playing.to_string()),
    );
    env.play.refresh_playing(&env.service).unwrap();
    assert!(env.until(|e| e.play.refreshed_at().is_some()));
    let rows = env.play.playing_rows();
    assert!(rows[0].my_turn);
    assert_eq!(rows[0].seconds_left, Some(172_800));
    // Abrir e continuar: o retrato traz o Lance que o adversário fez com o app fechado.
    env.play.open(&env.service, GAME).unwrap();
    assert!(second.wait_connected_after(|| env.frame(), WAIT));
    second.push(&correspondence(state("e2e4 e7e5 g1f3 b8c6", 0, 0, "started")).to_string());
    assert!(env.until(|e| e.plies() == 4));
    assert_eq!(env.sans(), ["e4", "e5", "Nf3", "Nc6"]);
    let id = env.session.online_store_id().unwrap();
    let rows = env
        .store
        .list_games(&GameQuery::default())
        .unwrap()
        .into_iter()
        .filter(|g| g.metadata.get("online").is_some())
        .count();
    assert_eq!(rows, 1, "a mesma Partida, sem segunda linha no banco");
    assert_eq!(
        env.store.load_game(id).unwrap().snapshot(),
        env.session.game().snapshot()
    );
    let _ = std::fs::remove_dir_all(&dir);
}

// ---- Engine do Lichess, desafios e variantes ---------------------------------------------------

#[test]
fn against_the_lichess_engine_the_lock_never_engages() {
    let mut env = env();
    let feed = LiveFeed::new();
    env.server.route(&stream_path(), FakeResponse::live(&feed));
    env.server.route(
        "/api/challenge/ai",
        FakeResponse::json(&json!({ "id": GAME }).to_string()),
    );
    env.frame();
    let spec = ChallengeSpec {
        time: Pace::Rapid10_5.time(3),
        rated: false,
        color: Some(Color::White),
    };
    env.play.play_engine(&env.service, 3, spec).unwrap();
    assert!(env.until(|e| e.posts_to("/api/challenge/ai") == 1));
    let ai = json!({ "id": null, "username": "A.I. level 3", "rating": null, "ai": 3 });
    env.events
        .push(&json!({ "type": "gameStart", "game": info("white", ai, "rapid") }).to_string());
    assert!(feed.wait_connected_after(|| env.frame(), WAIT));
    let mut value = full(state("", 600_000, 600_000, "started"));
    value["black"] = json!({ "aiLevel": 3 });
    feed.push(&value.to_string());
    assert!(env.until(|e| e.session.is_online()));
    assert!(!env.service.net().fair_play_guard().is_engaged());
    assert!(!env.fair.any());
    assert!(!env.session.competitive_in_progress());
    let mut analysis = analysis_with_fake_engine();
    analysis.drive(&env.session, false, &env.fair);
    assert!(
        analysis.has_engine(),
        "contra a Engine do Lichess a análise roda"
    );
}

fn challenge_json(id: &str, variant: &str, speed: &str) -> String {
    json!({
        "type": "challenge",
        "challenge": {
            "id": id,
            "url": format!("https://lichess.org/{id}"),
            "status": "created",
            "challenger": { "id": "torredemadeira", "name": "TorreDeMadeira", "rating": 1720 },
            "destUser": { "id": "voce_demo", "name": "Voce_Demo", "rating": 1566 },
            "variant": { "key": variant, "name": variant, "short": variant },
            "rated": true,
            "speed": speed,
            "timeControl": { "type": "clock", "limit": 300, "increment": 3, "show": "5+3" },
            "color": "random",
            "perf": { "icon": "", "name": "Blitz" },
        }
    })
    .to_string()
}

#[test]
fn received_challenges_are_declined_with_a_reason_or_accepted_and_the_game_starts() {
    let mut env = env();
    env.frame();
    env.play.listen(&env.service).unwrap();
    assert!(env.events.wait_connected(WAIT));
    env.events
        .push(&challenge_json("Hq3vT8aZ", "standard", "blitz"));
    assert!(env.until(|e| e.play.incoming.len() == 1));
    assert!(
        env.notices
            .iter()
            .any(|n| matches!(n, OnlineNotice::ChallengeReceived { .. }))
    );
    // Recusar com motivo.
    let decline = "/api/challenge/Hq3vT8aZ/decline";
    env.server.route(decline, ok());
    env.play.decline("Hq3vT8aZ", DeclineReason::TooFast);
    assert!(env.until(|e| e.posts_to(decline) == 1));
    let body = env
        .server
        .requests()
        .into_iter()
        .find(|r| r.path == decline)
        .map(|r| String::from_utf8(r.body).unwrap())
        .unwrap();
    assert!(body.contains("reason=tooFast"), "{body}");
    assert!(env.play.incoming.is_empty());
    // Aceitar outro (blitz por desafio direto): a Partida começa e vai ao tabuleiro.
    env.events
        .push(&challenge_json("Bv5nM1qX", "standard", "blitz"));
    assert!(env.until(|e| e.play.incoming.len() == 1));
    let accept = "/api/challenge/Bv5nM1qX/accept";
    env.server.route(accept, ok());
    let feed = LiveFeed::new();
    env.server.route(&stream_path(), FakeResponse::live(&feed));
    env.play.accept(&env.service, "Bv5nM1qX").unwrap();
    assert!(env.until(|e| e.posts_to(accept) == 1));
    env.events.push(&game_start("blitz"));
    assert!(feed.wait_connected_after(|| env.frame(), WAIT));
    feed.push(&full(state("", 300_000, 300_000, "started")).to_string());
    assert!(env.until(|e| e.session.is_online()));
}

#[test]
fn a_challenge_by_name_waits_and_can_be_canceled() {
    let mut env = env();
    env.frame();
    let created = json!({
        "id": "Zk2pR7wQ",
        "url": "https://lichess.org/Zk2pR7wQ",
        "status": "created",
        "challenger": { "id": "voce_demo", "name": "Voce_Demo", "rating": 1566 },
        "destUser": { "id": "peao_passado", "name": "Peao_Passado", "rating": 1490 },
        "variant": { "key": "standard", "name": "Standard", "short": "Std" },
        "rated": false,
        "speed": "blitz",
        "timeControl": { "type": "clock", "limit": 180, "increment": 2, "show": "3+2" },
        "color": "white",
        "perf": { "icon": "", "name": "Blitz" },
    });
    env.server.route(
        "/api/challenge/Peao_Passado",
        FakeResponse::json(&created.to_string()),
    );
    env.server.route("/api/challenge/Zk2pR7wQ/cancel", ok());
    let spec = ChallengeSpec {
        time: Pace::Blitz3_2.time(3),
        rated: false,
        color: Some(Color::White),
    };
    env.play
        .challenge(&env.service, "Peao_Passado", spec)
        .unwrap();
    assert!(env.until(|e| {
        e.play
            .outgoing
            .first()
            .is_some_and(|o| o.state == OutgoingState::Waiting)
    }));
    let body = env
        .server
        .requests()
        .into_iter()
        .find(|r| r.path == "/api/challenge/Peao_Passado")
        .map(|r| String::from_utf8(r.body).unwrap())
        .unwrap();
    assert!(
        body.contains("clock.limit=180") && body.contains("variant=standard"),
        "{body}"
    );
    env.play.cancel_challenge(0);
    assert!(env.until(|e| e.posts_to("/api/challenge/Zk2pR7wQ/cancel") == 1));
    assert_eq!(env.play.outgoing[0].state, OutgoingState::Canceled);
}

#[test]
fn a_non_standard_variant_is_refused_before_anything_starts() {
    let mut env = env();
    env.frame();
    env.play.listen(&env.service).unwrap();
    assert!(env.events.wait_connected(WAIT));
    env.events
        .push(&challenge_json("C960aaaa", "chess960", "rapid"));
    assert!(env.until(|e| e.play.incoming.len() == 1));
    env.play.accept(&env.service, "C960aaaa").unwrap();
    assert!(env.until(|e| {
        e.notices
            .iter()
            .any(|n| matches!(n, OnlineNotice::Failed(_)))
    }));
    assert_eq!(
        env.posts_to("/api/challenge/C960aaaa/accept"),
        0,
        "recusado antes da rede"
    );
    // Uma Partida em variante começada no site é ignorada: nenhuma sessão, nada no banco.
    let mut start = info("white", human(), "rapid");
    start["variant"] = json!({ "key": "chess960", "name": "Chess960" });
    env.events
        .push(&json!({ "type": "gameStart", "game": start }).to_string());
    assert!(env.until(|e| {
        e.notices
            .iter()
            .any(|n| matches!(n, OnlineNotice::VariantIgnored { .. }))
    }));
    assert!(env.play.games().is_empty());
    assert!(!env.session.is_online());
}

#[test]
fn only_the_authorized_player_connection_plays_never_the_engine_one() {
    let mut env = env_with(false);
    env.frame();
    assert_eq!(
        env.play.seek(&env.service, Seek::real_time(600, 5)),
        Err(Unavailable::NoConnection)
    );
    env.service.set_authorized_connection(
        ProviderId::Lichess,
        ConnectionRole::Engine,
        Some(ConnectionId(9)),
    );
    env.frame();
    assert_eq!(
        env.play.seek(&env.service, Seek::real_time(600, 5)),
        Err(Unavailable::NoConnection),
        "a Conexão de Engine nunca joga como pessoa"
    );
    assert!(env.server.requests().is_empty(), "nada saiu");
    // Blitz não se busca: só por desafio direto (recusado antes da rede).
    env.service.set_authorized_connection(
        ProviderId::Lichess,
        ConnectionRole::Player,
        Some(PLAYER),
    );
    env.frame();
    assert!(matches!(
        env.play.seek(&env.service, Seek::real_time(180, 2)),
        Err(Unavailable::Board(chess_providers::BoardError::TooFast))
    ));
}

#[test]
fn the_board_never_moves_out_of_turn_nor_while_a_move_is_being_sent() {
    let mut env = env();
    let feed = seek_and_start(&mut env);
    let mut so_far = String::new();
    exchange(&mut env, &feed, "e2e4", &mut so_far, "e7e5");
    // Lance enviado, sem resposta: um segundo gesto não sai.
    env.server.route(&move_path("g1f3"), FakeResponse::stall());
    env.tap("g1", "f3");
    assert!(env.until(|e| e.posts_to(&move_path("g1f3")) == 1));
    env.tap("d2", "d4");
    for _ in 0..10 {
        env.frame();
    }
    assert_eq!(env.posts_to(&move_path("d2d4")), 0);
    assert!(env.session.take_online_intents().is_empty());
    let _ = feed;
}

// ---- Trava de fair play nos serviços que calculam --------------------------------------------

/// Partida em tempo real em andamento no tabuleiro.
fn live_game() -> (Env, LiveFeed) {
    let mut env = env();
    let feed = seek_and_start(&mut env);
    let mut so_far = String::new();
    exchange(&mut env, &feed, "e2e4", &mut so_far, "e7e5");
    (env, feed)
}

#[test]
fn the_engine_analysis_refuses_during_the_online_game_even_in_review() {
    let (mut env, _feed) = live_game();
    let mut analysis = analysis_with_fake_engine();
    analysis.drive(&env.session, false, &env.fair);
    assert_eq!(analysis.status(), &Status::Locked(Lock::Competitive));
    assert!(!analysis.has_engine(), "nenhum processo de Engine sobe");
    // Revisar Lances anteriores não libera (revisão).
    env.session.set_review_ply(Some(1));
    analysis.set_enabled(true);
    analysis.drive(&env.session, false, &env.fair);
    assert!(matches!(analysis.status(), Status::Locked(_)));
    assert!(!analysis.has_engine());
    // Mesmo com a Partida online fora do tabuleiro (outra Partida aberta), a trava global
    // vale enquanto a de tempo real não terminar.
    env.session.new_game();
    analysis.set_enabled(true);
    analysis.drive(&env.session, false, &env.fair);
    assert_eq!(analysis.status(), &Status::Locked(Lock::FairPlay));
    assert!(!analysis.has_engine());
    // A barra de avaliação some junto (ela só aparece com a análise de pé).
    assert!(!crate::eval_bar::is_visible(
        analysis.is_enabled(),
        true,
        matches!(analysis.status(), Status::Locked(_))
    ));
}

#[test]
fn the_hint_and_every_calculating_aid_are_refused_while_rule_aids_stay() {
    let (env, _feed) = live_game();
    let allowed = crate::hud_policy::effective(&env.session, &env.fair);
    assert!(!allowed.hints && !allowed.engine_line && !allowed.suggestion_arrows);
    assert!(!allowed.attacked_squares && !allowed.threats);
    assert!(
        allowed.destinations && allowed.last_move && allowed.check,
        "Auxílios de regra ficam"
    );
    let settings = crate::hud::HudSettings {
        enabled: true,
        hint: true,
        engine_arrow: true,
        threatened: true,
        undefended: true,
        attacked: crate::hud::AttackedSide::White,
        ..crate::hud::HudSettings::default()
    };
    let marks = crate::hud::compute_hud(
        &settings,
        &allowed,
        crate::hud::HudShown {
            position: &env.session.snapshot().position,
            last_move: None,
        },
        &crate::hud::HudAnalysis::default(),
    );
    assert!(marks.arrows.is_empty() && marks.attacked.is_empty() && marks.threatened.is_empty());
}

#[test]
fn cloud_eval_endgames_explorer_and_remote_engine_send_nothing() {
    let (env, _feed) = live_game();
    let before = env.server.requests().len();
    let base = env.session.snapshot().position.clone();
    let subject = Subject::shown(&env.session).unwrap();
    // Avaliação em cache: o serviço recusa antes da rede.
    let mut cloud = CloudDriver::default();
    for ms in [0, 400, 900] {
        let row = cloud.drive(
            &base,
            Instant::now() + Duration::from_millis(ms),
            env.clock.now(),
            None,
            Some(&subject),
            &env.service,
        );
        assert_eq!(row, CloudRow::Notice(crate::explorer::Notice::FairPlay));
    }
    assert_eq!(cloud.requests_started(), 0);
    // Explorador (online) e finais: nada pedido.
    for computation in [
        Computation::Explorer,
        Computation::Endgames,
        Computation::RemoteEngine,
    ] {
        assert_eq!(
            env.fair.check(computation, &subject).unwrap_err().scope,
            LockScope::Everything
        );
    }
    // E o gate da rede, por baixo, recusa tudo o que calcula (Engine remota inclusive).
    for url in [
        "https://lichess.org/api/cloud-eval",
        "https://explorer.lichess.org/lichess",
        "https://tablebase.lichess.ovh/standard",
    ] {
        let err = env
            .service
            .net()
            .fetch(Request::get(ProviderId::Lichess, url).position("fen", &base.to_fen()))
            .wait()
            .unwrap_err();
        assert!(
            matches!(err, chess_net::NetError::FairPlayLocked { .. }),
            "{url}: {err:?}"
        );
    }
    assert_eq!(
        env.server.requests().len(),
        before,
        "nenhum pedido que calcula saiu"
    );
}

#[test]
fn the_local_library_statistics_refuse_for_the_online_game() {
    let (env, _feed) = live_game();
    let subject = Subject::shown(&env.session).unwrap();
    let fen = env.session.snapshot().position.to_fen();
    let refused =
        crate::fair_play::opening_stats(&env.fair, &subject, &env.store, env.profile, &fen);
    assert!(refused.is_err());
}

#[test]
fn a_correspondence_game_locks_only_itself_and_its_positions() {
    let mut env = env();
    env.server
        .route("/api/board/seek", FakeResponse::json(r#"{"id":"sk9Tq2"}"#));
    let feed = LiveFeed::new();
    env.server.route(&stream_path(), FakeResponse::live(&feed));
    env.frame();
    env.play
        .seek(&env.service, Seek::correspondence(3))
        .unwrap();
    assert!(env.events.wait_connected(WAIT));
    env.events.push(&game_start("correspondence"));
    assert!(feed.wait_connected_after(|| env.frame(), WAIT));
    feed.push(&correspondence(state("e2e4 e7e5 g1f3", 0, 0, "started")).to_string());
    assert!(env.until(|e| e.plies() == 3));
    let corr_position = env.session.snapshot().position.clone();
    // Com a Partida no tabuleiro (sessão aberta): tudo travado.
    assert!(env.fair.everything_locked());
    // O jogador vai estudar outra coisa: a sessão da correspondência fecha.
    env.session.new_game();
    assert!(env.until(|e| !e.service.net().fair_play_guard().is_engaged()));
    env.frame();
    env.frame();
    assert!(env.fair.any(), "a trava continua pela Partida do banco");
    // Outra Posição: livre (com a Biblioteca, a análise e a nuvem).
    let other = chess_core::CanonicalPosition::from_fen(
        "rnbqkb1r/pppppppp/5n2/8/3P4/8/PPP1PPPP/RNBQKBNR w KQkq - 1 2",
    )
    .unwrap();
    assert_eq!(
        env.fair
            .check(Computation::CloudEval, &Subject::position(&other)),
        Ok(())
    );
    let mut analysis = analysis_with_fake_engine();
    analysis.drive(&env.session, false, &env.fair);
    assert!(
        analysis.has_engine(),
        "estudar outra Partida continua possível"
    );
    // A Posição da correspondência num tabuleiro de estudo: recusada em todo serviço.
    env.session.study_from_fen(&corr_position.to_fen()).unwrap();
    let mut analysis = analysis_with_fake_engine();
    analysis.drive(&env.session, false, &env.fair);
    assert_eq!(analysis.status(), &Status::Locked(Lock::FairPlay));
    let subject = Subject::shown(&env.session).unwrap();
    for computation in Computation::ALL {
        assert_eq!(
            env.fair.check(computation, &subject).unwrap_err().scope,
            LockScope::ThisPosition
        );
    }
    let before = env.server.requests().len();
    let mut cloud = CloudDriver::default();
    for ms in [0, 400, 900] {
        cloud.drive(
            &corr_position,
            Instant::now() + Duration::from_millis(ms),
            env.clock.now(),
            None,
            Some(&subject),
            &env.service,
        );
    }
    assert_eq!(cloud.requests_started(), 0);
    assert_eq!(env.server.requests().len(), before);
}

#[test]
fn the_online_dialog_tabs_and_paces_map_to_the_board_api() {
    assert_eq!(
        Pace::Rapid10_5.time(3),
        OnlineTime::RealTime {
            base_s: 600,
            increment_s: 5
        }
    );
    assert_eq!(
        Pace::Correspondence.time(7),
        OnlineTime::Correspondence { days: 7 }
    );
    let mut ui = super::OnlineUi::default();
    ui.open_at(Tab::Playing);
    assert!(ui.open && ui.tab == Tab::Playing);
}

#[test]
fn the_board_requests_go_to_the_session_and_nothing_changes_until_confirmed() {
    let (mut env, _feed) = live_game();
    let draw = format!("/api/board/game/{GAME}/draw/yes");
    env.server.route(&draw, ok());
    env.session.online_request(OnlineIntent::OfferDraw);
    assert!(env.until(|e| e.posts_to(&draw) == 1));
    assert!(
        env.session.snapshot().pending_draw_offer.is_none(),
        "só com o retrato"
    );
}
