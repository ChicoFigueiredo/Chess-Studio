#![allow(clippy::unwrap_used)]
//! Engine online (#234, CX-13): uma Engine local joga pela Bot API do Lichess com uma Conexão
//! de Engine. Servidor e relógio falsos (CX-17); respostas **sintéticas** no formato da
//! documentação oficial (lichess-org/api: `GameFullEvent`, `GameStateEvent`, `ChallengeJson`,
//! `/api/bot/...`). A Engine é uma dupla roteirizada (`MoveEngine`); a partida de ponta a
//! ponta com a Engine UCI falsa do `chess-uci` está em `chess-uci/tests/online_engine.rs`.

mod common;

use chess_core::Move;
use chess_game::{EndReason, Game, GameResult, PolicyKind};
use chess_net::testing::{FakeResponse, LiveFeed};
use chess_providers::{
    Backoff, BoardError, Challenge, ChallengeDecision, ChallengePolicy, ChallengeSpeed,
    ConnectionEvent, ConnectionRole, DeclineReason, DrawResponse, EnginePolicy, GameInfo,
    IrreversibleConfirmation, LichessBoard, LichessBot, MoveEngine, OnlineGameSession,
    OpponentPolicy, RatedPolicy, SessionNotice,
};
use chess_uci::{ControllerEvent, SearchRequest};
use common::*;
use serde_json::{Value, json};
use std::collections::VecDeque;
use std::sync::{Arc, Mutex};
use std::time::Duration;

const WAIT: Duration = Duration::from_secs(10);
const GAME: &str = "En9ineQ1";

// ---- A Engine roteirizada -------------------------------------------------------------------

#[derive(Default)]
struct Shared {
    script: VecDeque<String>,
    requests: Vec<SearchRequest>,
    cancelled: usize,
}

/// Responde a cada busca com o próximo Lance do roteiro (texto UCI cru, legal ou não).
struct Scripted {
    shared: Arc<Mutex<Shared>>,
    queue: VecDeque<ControllerEvent>,
    next: u64,
}

impl Scripted {
    fn boxed(script: &[&str]) -> (Box<dyn MoveEngine>, Arc<Mutex<Shared>>) {
        let shared = Arc::new(Mutex::new(Shared {
            script: script.iter().map(|s| (*s).to_owned()).collect(),
            ..Shared::default()
        }));
        (
            Box::new(Self {
                shared: shared.clone(),
                queue: VecDeque::new(),
                next: 1,
            }),
            shared,
        )
    }
}

impl MoveEngine for Scripted {
    fn search(&mut self, request: SearchRequest) -> u64 {
        let id = self.next;
        self.next += 1;
        let mut shared = self.shared.lock().unwrap();
        shared.requests.push(request);
        if let Some(uci) = shared.script.pop_front() {
            self.queue.push_back(ControllerEvent::Move { id, uci });
        }
        id
    }

    fn cancel(&mut self) {
        self.shared.lock().unwrap().cancelled += 1;
        self.queue.clear();
    }

    fn poll(&mut self) -> Option<ControllerEvent> {
        self.queue.pop_front()
    }
}

// ---- Servidor falso -------------------------------------------------------------------------

fn env_enabled() -> Env {
    let env = common::env_enabled();
    store_stage5_tokens(&env);
    env
}

fn bot(env: &Env) -> LichessBot {
    LichessBot::new(
        env.net.clone(),
        env.clock.clone(),
        LICHESS_ENGINE_CONNECTION,
        ConnectionRole::Engine,
    )
    .unwrap()
    .backoff(Backoff {
        base: Duration::from_secs(2),
        max: Duration::from_secs(16),
    })
    .jitter_seed(9)
}

fn stream_path() -> String {
    format!("/api/bot/game/stream/{GAME}")
}

fn move_path(uci: &str) -> String {
    format!("/api/bot/game/{GAME}/move/{uci}")
}

fn ok() -> FakeResponse {
    FakeResponse::json(r#"{"ok":true}"#)
}

fn state(moves: &str, wtime: u64, btime: u64, status: &str) -> Value {
    json!({
        "type": "gameState", "moves": moves, "wtime": wtime, "btime": btime,
        "winc": 2000, "binc": 2000, "status": status,
    })
}

fn with(mut value: Value, key: &str, extra: Value) -> Value {
    value[key] = extra;
    value
}

/// `gameFull` sintético: a Engine (conta BOT) joga de brancas contra Salma.
fn full(state: Value) -> Value {
    json!({
        "type": "gameFull", "id": GAME,
        "variant": { "key": "standard", "name": "Standard", "short": "Std" },
        "speed": "blitz", "rated": false,
        "white": { "id": "motor", "name": "Motor", "title": "BOT", "rating": 2500 },
        "black": { "id": "salma", "name": "Salma", "rating": 1662 },
        "initialFen": "startpos",
        "clock": { "initial": 300_000, "increment": 2_000 },
        "state": state,
    })
}

fn info_json(color: &str) -> Value {
    json!({
        "fullId": format!("{GAME}91cB"), "gameId": GAME,
        "fen": "rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 0 1",
        "color": color, "lastMove": "", "source": "friend",
        "status": { "id": 20, "name": "started" },
        "variant": { "key": "standard", "name": "Standard" },
        "speed": "blitz", "perf": "blitz", "rated": false, "hasMoved": false,
        "opponent": { "id": "salma", "username": "Salma", "rating": 1662 },
        "isMyTurn": color == "white", "secondsLeft": 300,
        "compat": { "bot": true, "board": false },
        "id": GAME,
    })
}

fn game_info(color: &str) -> GameInfo {
    serde_json::from_value(info_json(color)).unwrap()
}

fn mv(uci: &str) -> Move {
    uci.parse().unwrap()
}

struct Run {
    session: OnlineGameSession,
    events: Vec<chess_game::Event>,
    notices: Vec<SessionNotice>,
}

impl Run {
    fn new(session: OnlineGameSession) -> Self {
        Self {
            session,
            events: Vec::new(),
            notices: Vec::new(),
        }
    }

    fn pump(&mut self) {
        let polled = self.session.poll();
        self.events.extend(polled.events);
        self.notices.extend(polled.notices);
    }

    fn until(&mut self, mut cond: impl FnMut(&Self) -> bool) -> bool {
        eventually(WAIT, || {
            self.pump();
            cond(self)
        })
    }

    fn game(&self) -> &Game {
        self.session.game().expect("Partida criada")
    }

    fn plies(&self) -> usize {
        self.session
            .game()
            .map_or(0, |g| g.snapshot().history.len())
    }
}

fn posts_to(env: &Env, path: &str) -> usize {
    env.server
        .requests()
        .iter()
        .filter(|r| r.method == "POST" && r.path == path)
        .count()
}

fn started(
    env: &Env,
    script: &[&str],
    policy: EnginePolicy,
) -> (Run, LiveFeed, Arc<Mutex<Shared>>) {
    let feed = LiveFeed::new();
    env.server.route(&stream_path(), FakeResponse::live(&feed));
    let (engine, shared) = Scripted::boxed(script);
    let mut run = Run::new(bot(env).open_game(&game_info("white"), engine, policy));
    assert!(feed.wait_connected(WAIT));
    feed.push(&full(state("", 300_000, 300_000, "started")).to_string());
    assert!(run.until(|r| r.session.game().is_some()));
    (run, feed, shared)
}

// ---- Partida completa -----------------------------------------------------------------------

#[test]
fn full_game_of_the_online_engine_is_driven_validated_and_sent_once_per_move() {
    let env = env_enabled();
    for path in ["e2e4", "f1c4", "d1h5", "h5f7"] {
        env.server.route(&move_path(path), ok());
    }
    env.server.route("/api/challenge/chal0001/accept", ok());
    let events = LiveFeed::new();
    env.server
        .route("/api/stream/event", FakeResponse::live(&events));

    // O desafio chega pelo fluxo de eventos da Conexão de Engine e a política o aceita.
    let bot = bot(&env);
    let mut connection = bot.events();
    assert!(events.wait_connected(WAIT));
    events.push(
        &json!({ "type": "challenge", "challenge": {
            "id": "chal0001", "status": "created",
            "challenger": { "id": "salma", "name": "Salma", "rating": 1662 },
            "destUser": { "id": "motor", "name": "Motor", "title": "BOT" },
            "variant": { "key": "standard", "name": "Standard" },
            "rated": false, "speed": "blitz",
            "timeControl": { "type": "clock", "limit": 300, "increment": 2, "show": "5+2" },
            "color": "random",
        } })
        .to_string(),
    );
    let mut challenge = None;
    assert!(eventually(WAIT, || {
        for event in connection.poll() {
            if let ConnectionEvent::Challenge(c) = event {
                challenge = Some(c);
            }
        }
        challenge.is_some()
    }));
    let answer = bot.answer_challenge(&challenge.unwrap(), 0);
    assert_eq!(answer.decision, ChallengeDecision::Accept);
    assert!(answer.request.wait().is_ok());
    let accept = env
        .server
        .requests()
        .into_iter()
        .find(|r| r.path == "/api/challenge/chal0001/accept")
        .unwrap();
    assert_eq!(
        accept.header("authorization"),
        Some(format!("Bearer {LICHESS_ENGINE_TOKEN}").as_str()),
        "pela Conexão de Engine"
    );

    // A Partida começa: a Engine joga de brancas.
    let (mut run, feed, shared) = started(
        &env,
        &["e2e4", "f1c4", "d1h5", "h5f7"],
        EnginePolicy::default(),
    );
    assert!(run.session.is_engine_game());
    assert_eq!(run.game().setup().policy(), PolicyKind::Online);
    let online = run.game().setup().online_game().unwrap().clone();
    assert_eq!(online.provider, "lichess-bot", "nunca é Partida de pessoa");
    assert!(online.is_engine_game());
    assert!(!run.game().online_human_in_progress());
    assert!(
        !env.net.fair_play_guard().is_engaged(),
        "Engine declarada: sem trava"
    );

    // Lances da Engine e respostas do adversário, com o relógio do Provedor.
    let script = [
        ("e2e4", "e2e4 e7e5", 300_000, 299_000),
        ("f1c4", "e2e4 e7e5 f1c4 b8c6", 297_000, 296_500),
        ("d1h5", "e2e4 e7e5 f1c4 b8c6 d1h5 g8f6", 290_000, 280_000),
    ];
    let mut so_far = String::new();
    for (mine, after_reply, wtime, btime) in script {
        assert!(
            eventually(WAIT, || {
                run.pump();
                posts_to(&env, &move_path(mine)) == 1
            }),
            "a Engine enviou {mine}"
        );
        assert!(run.game().sent_move().is_some(), "intenção enviada");
        let plies = run.plies();
        so_far = if so_far.is_empty() {
            mine.to_owned()
        } else {
            format!("{so_far} {mine}")
        };
        feed.push(&state(&so_far, wtime, btime, "started").to_string());
        assert!(run.until(|r| r.plies() == plies + 1));
        assert!(run.game().sent_move().is_none(), "confirmado pelo Provedor");
        so_far = after_reply.to_owned();
        feed.push(&state(&so_far, wtime, btime, "started").to_string());
        assert!(run.until(|r| r.plies() == plies + 2));
    }
    // O mate.
    assert!(eventually(WAIT, || {
        run.pump();
        posts_to(&env, &move_path("h5f7")) == 1
    }));
    let mate = with(
        state(&format!("{so_far} h5f7"), 289_000, 280_000, "mate"),
        "winner",
        json!("white"),
    );
    feed.push(&mate.to_string());
    assert!(run.until(|r| r.game().snapshot().is_over()));
    let outcome = run.game().snapshot().outcome.unwrap();
    assert_eq!(outcome.result, GameResult::WhiteWins);
    assert_eq!(outcome.reason, EndReason::Checkmate);

    // Cada Lance saiu uma vez, nenhum pela Board API, e a trava nunca ligou.
    for uci in ["e2e4", "f1c4", "d1h5", "h5f7"] {
        assert_eq!(posts_to(&env, &move_path(uci)), 1, "{uci}");
    }
    assert!(
        env.server
            .requests()
            .iter()
            .all(|r| !r.path.contains("/api/board/")),
        "a Engine nunca usa a Board API"
    );
    assert!(!env.net.fair_play_guard().is_engaged());
    assert!(!run.notices.iter().any(|n| matches!(
        n,
        SessionNotice::EngineMoveRejected { .. } | SessionNotice::EngineFailed { .. }
    )));

    // O pedido de busca levou a Posição desde o início e o relógio do Provedor.
    let requests = shared.lock().unwrap().requests.clone();
    assert_eq!(requests.len(), 4);
    assert!(requests[0].position.moves.is_empty());
    assert_eq!(requests[1].position.moves.len(), 2);
    let clock = requests[2].go.clock.expect("relógio do Provedor");
    assert_eq!(clock.wtime, Duration::from_millis(297_000));
    assert_eq!(clock.btime, Duration::from_millis(296_500));
    assert_eq!(clock.winc, Duration::from_millis(2_000));
}

// ---- Lance ilegal da Engine -----------------------------------------------------------------

#[test]
fn illegal_engine_move_is_refused_by_the_rules_core_and_nothing_is_sent() {
    let env = env_enabled();
    let policy = EnginePolicy {
        resign_if_engine_fails: false,
        ..EnginePolicy::default()
    };
    let (mut run, _feed, shared) = started(&env, &["e2e5", "a1a1"], policy);
    assert!(run.until(|r| {
        r.notices
            .iter()
            .any(|n| matches!(n, SessionNotice::EngineFailed { .. }))
    }));
    let rejected: Vec<&str> = run
        .notices
        .iter()
        .filter_map(|n| match n {
            SessionNotice::EngineMoveRejected { uci } => Some(uci.as_str()),
            _ => None,
        })
        .collect();
    assert_eq!(rejected, ["e2e5", "a1a1"]);
    assert!(run.game().sent_move().is_none());
    assert_eq!(run.plies(), 0);
    assert_eq!(shared.lock().unwrap().requests.len(), 2);
    assert!(run.session.engine_failed());
    // Nenhuma requisição de Lance nem de abandono saiu.
    assert!(
        env.server.requests().iter().all(|r| r.method != "POST"),
        "{:?}",
        env.server.requests()
    );
}

#[test]
fn engine_failure_resigns_by_default_instead_of_letting_the_clock_run() {
    let env = env_enabled();
    env.server
        .route(&format!("/api/bot/game/{GAME}/resign"), ok());
    let (mut run, _feed, _shared) = started(&env, &["a1a1", "xyz"], EnginePolicy::default());
    assert!(eventually(WAIT, || {
        run.pump();
        posts_to(&env, &format!("/api/bot/game/{GAME}/resign")) == 1
    }));
    assert!(
        run.notices
            .iter()
            .any(|n| matches!(n, SessionNotice::EngineFailed { .. }))
    );
    assert_eq!(posts_to(&env, &move_path("a1a1")), 0);
}

#[test]
fn one_illegal_move_then_a_legal_one_keeps_the_game_going() {
    let env = env_enabled();
    env.server.route(&move_path("e2e4"), ok());
    let (mut run, _feed, _shared) = started(&env, &["e2e5", "e2e4"], EnginePolicy::default());
    assert!(eventually(WAIT, || {
        run.pump();
        posts_to(&env, &move_path("e2e4")) == 1
    }));
    assert_eq!(posts_to(&env, &move_path("e2e5")), 0, "o ilegal nunca saiu");
    assert!(
        run.notices
            .iter()
            .any(|n| matches!(n, SessionNotice::EngineMoveRejected { uci } if uci == "e2e5"))
    );
    assert!(!run.session.engine_failed());
}

#[test]
fn a_person_cannot_play_through_an_engine_session() {
    let env = env_enabled();
    let (mut run, _feed, _shared) = started(&env, &[], EnginePolicy::default());
    assert!(matches!(
        run.session.play(mv("e2e4")),
        Err(BoardError::NotAllowed(_))
    ));
    assert!(run.session.claim_victory().is_err());
}

// ---- Envio único e reconciliação ------------------------------------------------------------

#[test]
fn a_dropped_send_is_never_repeated_blindly_and_the_provider_state_decides() {
    for applied_by_server in [true, false] {
        let env = env_enabled();
        env.server
            .route(&move_path("e2e4"), FakeResponse::drop_connection());
        let first = LiveFeed::new();
        let second = LiveFeed::new();
        env.server.route_seq(
            &stream_path(),
            vec![FakeResponse::live(&first), FakeResponse::live(&second)],
        );
        let (engine, _shared) = Scripted::boxed(&["e2e4", "e2e4"]);
        let mut run =
            Run::new(bot(&env).open_game(&game_info("white"), engine, EnginePolicy::default()));
        assert!(first.wait_connected(WAIT));
        first.push(&full(state("", 300_000, 300_000, "started")).to_string());
        assert!(run.until(|r| r.session.game().is_some()));

        // O POST caiu sem resposta: a sessão reabre o fluxo em vez de reenviar.
        assert!(run.until(|_| second.is_connected()), "reabre o fluxo");
        assert_eq!(posts_to(&env, &move_path("e2e4")), 1);
        let moves = if applied_by_server { "e2e4" } else { "" };
        second.push(&full(state(moves, 300_000, 300_000, "started")).to_string());
        if applied_by_server {
            assert!(run.until(|r| r.plies() == 1));
            for _ in 0..20 {
                run.pump();
            }
            assert_eq!(posts_to(&env, &move_path("e2e4")), 1, "o servidor já tinha");
        } else {
            // Não entrou: a intenção foi descartada e a Engine decide de novo (novo Lance,
            // pelo estado do Provedor; nunca o mesmo POST reenviado).
            assert!(run.until(|r| {
                r.notices
                    .iter()
                    .any(|n| matches!(n, SessionNotice::MoveLost { .. }))
            }));
        }
    }
}

// ---- Empate e abandono ----------------------------------------------------------------------

#[test]
fn draw_offers_follow_the_simple_policy() {
    for (response, path) in [
        (DrawResponse::Decline, "draw/no"),
        (DrawResponse::Accept, "draw/yes"),
    ] {
        let env = env_enabled();
        env.server
            .route(&format!("/api/bot/game/{GAME}/{path}"), ok());
        let policy = EnginePolicy {
            draw: response,
            ..EnginePolicy::default()
        };
        let (mut run, feed, _shared) = started(&env, &[], policy);
        // Lance dela (brancas) já está no estado; as pretas propõem empate.
        let offer = with(
            state("e2e4 e7e5", 290_000, 280_000, "started"),
            "bdraw",
            json!(true),
        );
        feed.push(&offer.to_string());
        assert!(eventually(WAIT, || {
            run.pump();
            posts_to(&env, &format!("/api/bot/game/{GAME}/{path}")) == 1
        }));
        // A mesma proposta não é respondida duas vezes.
        feed.push(&offer.to_string());
        for _ in 0..20 {
            run.pump();
        }
        assert_eq!(posts_to(&env, &format!("/api/bot/game/{GAME}/{path}")), 1);
    }
}

// ---- Conexões separadas ---------------------------------------------------------------------

#[test]
fn player_connection_does_not_play_as_engine_and_engine_connection_does_not_play_as_person() {
    let env = env_enabled();
    let as_engine = LichessBot::new(
        env.net.clone(),
        env.clock.clone(),
        EXPLORER_CONNECTION,
        ConnectionRole::Player,
    );
    assert!(matches!(as_engine, Err(BoardError::PlayerConnection)));
    assert!(
        BoardError::PlayerConnection
            .to_string()
            .contains("Conexão do jogador")
    );
    let as_person = LichessBoard::new(
        env.net.clone(),
        env.clock.clone(),
        LICHESS_ENGINE_CONNECTION,
        ConnectionRole::Engine,
    );
    assert!(matches!(as_person, Err(BoardError::EngineConnection)));
    assert!(env.server.requests().is_empty());
}

#[test]
fn a_game_of_a_person_never_resumes_as_engine_game_and_vice_versa() {
    let env = env_enabled();
    let (run, _feed, _shared) = started(&env, &[], EnginePolicy::default());
    let engine_game = run.game().clone();
    let board = LichessBoard::new(
        env.net.clone(),
        env.clock.clone(),
        EXPLORER_CONNECTION,
        ConnectionRole::Player,
    )
    .unwrap();
    assert!(matches!(
        board.resume_game(engine_game.clone()),
        Err(BoardError::WrongGame)
    ));
    // O contrário: a Partida de pessoa (chave `lichess`) não reabre pela Bot API.
    let person = chess_game::GameSetup::online(
        chess_game::OnlineGame {
            provider: "lichess".to_owned(),
            remote_id: GAME.to_owned(),
            local_color: chess_core::Color::White,
            opponent: chess_game::Opponent::Human,
            white: chess_game::OnlinePlayer {
                name: "Ana".to_owned(),
                rating: None,
            },
            black: chess_game::OnlinePlayer {
                name: "Salma".to_owned(),
                rating: None,
            },
            rated: false,
            days_per_move: None,
        },
        chess_game::Variant::Standard,
        None,
    )
    .unwrap();
    let (engine, _s) = Scripted::boxed(&[]);
    assert!(matches!(
        bot(&env).resume_game(Game::new(person), engine, EnginePolicy::default()),
        Err(BoardError::WrongGame)
    ));
    // Já a Partida da Engine reabre pela Bot API, sem ligar a trava.
    let (engine, _s) = Scripted::boxed(&[]);
    let resumed = bot(&env)
        .resume_game(engine_game, engine, EnginePolicy::default())
        .unwrap();
    assert!(resumed.is_engine_game());
    assert!(!resumed.holds_fair_play());
}

// ---- Política de desafios -------------------------------------------------------------------

fn challenge(variant: &str, speed: &str, rated: bool, title: Option<&str>) -> Challenge {
    serde_json::from_value(json!({
        "id": "chalABC1",
        "challenger": { "id": "alguem", "name": "Alguem", "title": title, "rating": 1800 },
        "variant": { "key": variant, "name": variant },
        "rated": rated,
        "speed": speed,
        "timeControl": { "type": "clock", "limit": 300, "increment": 2 },
        "color": "random",
    }))
    .unwrap()
}

#[test]
fn challenge_policy_decides_by_variant_pace_rating_opponent_and_concurrency() {
    let env = env_enabled();
    let bot = bot(&env);
    let accept = ChallengeDecision::Accept;
    let decline = ChallengeDecision::Decline;
    assert_eq!(
        bot.decide(&challenge("standard", "blitz", false, None), 0),
        accept
    );
    assert_eq!(
        bot.decide(&challenge("fromPosition", "rapid", true, None), 0),
        accept
    );
    // Só xadrez padrão, sempre.
    assert_eq!(
        bot.decide(&challenge("chess960", "blitz", false, None), 0),
        decline(DeclineReason::Standard)
    );
    assert_eq!(
        bot.decide(&challenge("crazyhouse", "blitz", false, None), 0),
        decline(DeclineReason::Standard)
    );
    // Ritmo fora da política.
    assert_eq!(
        bot.decide(&challenge("standard", "bullet", false, None), 0),
        decline(DeclineReason::TimeControl)
    );
    // Só pessoas por padrão.
    assert_eq!(
        bot.decide(&challenge("standard", "blitz", false, Some("BOT")), 0),
        decline(DeclineReason::NoBot)
    );
    // Uma Partida por vez.
    assert_eq!(
        bot.decide(&challenge("standard", "blitz", false, None), 1),
        decline(DeclineReason::Later)
    );

    let policy = ChallengePolicy {
        speeds: vec![ChallengeSpeed::Bullet, ChallengeSpeed::Blitz],
        rated: RatedPolicy::CasualOnly,
        opponents: OpponentPolicy::PeopleAndEngines,
        max_concurrent: 2,
    };
    let open = LichessBot::new(
        env.net.clone(),
        env.clock.clone(),
        LICHESS_ENGINE_CONNECTION,
        ConnectionRole::Engine,
    )
    .unwrap()
    .challenge_policy(policy);
    assert_eq!(
        open.decide(&challenge("standard", "bullet", false, None), 1),
        accept
    );
    assert_eq!(
        open.decide(&challenge("standard", "bullet", false, Some("BOT")), 0),
        accept
    );
    assert_eq!(
        open.decide(&challenge("standard", "blitz", true, None), 0),
        decline(DeclineReason::Casual)
    );
    assert_eq!(
        open.decide(&challenge("standard", "rapid", false, None), 0),
        decline(DeclineReason::TimeControl)
    );
    assert_eq!(
        open.decide(&challenge("standard", "blitz", false, None), 2),
        decline(DeclineReason::Later)
    );
    let only_rated = ChallengePolicy {
        rated: RatedPolicy::RatedOnly,
        opponents: OpponentPolicy::EnginesOnly,
        ..ChallengePolicy::default()
    };
    assert_eq!(
        only_rated.decide(
            open.provider_profile(),
            &challenge("standard", "blitz", true, None),
            0
        ),
        decline(DeclineReason::OnlyBot)
    );
    assert_eq!(
        only_rated.decide(
            open.provider_profile(),
            &challenge("standard", "blitz", false, Some("BOT")),
            0
        ),
        decline(DeclineReason::Rated)
    );
}

#[test]
fn declined_challenge_tells_the_provider_why() {
    let env = env_enabled();
    env.server.route("/api/challenge/chalABC1/decline", ok());
    let bot = bot(&env);
    let answer = bot.answer_challenge(&challenge("chess960", "blitz", false, None), 0);
    assert_eq!(
        answer.decision,
        ChallengeDecision::Decline(DeclineReason::Standard)
    );
    assert!(answer.request.wait().is_ok());
    let sent = env
        .server
        .requests()
        .into_iter()
        .find(|r| r.path == "/api/challenge/chalABC1/decline")
        .unwrap();
    assert_eq!(String::from_utf8(sent.body).unwrap(), "reason=standard");
    assert!(
        env.server
            .requests()
            .iter()
            .all(|r| !r.path.ends_with("/accept")),
        "recusar nunca aceita"
    );
}

// ---- Conversão em BOT -----------------------------------------------------------------------

#[test]
fn upgrading_the_account_to_bot_is_explicit_and_never_automatic() {
    let env = env_enabled();
    let bot = bot(&env);
    // Abrir eventos, desafios, Partidas: nada converte a conta.
    let (mut run, _feed, _shared) = started(&env, &[], EnginePolicy::default());
    run.pump();
    assert!(
        env.server
            .requests()
            .iter()
            .all(|r| !r.path.contains("upgrade")),
        "nenhuma conversão sozinha"
    );
    // A operação existe, separada, e exige a confirmação.
    env.server.route("/api/bot/account/upgrade", ok());
    assert!(IrreversibleConfirmation::WARNING.contains("não tem volta"));
    bot.upgrade_to_bot(IrreversibleConfirmation::user_confirmed())
        .wait()
        .unwrap();
    let sent = env
        .server
        .requests()
        .into_iter()
        .find(|r| r.path == "/api/bot/account/upgrade")
        .unwrap();
    assert_eq!(sent.method, "POST");
    assert_eq!(
        sent.header("authorization"),
        Some(format!("Bearer {LICHESS_ENGINE_TOKEN}").as_str())
    );
}
