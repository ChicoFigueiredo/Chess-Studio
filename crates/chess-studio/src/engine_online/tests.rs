//! A Engine online pela camada do app (#252): o serviço, a política de desafios, o banco e a
//! Conexão de Engine juntos, como a tela os usa a cada quadro. Servidor falso, relógio falso e
//! cofre falso: nenhuma chamada ao Lichess, nenhuma autorização real (CX-17). As respostas são
//! **sintéticas**, no formato documentado da Bot API (as mesmas dos testes do
//! `chess-providers`), com apelidos inventados. A Engine é uma dupla roteirizada.

#![allow(clippy::unwrap_used)]

use super::service::{
    Activity, Conversion, EngineFactory, EngineOnline, EngineSpec, RunState, Settings, StopReason,
    Unavailable,
};
use super::view::{ConversionView, Form, ViewModel};
use super::{EngineOnlineUi, apply_intent, view::Intent};
use crate::net_service::NetService;
use crate::online::service::StoreAccess;
use chess_net::testing::{FakeResponse, FakeServer, LiveFeed};
use chess_net::{ConnectionId, FakeClock, Net, NetConfig, ProviderId, Timestamp};
use chess_providers::{ChallengePolicy, DeclineReason, IrreversibleConfirmation, MoveEngine};
use chess_store::{ConnectionRole, GameQuery, ProfileId, Store};
use chess_uci::{ControllerEvent, SearchRequest};
use serde_json::{Value, json};
use std::collections::VecDeque;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

/// 2026-11-11T12:00:00Z.
pub(super) const T0: Timestamp = Timestamp::from_millis(1_794_398_400_000);
pub(super) const GAME: &str = "En9ineQ1";
pub(super) const WAIT: Duration = Duration::from_secs(10);
const ENGINE_CONNECTION: ConnectionId = ConnectionId(2);

/// Cofre falso: um token de teste para a Conexão de Engine (nunca o cofre real).
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
            &chess_net::Secret::new("lip_TOKEN_DA_ENGINE_DE_TESTE_DO_APP"),
        ))
    }
}

// ---- A Engine roteirizada -------------------------------------------------------------------

#[derive(Default)]
pub(super) struct Shared {
    script: VecDeque<String>,
    requests: usize,
}

struct Scripted {
    shared: Arc<Mutex<Shared>>,
    queue: VecDeque<ControllerEvent>,
    next: u64,
}

impl MoveEngine for Scripted {
    fn search(&mut self, _request: SearchRequest) -> u64 {
        let id = self.next;
        self.next += 1;
        let mut shared = self.shared.lock().unwrap();
        shared.requests += 1;
        if let Some(uci) = shared.script.pop_front() {
            self.queue.push_back(ControllerEvent::Move { id, uci });
        }
        id
    }

    fn cancel(&mut self) {
        self.queue.clear();
    }

    fn poll(&mut self) -> Option<ControllerEvent> {
        self.queue.pop_front()
    }
}

pub(super) fn scripted(moves: &[&str]) -> (EngineFactory, Arc<Mutex<Shared>>) {
    let shared = Arc::new(Mutex::new(Shared {
        script: moves.iter().map(|m| (*m).to_owned()).collect(),
        ..Shared::default()
    }));
    let handle = shared.clone();
    (
        Box::new(move |_| {
            Ok(Box::new(Scripted {
                shared: handle.clone(),
                queue: VecDeque::new(),
                next: 1,
            }))
        }),
        shared,
    )
}

pub(super) fn spec() -> EngineSpec {
    EngineSpec {
        name: "Stockfish de teste".to_owned(),
        path: PathBuf::from("/engines/stockfish-de-teste"),
        options: Vec::new(),
    }
}

// ---- Ambiente -------------------------------------------------------------------------------

pub(super) struct Env {
    pub(super) server: FakeServer,
    pub(super) service: NetService,
    pub(super) run: EngineOnline,
    pub(super) store: Store,
    pub(super) profile: ProfileId,
    pub(super) events: LiveFeed,
}

pub(super) fn env() -> Env {
    env_with(true, true)
}

fn env_with(authorized: bool, provider_on: bool) -> Env {
    let clock = Arc::new(FakeClock::new(T0));
    let server = FakeServer::with_clock(clock.clone());
    let config = NetConfig::in_memory()
        .clock(clock.clone())
        .jitter_seed(7)
        .vault(Arc::new(FakeVault))
        .route("lichess.org", &server.base_url());
    let net = Net::open(config).unwrap();
    net.set_enabled(ProviderId::Lichess, provider_on).unwrap();
    let service = NetService::from_net(net, clock, false);
    if authorized {
        service.set_authorized_connection(
            ProviderId::Lichess,
            ConnectionRole::Engine,
            Some(ENGINE_CONNECTION),
        );
    }
    let events = LiveFeed::new();
    server.route("/api/stream/event", FakeResponse::live(&events));
    let store = Store::open_in_memory().unwrap();
    let profile = store.default_profile().unwrap();
    Env {
        server,
        service,
        run: EngineOnline::default(),
        store,
        profile,
        events,
    }
}

pub(super) fn ok() -> FakeResponse {
    FakeResponse::json(r#"{"ok":true}"#)
}

impl Env {
    pub(super) fn frame(&mut self) {
        self.run.frame(Some(StoreAccess {
            store: &mut self.store,
            profile: self.profile,
        }));
    }

    pub(super) fn until(&mut self, mut cond: impl FnMut(&Self) -> bool) -> bool {
        let start = Instant::now();
        while start.elapsed() < WAIT {
            self.frame();
            if cond(self) {
                return true;
            }
            std::thread::sleep(Duration::from_millis(10));
        }
        false
    }

    pub(super) fn start(&mut self, settings: Settings, factory: EngineFactory) {
        self.run
            .start(&self.service, spec(), settings, factory)
            .unwrap();
        assert!(self.events.wait_connected(WAIT), "o fluxo de eventos abriu");
    }

    pub(super) fn posts_to(&self, path: &str) -> usize {
        self.server
            .requests()
            .iter()
            .filter(|r| r.method == "POST" && r.path == path)
            .count()
    }

    pub(super) fn posts(&self) -> usize {
        self.server
            .requests()
            .iter()
            .filter(|r| r.method == "POST")
            .count()
    }

    fn push_challenge(
        &self,
        id: &str,
        name: &str,
        speed: &str,
        variant: &str,
        title: Option<&str>,
    ) {
        self.events.push(
            &json!({ "type": "challenge", "challenge": {
                "id": id, "status": "created",
                "challenger": { "id": name.to_lowercase(), "name": name, "title": title, "rating": 1662 },
                "destUser": { "id": "motor", "name": "Motor", "title": "BOT" },
                "variant": { "key": variant, "name": variant },
                "rated": false, "speed": speed,
                "timeControl": { "type": "clock", "limit": 300, "increment": 2, "show": "5+2" },
                "color": "random",
            } })
            .to_string(),
        );
    }

    /// O Lichess abre a Partida `id`: a Engine joga de brancas contra `name`.
    pub(super) fn open_game(&self, id: &str, name: &str) -> LiveFeed {
        let feed = LiveFeed::new();
        self.server.route(
            &format!("/api/bot/game/stream/{id}"),
            FakeResponse::live(&feed),
        );
        self.events
            .push(&json!({ "type": "gameStart", "game": info(id, name) }).to_string());
        feed
    }
}

fn info(id: &str, opponent: &str) -> Value {
    json!({
        "fullId": format!("{id}91cB"), "gameId": id,
        "fen": "rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 0 1",
        "color": "white", "lastMove": "", "source": "friend",
        "status": { "id": 20, "name": "started" },
        "variant": { "key": "standard", "name": "Standard" },
        "speed": "blitz", "perf": "blitz", "rated": false, "hasMoved": false,
        "opponent": { "id": opponent.to_lowercase(), "username": opponent, "rating": 1662 },
        "isMyTurn": true, "secondsLeft": 300,
        "compat": { "bot": true, "board": false },
        "id": id,
    })
}

pub(super) fn state(moves: &str, status: &str) -> Value {
    json!({
        "type": "gameState", "moves": moves, "wtime": 300_000, "btime": 300_000,
        "winc": 2000, "binc": 2000, "status": status,
    })
}

pub(super) fn full(id: &str, opponent: &str) -> Value {
    json!({
        "type": "gameFull", "id": id,
        "variant": { "key": "standard", "name": "Standard", "short": "Std" },
        "speed": "blitz", "rated": false,
        "white": { "id": "motor", "name": "Motor", "title": "BOT", "rating": 2500 },
        "black": { "id": opponent.to_lowercase(), "name": opponent, "rating": 1662 },
        "initialFen": "startpos",
        "clock": { "initial": 300_000, "increment": 2_000 },
        "state": state("", "started"),
    })
}

pub(super) fn move_path(id: &str, uci: &str) -> String {
    format!("/api/bot/game/{id}/move/{uci}")
}

// ---- Nada inicia sozinho ----------------------------------------------------------------------

#[test]
fn nothing_is_requested_until_the_user_turns_it_on() {
    let mut env = env();
    for _ in 0..20 {
        env.frame();
    }
    assert_eq!(env.run.state(), RunState::Stopped);
    assert!(env.server.requests().is_empty(), "parada: nenhum pedido");

    let (factory, _) = scripted(&[]);
    env.start(Settings::default(), factory);
    assert_eq!(env.run.state(), RunState::Waiting);
    assert!(
        env.server
            .requests()
            .iter()
            .any(|r| r.path == "/api/stream/event")
    );
}

#[test]
fn it_needs_the_engine_connection_and_the_provider_before_any_request() {
    // Sem Conexão de Engine (a do jogador não vale).
    let mut env = env_with(false, true);
    env.service.set_authorized_connection(
        ProviderId::Lichess,
        ConnectionRole::Player,
        Some(ConnectionId(1)),
    );
    let (factory, _) = scripted(&[]);
    let result = env
        .run
        .start(&env.service, spec(), Settings::default(), factory);
    assert_eq!(result, Err(Unavailable::NoConnection));
    assert!(env.server.requests().is_empty());
    assert_eq!(env.run.state(), RunState::Stopped);

    // Lichess desligado: nada sai.
    let mut env = env_with(true, false);
    let (factory, _) = scripted(&[]);
    let result = env
        .run
        .start(&env.service, spec(), Settings::default(), factory);
    assert_eq!(result, Err(Unavailable::ProviderOff));
    env.frame();
    assert!(
        env.server.requests().is_empty(),
        "Provedor desligado: nada sai"
    );
}

// ---- Desafios e Partida -----------------------------------------------------------------------

#[test]
fn it_accepts_a_challenge_plays_validated_moves_and_stores_the_game() {
    let mut env = env();
    env.server.route("/api/challenge/chal0001/accept", ok());
    env.server.route(&move_path(GAME, "e2e4"), ok());
    let (factory, shared) = scripted(&["e2e4"]);
    env.start(Settings::default(), factory);

    env.push_challenge("chal0001", "Salma", "blitz", "standard", None);
    assert!(env.until(|e| e.posts_to("/api/challenge/chal0001/accept") == 1));
    let accept = env
        .server
        .requests()
        .into_iter()
        .find(|r| r.path == "/api/challenge/chal0001/accept")
        .unwrap();
    assert_eq!(
        accept.header("authorization"),
        Some("Bearer lip_TOKEN_DA_ENGINE_DE_TESTE_DO_APP"),
        "pela Conexão de Engine"
    );

    let feed = env.open_game(GAME, "Salma");
    assert!(env.until(|_| feed.wait_connected(Duration::from_millis(1))));
    feed.push(&full(GAME, "Salma").to_string());
    assert!(env.until(|e| e.run.state() == RunState::Playing));
    assert!(env.until(|e| e.posts_to(&move_path(GAME, "e2e4")) == 1));
    assert_eq!(shared.lock().unwrap().requests, 1);
    let rows = env.run.rows();
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0].opponent, "Salma");
    assert!(rows[0].white);

    // O Lichess confirma e a Partida termina: conta como jogada e sai da lista.
    feed.push(&state("e2e4", "started").to_string());
    feed.push(
        &json!({
            "type": "gameState", "moves": "e2e4 e7e5", "wtime": 300_000, "btime": 290_000,
            "winc": 2000, "binc": 2000, "status": "resign", "winner": "white",
        })
        .to_string(),
    );
    feed.close();
    assert!(env.until(|e| e.run.counts().2 == 1), "{:?}", env.run);
    assert!(env.run.rows().is_empty());

    // A Partida ficou no banco como Partida da Engine, nunca como Partida de pessoa.
    let games = env.store.list_games(&GameQuery::default()).unwrap();
    assert_eq!(games.len(), 1);
    let metadata = &games[0].metadata;
    assert_eq!(metadata["online"]["provider"], "lichess-bot");
    assert_eq!(metadata["online"]["remote_id"], GAME);
    assert!(
        env.service.fair_play().followed() == 0 && !env.service.fair_play().any(),
        "Engine declarada: a Trava de fair play nunca liga"
    );
}

#[test]
fn challenges_outside_the_users_limits_are_declined_with_the_reason() {
    let mut env = env();
    env.server.route("/api/challenge/varia001/decline", ok());
    env.server.route("/api/challenge/engine01/decline", ok());
    env.server.route("/api/challenge/bullet01/decline", ok());
    let (factory, _) = scripted(&[]);
    env.start(Settings::default(), factory);

    env.push_challenge("varia001", "Gambito_Verde", "blitz", "chess960", None);
    env.push_challenge("engine01", "OutroMotor", "blitz", "standard", Some("BOT"));
    env.push_challenge("bullet01", "Veloz", "bullet", "standard", None);
    assert!(env.until(|e| {
        e.posts_to("/api/challenge/varia001/decline") == 1
            && e.posts_to("/api/challenge/engine01/decline") == 1
            && e.posts_to("/api/challenge/bullet01/decline") == 1
    }));
    assert_eq!(env.run.counts(), (0, 3, 0));
    assert_eq!(env.posts_to("/api/challenge/varia001/accept"), 0);
    let reasons: Vec<DeclineReason> = env
        .run
        .activity()
        .filter_map(|a| match a {
            Activity::Declined { reason, .. } => Some(*reason),
            _ => None,
        })
        .collect();
    assert_eq!(
        reasons,
        [
            DeclineReason::Standard,
            DeclineReason::NoBot,
            DeclineReason::TimeControl
        ]
    );
    assert_eq!(env.run.state(), RunState::Waiting, "recusar não é jogar");
}

#[test]
fn the_games_limit_stops_accepting_and_the_user_chose_it() {
    let mut env = env();
    env.server.route("/api/challenge/chal0001/accept", ok());
    env.server.route("/api/challenge/chal0002/decline", ok());
    let (factory, _) = scripted(&[]);
    let settings = Settings {
        max_games: Some(1),
        ..Settings::default()
    };
    env.start(settings, factory);

    env.push_challenge("chal0001", "Salma", "blitz", "standard", None);
    assert!(env.until(|e| e.posts_to("/api/challenge/chal0001/accept") == 1));
    assert_eq!(env.run.stop_reason(), Some(&StopReason::LimitReached));
    assert_eq!(env.run.state(), RunState::Stopping);

    // Um segundo desafio, já no limite: recusado "mais tarde", nunca aceito.
    env.push_challenge("chal0002", "Veloz", "blitz", "standard", None);
    assert!(env.until(|e| e.posts_to("/api/challenge/chal0002/decline") == 1));
    assert_eq!(env.posts_to("/api/challenge/chal0002/accept"), 0);
}

#[test]
fn the_concurrent_limit_comes_from_the_policy() {
    let mut env = env();
    env.server.route("/api/challenge/chal0001/accept", ok());
    env.server.route("/api/challenge/chal0002/decline", ok());
    let (factory, _) = scripted(&[]);
    let settings = Settings {
        policy: ChallengePolicy {
            max_concurrent: 1,
            ..ChallengePolicy::default()
        },
        max_games: None,
        ..Settings::default()
    };
    env.start(settings, factory);
    env.push_challenge("chal0001", "Salma", "blitz", "standard", None);
    assert!(env.until(|e| e.posts_to("/api/challenge/chal0001/accept") == 1));
    env.push_challenge("chal0002", "Veloz", "rapid", "standard", None);
    assert!(env.until(|e| e.posts_to("/api/challenge/chal0002/decline") == 1));
    assert_eq!(env.posts_to("/api/challenge/chal0002/accept"), 0);
}

// ---- Parar ------------------------------------------------------------------------------------

#[test]
fn stop_with_nothing_running_closes_and_ignores_later_challenges() {
    let mut env = env();
    env.server.route("/api/challenge/chal0009/accept", ok());
    let (factory, _) = scripted(&[]);
    env.start(Settings::default(), factory);

    env.run.stop();
    env.frame();
    assert_eq!(env.run.state(), RunState::Stopped);
    // Um desafio que chega depois não é visto nem aceito.
    env.push_challenge("chal0009", "Atrasado", "blitz", "standard", None);
    for _ in 0..30 {
        env.frame();
        std::thread::sleep(Duration::from_millis(5));
    }
    assert_eq!(env.posts(), 0);
    assert_eq!(env.run.state(), RunState::Stopped);
}

#[test]
fn stop_lets_the_running_game_finish_and_declines_new_challenges() {
    let mut env = env();
    env.server.route("/api/challenge/En9ineQ1/accept", ok());
    env.server.route("/api/challenge/chal0002/decline", ok());
    env.server.route(&move_path(GAME, "e2e4"), ok());
    let (factory, _) = scripted(&["e2e4"]);
    env.start(
        Settings {
            max_games: None,
            ..Settings::default()
        },
        factory,
    );
    env.push_challenge("En9ineQ1", "Salma", "blitz", "standard", None);
    assert!(env.until(|e| e.posts_to("/api/challenge/En9ineQ1/accept") == 1));
    let feed = env.open_game(GAME, "Salma");
    assert!(env.until(|_| feed.wait_connected(Duration::from_millis(1))));
    feed.push(&full(GAME, "Salma").to_string());
    assert!(env.until(|e| e.run.state() == RunState::Playing));

    env.run.stop();
    assert_eq!(env.run.state(), RunState::Stopping);
    assert_eq!(env.run.stop_reason(), Some(&StopReason::User));
    env.push_challenge("chal0002", "Veloz", "blitz", "standard", None);
    assert!(env.until(|e| e.posts_to("/api/challenge/chal0002/decline") == 1));
    assert_eq!(env.posts_to("/api/challenge/chal0002/accept"), 0);
    assert_eq!(env.run.rows().len(), 1, "a Partida em andamento segue");

    // Ela termina e só então a Engine fecha.
    feed.push(
        &json!({
            "type": "gameState", "moves": "e2e4 e7e5", "wtime": 1, "btime": 1,
            "winc": 0, "binc": 0, "status": "resign", "winner": "black",
        })
        .to_string(),
    );
    feed.close();
    assert!(
        env.until(|e| e.run.state() == RunState::Stopped),
        "{:?}",
        env.run
    );
}

#[test]
fn stop_now_resigns_the_running_games_once() {
    let mut env = env();
    env.server.route("/api/challenge/chal0001/accept", ok());
    env.server
        .route(&format!("/api/bot/game/{GAME}/resign"), ok());
    let (factory, _) = scripted(&[]);
    env.start(Settings::default(), factory);
    env.push_challenge("chal0001", "Salma", "blitz", "standard", None);
    assert!(env.until(|e| e.posts_to("/api/challenge/chal0001/accept") == 1));
    let feed = env.open_game(GAME, "Salma");
    assert!(env.until(|_| feed.wait_connected(Duration::from_millis(1))));
    // A vez é do adversário (a Engine é de pretas): nada para a Engine jogar.
    let black = full(GAME, "Salma")
        .to_string()
        .replace("\"motor\"", "\"salma\"");
    feed.push(&black);
    assert!(env.until(|e| e.run.state() == RunState::Playing));

    env.run.stop_now();
    assert_eq!(env.run.stop_reason(), Some(&StopReason::UserNow));
    assert!(env.until(|e| e.posts_to(&format!("/api/bot/game/{GAME}/resign")) == 1));
    env.frame();
    env.frame();
    assert_eq!(
        env.posts_to(&format!("/api/bot/game/{GAME}/resign")),
        1,
        "um abandono só"
    );
}

// ---- A Engine ---------------------------------------------------------------------------------

#[test]
fn an_illegal_engine_move_is_refused_by_the_rules_core_and_never_sent() {
    let mut env = env();
    env.server.route("/api/challenge/chal0001/accept", ok());
    env.server.route(&move_path(GAME, "e2e4"), ok());
    let (factory, _) = scripted(&["e2e5", "e2e4"]);
    env.start(Settings::default(), factory);
    env.push_challenge("chal0001", "Salma", "blitz", "standard", None);
    assert!(env.until(|e| e.posts_to("/api/challenge/chal0001/accept") == 1));
    let feed = env.open_game(GAME, "Salma");
    assert!(env.until(|_| feed.wait_connected(Duration::from_millis(1))));
    feed.push(&full(GAME, "Salma").to_string());

    assert!(env.until(|e| e.posts_to(&move_path(GAME, "e2e4")) == 1));
    assert_eq!(
        env.posts_to(&move_path(GAME, "e2e5")),
        0,
        "ilegal: nada saiu"
    );
    assert!(
        env.run
            .activity()
            .any(|a| matches!(a, Activity::EngineMoveRejected))
    );
}

#[test]
fn an_engine_that_does_not_start_resigns_instead_of_letting_the_clock_run() {
    let mut env = env();
    env.server.route("/api/challenge/chal0001/accept", ok());
    env.server
        .route(&format!("/api/bot/game/{GAME}/resign"), ok());
    let factory: EngineFactory = Box::new(|_| Err("O executável não iniciou.".to_owned()));
    env.start(Settings::default(), factory);
    env.push_challenge("chal0001", "Salma", "blitz", "standard", None);
    assert!(env.until(|e| e.posts_to("/api/challenge/chal0001/accept") == 1));
    let feed = env.open_game(GAME, "Salma");
    assert!(env.until(|_| feed.wait_connected(Duration::from_millis(1))));
    feed.push(&full(GAME, "Salma").to_string());

    assert!(env.until(|e| e.posts_to(&format!("/api/bot/game/{GAME}/resign")) == 1));
    assert!(
        env.run
            .activity()
            .any(|a| matches!(a, Activity::EngineStartFailed { .. }))
    );
}

/// A Engine que a Trava de fair play recusou: devolve o motivo da Trava como falha.
struct Locked {
    queue: VecDeque<ControllerEvent>,
}

impl MoveEngine for Locked {
    fn search(&mut self, _request: SearchRequest) -> u64 {
        self.queue.push_back(ControllerEvent::Failed {
            id: Some(1),
            failure: chess_uci::Failure::new(
                chess_uci::FailureKind::Spawn,
                crate::fair_play::FairPlayLocked {
                    computation: crate::fair_play::Computation::OnlineEngine,
                    scope: crate::fair_play::LockScope::Everything,
                }
                .to_string(),
            ),
        });
        1
    }

    fn cancel(&mut self) {}

    fn poll(&mut self) -> Option<ControllerEvent> {
        self.queue.pop_front()
    }
}

#[test]
fn the_fair_play_lock_stopping_a_game_is_shown_as_the_lock_and_not_as_an_engine_failure() {
    let mut env = env();
    env.server.route("/api/challenge/chal0001/accept", ok());
    env.server
        .route(&format!("/api/bot/game/{GAME}/resign"), ok());
    let factory: EngineFactory = Box::new(|_| {
        Ok(Box::new(Locked {
            queue: VecDeque::new(),
        }))
    });
    env.start(Settings::default(), factory);
    env.push_challenge("chal0001", "Salma", "blitz", "standard", None);
    assert!(env.until(|e| e.posts_to("/api/challenge/chal0001/accept") == 1));
    let feed = env.open_game(GAME, "Salma");
    assert!(env.until(|_| feed.wait_connected(Duration::from_millis(1))));
    feed.push(&full(GAME, "Salma").to_string());

    assert!(env.until(|e| {
        e.run
            .activity()
            .any(|a| matches!(a, Activity::FairPlayStopped { .. }))
    }));
    assert!(
        !env.run
            .activity()
            .any(|a| matches!(a, Activity::EngineFailed { .. })),
        "a Trava não é falha da Engine"
    );
    let i18n = crate::i18n::I18n::pt_br();
    let text = i18n.t_with(
        "engine-online-act-fairplay-stopped",
        &[("why", &i18n.t("fairplay-locked-everything"))],
    );
    assert!(text.contains("Trava de fair play"), "{text}");
    assert!(!text.contains("falhou"), "{text}");
}

#[test]
fn a_provider_turned_off_while_running_sends_nothing_more() {
    let mut env = env();
    env.server.route("/api/challenge/chal0001/accept", ok());
    let (factory, _) = scripted(&[]);
    env.start(Settings::default(), factory);
    env.service.set_enabled(ProviderId::Lichess, false).unwrap();

    env.push_challenge("chal0001", "Salma", "blitz", "standard", None);
    assert!(env.until(|e| {
        e.run
            .activity()
            .any(|a| matches!(a, Activity::AcceptFailed { .. }))
    }));
    assert_eq!(env.posts_to("/api/challenge/chal0001/accept"), 0);
    assert_eq!(env.run.counts().0, 0, "o aceite que não saiu não conta");
}

// ---- Conversão em BOT -------------------------------------------------------------------------

#[test]
fn converting_to_bot_needs_the_confirmation_and_is_sent_once() {
    let mut env = env();
    env.server.route("/api/bot/account/upgrade", ok());
    for _ in 0..5 {
        env.frame();
    }
    assert_eq!(env.posts_to("/api/bot/account/upgrade"), 0, "nunca sozinha");

    env.run
        .upgrade_to_bot(&env.service, IrreversibleConfirmation::user_confirmed())
        .unwrap();
    // Um segundo clique durante o envio não repete o pedido.
    env.run
        .upgrade_to_bot(&env.service, IrreversibleConfirmation::user_confirmed())
        .unwrap();
    assert!(env.until(|e| matches!(e.run.conversion(), Some(Conversion::Done))));
    assert_eq!(env.posts_to("/api/bot/account/upgrade"), 1);
    assert_eq!(
        ConversionView::of(env.run.conversion()),
        Some(ConversionView::Done)
    );
}

#[test]
fn converting_without_the_engine_connection_asks_nothing() {
    let mut env = env_with(false, true);
    let result = env
        .run
        .upgrade_to_bot(&env.service, IrreversibleConfirmation::user_confirmed());
    assert_eq!(result, Err(Unavailable::NoConnection));
    assert!(env.server.requests().is_empty());
}

// ---- A tela -----------------------------------------------------------------------------------

#[test]
fn the_view_model_and_intents_follow_the_service() {
    let mut env = env();
    let engines = vec![spec()];
    let model = ViewModel::of(&env.run, &env.service, engines.clone());
    assert!(model.connected && model.provider_on);
    assert_eq!(model.state, RunState::Stopped);

    // Ligar sem Engine escolhida não faz nada.
    let mut ui = EngineOnlineUi::default();
    apply_intent(
        Intent::Start,
        &mut ui,
        &mut env.run,
        &env.service,
        &engines,
        &mut || {},
    );
    assert_eq!(env.run.state(), RunState::Stopped);
    assert!(env.server.requests().is_empty());

    // Com a Engine escolhida, liga; "Parar" fecha.
    ui.form = Form {
        engine: Some(spec().path),
        ..Form::default()
    };
    apply_intent(
        Intent::Start,
        &mut ui,
        &mut env.run,
        &env.service,
        &engines,
        &mut || {},
    );
    assert_eq!(env.run.state(), RunState::Waiting);
    let model = ViewModel::of(&env.run, &env.service, engines.clone());
    assert_eq!(model.running_engine.as_deref(), Some("Stockfish de teste"));
    apply_intent(
        Intent::Stop,
        &mut ui,
        &mut env.run,
        &env.service,
        &engines,
        &mut || {},
    );
    env.frame();
    assert_eq!(env.run.state(), RunState::Stopped);

    // Sem Conexão de Engine, a tela manda para a Central.
    let mut opened = false;
    apply_intent(
        Intent::OpenCenter,
        &mut ui,
        &mut env.run,
        &env.service,
        &engines,
        &mut || opened = true,
    );
    assert!(opened && !ui.open);
}

#[test]
fn every_engine_online_text_exists_in_pt_br_and_uses_the_glossary_term() {
    let i18n = crate::i18n::I18n::pt_br();
    let all = std::fs::read_to_string(
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("assets/i18n/pt-BR/engineonline.ftl"),
    )
    .unwrap();
    for line in all.lines().filter(|l| l.contains(" = ")) {
        let key = line.split(" = ").next().unwrap().trim();
        assert_ne!(i18n.t(key), key, "{key}");
    }
    let lower = all.to_lowercase();
    // Jargão fora do texto de usuário (as chaves podem citar o ritmo): "FEN", "Threads", "Hash" e
    // "Bullet" (o app diz "Relâmpago").
    for line in all.lines().filter(|l| l.contains(" = ")) {
        let value = line.split_once(" = ").unwrap().1.to_lowercase();
        for jargon in ["fen", "threads", "hash", "bullet"] {
            assert!(
                !value
                    .split(|c: char| !c.is_alphanumeric())
                    .any(|w| w == jargon),
                "{jargon} em: {line}"
            );
        }
    }
    // O termo do GLOSSARY é "Engine online", nunca "bot" solto (só a sigla da conta BOT).
    for banned in ["bot online", "robô", "http", "json", "endpoint", "token"] {
        assert!(!lower.contains(banned), "{banned}");
    }
}

// ---- Fechar o app, força e retomada (#271) --------------------------------------------------------

/// Uma Partida em andamento em que a vez é do adversário (a Engine é de pretas).
pub(super) fn playing_env() -> (Env, LiveFeed) {
    let mut env = env();
    let (factory, _) = scripted(&[]);
    env.start(Settings::default(), factory);
    let feed = env.open_game(GAME, "Salma");
    assert!(env.until(|_| feed.wait_connected(Duration::from_millis(1))));
    feed.push(
        &full(GAME, "Salma")
            .to_string()
            .replace("\"motor\"", "\"salma\""),
    );
    assert!(env.until(|e| e.run.state() == RunState::Playing));
    (env, feed)
}

#[test]
fn closing_the_app_asks_only_while_a_game_is_in_progress() {
    use super::exit::{CloseDecision, ExitChoice, ExitPrompt, apply_choice, decide};
    let idle = EngineOnline::default();
    assert_eq!(decide(&idle), CloseDecision::Close, "parada: fecha direto");

    let (mut env, _feed) = playing_env();
    assert_eq!(decide(&env.run), CloseDecision::Ask { games: 1 });

    let mut ui = EngineOnlineUi::default();
    let mut prompt = ExitPrompt { open: true };
    // Cancelar: nada muda e o app segue.
    assert!(!apply_choice(
        ExitChoice::Cancel,
        &mut prompt,
        &mut ui,
        &mut env.run
    ));
    assert!(!prompt.open && env.run.state() == RunState::Playing);
    // Parar e esperar: não aceita mais desafios, o app segue aberto e a tela mostra o estado.
    prompt.open = true;
    assert!(!apply_choice(
        ExitChoice::StopAndWait,
        &mut prompt,
        &mut ui,
        &mut env.run
    ));
    assert_eq!(env.run.state(), RunState::Stopping);
    assert_eq!(env.run.stop_reason(), Some(&StopReason::User));
    assert!(ui.open, "a tela da Engine online abre");
    assert_eq!(env.posts(), 0, "nada foi enviado ao Lichess");
    // Sair mesmo assim: o app fecha.
    prompt.open = true;
    assert!(apply_choice(
        ExitChoice::LeaveNow,
        &mut prompt,
        &mut ui,
        &mut env.run
    ));
}

#[test]
fn a_finished_game_no_longer_blocks_closing() {
    use super::exit::{CloseDecision, decide};
    let (mut env, feed) = playing_env();
    feed.push(&state("e2e4", "mate").to_string());
    feed.close();
    assert!(env.until(|e| e.run.rows().is_empty()));
    assert_eq!(decide(&env.run), CloseDecision::Close);
}

#[test]
fn the_strength_is_the_same_elo_the_local_opponent_uses() {
    use super::service::strength_of;
    use chess_uci::Strength;
    assert_eq!(strength_of(None), Strength::Full);
    assert_eq!(strength_of(Some(1500)), Strength::Elo(1500));
    // Fora da faixa da Engine adversária local, ajusta para ela.
    assert_eq!(
        strength_of(Some(100)),
        Strength::Elo(crate::engine::MIN_ELO)
    );
    assert_eq!(
        strength_of(Some(9000)),
        Strength::Elo(crate::engine::MAX_ELO)
    );
    assert_eq!(Settings::default().elo, None, "força total por padrão");
}

#[test]
fn a_game_in_the_database_is_resumed_in_the_same_log_when_the_engine_is_turned_on_again() {
    let (mut first, _feed) = playing_env();
    assert!(first.until(|e| e.store.list_games(&GameQuery::default()).unwrap().len() == 1));
    assert_eq!(
        first.store.list_games(&GameQuery::default()).unwrap().len(),
        1
    );

    // O app fecha e abre de novo: outro serviço, o mesmo banco, o Lichess reenvia o gameStart.
    let mut second = env();
    second.store = first.store;
    second.profile = first.profile;
    let (factory, _) = scripted(&[]);
    second.start(Settings::default(), factory);
    let feed = second.open_game(GAME, "Salma");
    assert!(second.until(|_| feed.wait_connected(Duration::from_millis(1))));
    feed.push(
        &full(GAME, "Salma")
            .to_string()
            .replace("\"motor\"", "\"salma\""),
    );
    assert!(second.until(|e| e.run.state() == RunState::Playing));
    assert!(
        second
            .run
            .activity()
            .any(|a| matches!(a, Activity::GameResumed { .. })),
        "{:?}",
        second.run
    );
    second.frame();
    assert_eq!(
        second
            .store
            .list_games(&GameQuery::default())
            .unwrap()
            .len(),
        1,
        "nunca uma segunda linha na Biblioteca"
    );
}
