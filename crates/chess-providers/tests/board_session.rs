#![allow(clippy::unwrap_used)]
//! Partida online pela Board API do Lichess (#230), contra o servidor falso e o relógio
//! falso (CX-17: nada de serviço real). Todas as respostas são **sintéticas**, escritas no
//! formato da documentação oficial (lichess-org/api, `GameFullEvent`, `GameStateEvent`,
//! `GameEventInfo`, `ChallengeJson`, conferida em 11/11/2026); nenhuma foi gravada de uma
//! Partida real.

mod common;

use chess_core::{Color, Move};
use chess_game::{EndReason, Event, Game, GameResult, PolicyKind};
use chess_net::Clock;
use chess_net::testing::{FakeResponse, LiveFeed};
use chess_providers::{
    Backoff, BoardError, ConnectionRole, GameInfo, LichessBoard, LinkStatus, OnlineGameSession,
    SessionNotice,
};
use chess_store::{GameMetadata, Store};
use common::*;
use serde_json::{Value, json};
use std::time::Duration;

const GAME: &str = "qP8XaFtN";
const WAIT: Duration = Duration::from_secs(10);

fn board(env: &Env) -> LichessBoard {
    LichessBoard::new(
        env.net.clone(),
        env.clock.clone(),
        EXPLORER_CONNECTION,
        ConnectionRole::Player,
    )
    .unwrap()
    .backoff(Backoff {
        base: Duration::from_secs(2),
        max: Duration::from_secs(16),
    })
    .jitter_seed(11)
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

/// `gameState` sintético.
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

/// `gameFull` sintético de uma Partida rápida 10+5, brancas Kenneth (o jogador local).
fn full(state: Value) -> Value {
    json!({
        "type": "gameFull",
        "id": GAME,
        "variant": { "key": "standard", "name": "Standard", "short": "Std" },
        "speed": "rapid",
        "perf": { "name": "Rapid" },
        "rated": false,
        "createdAt": 1_794_398_400_000_i64,
        "white": player("kenneth", "Kenneth", 1566),
        "black": player("salma", "Salma", 1662),
        "initialFen": "startpos",
        "clock": { "initial": 600_000, "increment": 5_000 },
        "state": state,
    })
}

/// `GameEventInfo` sintético (gameStart e `/api/account/playing`).
fn info_json(color: &str, opponent: Value, speed: &str) -> Value {
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

fn human_info(color: &str) -> GameInfo {
    let value = info_json(
        color,
        json!({ "id": "salma", "username": "Salma", "rating": 1662 }),
        "rapid",
    );
    serde_json::from_value(value).unwrap()
}

fn mv(uci: &str) -> Move {
    uci.parse().unwrap()
}

struct Run {
    session: OnlineGameSession,
    created: bool,
    events: Vec<Event>,
    notices: Vec<SessionNotice>,
}

impl Run {
    fn new(session: OnlineGameSession) -> Self {
        Self {
            session,
            created: false,
            events: Vec::new(),
            notices: Vec::new(),
        }
    }

    fn pump(&mut self) {
        let polled = self.session.poll();
        self.created |= polled.created;
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

    fn history(&self) -> Vec<String> {
        self.session.game().map_or_else(Vec::new, |g| {
            g.snapshot()
                .history
                .iter()
                .map(|h| h.mv.to_string())
                .collect()
        })
    }

    fn plies(&self) -> usize {
        self.history().len()
    }

    fn feed(&mut self, feed: &LiveFeed, value: &Value, plies: usize) {
        feed.push(&value.to_string());
        assert!(
            self.until(|r| r.plies() == plies),
            "esperava {plies} meios-lances, há {:?}",
            self.history()
        );
    }
}

fn posts_to(env: &Env, path: &str) -> usize {
    env.server
        .requests()
        .iter()
        .filter(|r| r.method == "POST" && r.path == path)
        .count()
}

fn guard_engaged(env: &Env) -> bool {
    env.net.fair_play_guard().is_engaged()
}

/// Sessão aberta e viva, com o `gameFull` inicial aplicado.
fn started(env: &Env, initial: Value) -> (Run, LiveFeed) {
    let feed = LiveFeed::new();
    env.server.route(&stream_path(), FakeResponse::live(&feed));
    let mut run = Run::new(board(env).open_game(&human_info("white")));
    assert!(feed.wait_connected(WAIT));
    feed.push(&initial.to_string());
    assert!(run.until(|r| r.session.game().is_some()));
    (run, feed)
}

// ---- Partida completa ----------------------------------------------------------------------

#[test]
fn partida_completa_buscar_comecar_lances_relogios_mate_e_o_log_reabre_igual() {
    let env = env_enabled();
    let board = board(&env);
    let events_feed = LiveFeed::new();
    let seek_feed = LiveFeed::new();
    env.server
        .route("/api/stream/event", FakeResponse::live(&events_feed));
    env.server
        .route("/api/board/seek", FakeResponse::live(&seek_feed));

    // Buscar adversário: fluxo de eventos primeiro, depois a busca (documentação do Lichess).
    let mut connection = board.events();
    assert!(events_feed.wait_connected(WAIT));
    let mut seeking = board
        .seek(&chess_providers::Seek::real_time(600, 5))
        .unwrap();
    assert!(seek_feed.wait_connected(WAIT));
    let seek = env
        .server
        .requests()
        .into_iter()
        .find(|r| r.path == "/api/board/seek")
        .unwrap();
    let body = String::from_utf8(seek.body).unwrap();
    assert!(
        body.contains("time=10") && body.contains("increment=5"),
        "{body}"
    );
    assert!(body.contains("variant=standard"), "{body}");
    assert_eq!(seeking.poll(), chess_providers::SeekState::Waiting);

    // Pareado: o Lichess fecha a busca e avisa no fluxo de eventos.
    let start = json!({ "type": "gameStart", "game": info_json(
        "white",
        json!({ "id": "salma", "username": "Salma", "rating": 1662 }),
        "rapid",
    ) });
    events_feed.push(&start.to_string());
    seek_feed.close();
    assert!(eventually(WAIT, || matches!(
        seeking.poll(),
        chess_providers::SeekState::Ended
    )));
    let mut started_info = None;
    assert!(eventually(WAIT, || {
        for event in connection.poll() {
            if let chess_providers::ConnectionEvent::GameStarted(info) = event {
                started_info = Some(info);
            }
        }
        started_info.is_some()
    }));
    let info = started_info.unwrap();
    assert_eq!(info.color, Color::White);
    assert_eq!(info.game_id, GAME);

    // Começar: a trava liga antes do primeiro Lance.
    let feed = LiveFeed::new();
    env.server.route(&stream_path(), FakeResponse::live(&feed));
    let mut run = Run::new(board.open_game(&info));
    assert!(guard_engaged(&env), "trava ligada desde o início");
    assert!(feed.wait_connected(WAIT));
    feed.push(&full(state("", 600_000, 600_000, "started")).to_string());
    assert!(run.until(|r| r.session.game().is_some()));
    assert!(run.created);
    assert_eq!(run.game().setup().policy(), PolicyKind::Online);
    assert_eq!(run.session.link(), LinkStatus::Live);

    // Lances dos dois lados com os relógios do Provedor.
    let script = [
        ("e2e4", "e2e4 e7e5", 600_000, 598_000),
        ("f1c4", "e2e4 e7e5 f1c4 b8c6", 597_000, 596_500),
        ("d1h5", "e2e4 e7e5 f1c4 b8c6 d1h5 g8f6", 590_000, 580_000),
    ];
    let mut moves_so_far = String::new();
    for (mine, after_reply, wtime, btime) in script {
        env.server.route(&move_path(mine), ok());
        run.session.play(mv(mine)).unwrap();
        assert!(run.game().sent_move().is_some());
        assert!(env.server.wait_for_requests(1, WAIT));
        assert!(eventually(WAIT, || posts_to(&env, &move_path(mine)) == 1));
        let plies = run.plies();
        moves_so_far = if moves_so_far.is_empty() {
            mine.to_owned()
        } else {
            format!("{moves_so_far} {mine}")
        };
        run.feed(
            &feed,
            &state(&moves_so_far, wtime, btime, "started"),
            plies + 1,
        );
        assert_eq!(run.game().sent_move(), None, "confirmado pelo Provedor");
        moves_so_far = after_reply.to_owned();
        run.feed(
            &feed,
            &state(&moves_so_far, wtime, btime, "started"),
            plies + 2,
        );
        let clock = run.game().snapshot().clock.unwrap();
        assert_eq!((clock.white_ms, clock.black_ms), (wtime, btime));
    }
    env.server.route(&move_path("h5f7"), ok());
    run.session.play(mv("h5f7")).unwrap();
    assert!(eventually(WAIT, || posts_to(&env, &move_path("h5f7")) == 1));
    assert!(guard_engaged(&env));
    let mate = with(
        state(&format!("{moves_so_far} h5f7"), 589_000, 580_000, "mate"),
        "winner",
        json!("white"),
    );
    feed.push(&mate.to_string());
    assert!(run.until(|r| r.game().snapshot().is_over()));
    let outcome = run.game().snapshot().outcome.unwrap();
    assert_eq!(outcome.result, GameResult::WhiteWins);
    assert_eq!(outcome.reason, EndReason::Checkmate);
    assert!(!guard_engaged(&env), "o Provedor confirmou o fim");
    feed.close();
    assert!(run.until(|r| r.session.link() == LinkStatus::Closed));

    // Cada Lance foi enviado uma única vez.
    for uci in ["e2e4", "f1c4", "d1h5", "h5f7"] {
        assert_eq!(posts_to(&env, &move_path(uci)), 1, "{uci}");
    }
    // O log (gravado como a interface gravaria) reabre igual.
    let mut store = Store::open_in_memory().unwrap();
    let id = store
        .create_game(run.game().setup(), &GameMetadata::default())
        .unwrap();
    store.append_events(id, &run.events).unwrap();
    let reopened = store.load_game(id).unwrap();
    assert_eq!(reopened.snapshot(), run.game().snapshot());
    assert_eq!(reopened.events(), run.game().events());
}

#[test]
fn fim_por_tempo_vem_do_provedor_nunca_do_relogio_local() {
    let env = env_enabled();
    let (mut run, feed) = started(&env, full(state("e2e4 e7e5", 1_000, 2_000, "started")));
    // Muito tempo depois, nada de bandeira local.
    env.clock.advance(Duration::from_secs(60));
    run.pump();
    assert!(!run.game().snapshot().is_over());
    assert!(guard_engaged(&env));
    let flag = with(
        state("e2e4 e7e5", 0, 2_000, "outoftime"),
        "winner",
        json!("black"),
    );
    feed.push(&flag.to_string());
    assert!(run.until(|r| r.game().snapshot().is_over()));
    let outcome = run.game().snapshot().outcome.unwrap();
    assert_eq!(
        (outcome.result, outcome.reason),
        (GameResult::BlackWins, EndReason::Timeout)
    );
    assert!(!guard_engaged(&env));
}

#[test]
fn abandonar_e_pedido_a_trava_so_cai_com_a_confirmacao() {
    let env = env_enabled();
    let resign = format!("/api/board/game/{GAME}/resign");
    env.server.route(&resign, ok());
    let (mut run, feed) = started(&env, full(state("e2e4 e7e5", 600_000, 600_000, "started")));
    run.session.resign().unwrap();
    assert!(eventually(WAIT, || posts_to(&env, &resign) == 1));
    run.pump();
    assert!(!run.game().snapshot().is_over(), "pedido não é fim");
    assert!(guard_engaged(&env), "clicar em abandonar não solta a trava");
    let resigned = with(
        state("e2e4 e7e5", 600_000, 600_000, "resign"),
        "winner",
        json!("black"),
    );
    feed.push(&resigned.to_string());
    assert!(run.until(|r| r.game().snapshot().is_over()));
    assert_eq!(
        run.game().snapshot().outcome.unwrap().reason,
        EndReason::Resignation
    );
    assert!(!guard_engaged(&env));
    assert_eq!(posts_to(&env, &resign), 1);
}

// ---- Intenção enviada ----------------------------------------------------------------------

#[test]
fn intencao_recusada_pelo_provedor_some_e_nunca_vai_ao_log() {
    let env = env_enabled();
    env.server.route(
        &move_path("e2e4"),
        FakeResponse::status(400).header("content-type", "application/json"),
    );
    let (mut run, _feed) = started(&env, full(state("", 600_000, 600_000, "started")));
    run.session.play(mv("e2e4")).unwrap();
    assert!(run.until(|r| r.game().sent_move().is_none()));
    assert!(
        run.notices
            .iter()
            .any(|n| matches!(n, SessionNotice::MoveRefused { mv, .. } if *mv == self::mv("e2e4"))),
        "{:?}",
        run.notices
    );
    assert!(run.history().is_empty());
    assert!(
        !run.events
            .iter()
            .any(|e| matches!(e.kind, chess_game::EventKind::MoveCompleted { .. })),
        "nada de Lance no log: {:?}",
        run.events
    );
    assert_eq!(posts_to(&env, &move_path("e2e4")), 1);
}

#[test]
fn intencao_substituida_por_outro_lance_do_provedor() {
    let env = env_enabled();
    env.server.route(&move_path("e2e4"), ok());
    let (mut run, feed) = started(&env, full(state("", 600_000, 600_000, "started")));
    run.session.play(mv("e2e4")).unwrap();
    assert!(eventually(WAIT, || posts_to(&env, &move_path("e2e4")) == 1));
    run.feed(&feed, &state("d2d4", 600_000, 600_000, "started"), 1);
    assert_eq!(run.history(), ["d2d4"]);
    assert_eq!(run.game().sent_move(), None);
    assert!(
        run.notices
            .iter()
            .any(|n| matches!(n, SessionNotice::MoveReplaced { sent } if *sent == mv("e2e4")))
    );
}

#[test]
fn queda_no_meio_do_envio_nao_repete_o_post_e_confere_o_estado_ao_reconectar() {
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
        let mut run = Run::new(board(&env).open_game(&human_info("white")));
        assert!(first.wait_connected(WAIT));
        first.push(&full(state("", 600_000, 600_000, "started")).to_string());
        assert!(run.until(|r| r.session.game().is_some()));

        run.session.play(mv("e2e4")).unwrap();
        // Sem resposta: a sessão consulta o estado (reabre o fluxo) em vez de reenviar.
        assert!(run.until(|_| second.is_connected()), "reabre o fluxo");
        let moves = if applied_by_server { "e2e4" } else { "" };
        second.push(&full(state(moves, 600_000, 600_000, "started")).to_string());
        assert!(run.until(|r| r.game().sent_move().is_none()));
        if applied_by_server {
            assert_eq!(run.history(), ["e2e4"], "confere com o servidor");
        } else {
            assert!(run.history().is_empty(), "confere com o servidor");
            assert!(
                run.notices.iter().any(
                    |n| matches!(n, SessionNotice::MoveLost { mv } if *mv == self::mv("e2e4"))
                )
            );
        }
        // Mesmo depois, nada de reenvio às cegas.
        for _ in 0..20 {
            run.pump();
        }
        assert_eq!(posts_to(&env, &move_path("e2e4")), 1);
    }
}

// ---- Reconciliação ---------------------------------------------------------------------------

#[test]
fn eventos_duplicados_e_fora_de_ordem_nao_alteram_nem_regridem() {
    let env = env_enabled();
    let (mut run, feed) = started(&env, full(state("", 600_000, 600_000, "started")));
    run.feed(
        &feed,
        &state("e2e4 e7e5 g1f3", 590_000, 595_000, "started"),
        3,
    );
    let log = run.game().events().to_vec();
    for late in [
        state("e2e4 e7e5 g1f3", 590_000, 595_000, "started"),
        state("e2e4", 600_000, 600_000, "started"),
        state("e2e4 e7e5", 595_000, 595_000, "started"),
        state("e2e4 e7e5 g1f3", 590_000, 595_000, "started"),
    ] {
        feed.push(&late.to_string());
    }
    // Um marcador no fim garante que tudo antes dele já foi lido.
    feed.push(
        &json!({"type": "chatLine", "room": "player", "username": "salma", "text": "oi"})
            .to_string(),
    );
    feed.push(&state("e2e4 e7e5 g1f3 b8c6", 590_000, 594_000, "started").to_string());
    assert!(run.until(|r| r.plies() == 4));
    assert_eq!(&run.game().events()[..log.len()], &log[..]);
    assert_eq!(run.history(), ["e2e4", "e7e5", "g1f3", "b8c6"]);
}

#[test]
fn retrato_divergente_vira_aviso_tipado_sem_mexer_no_log() {
    let env = env_enabled();
    let (mut run, feed) = started(&env, full(state("e2e4 e7e5", 600_000, 600_000, "started")));
    let log = run.game().events().to_vec();
    feed.push(&state("d2d4 d7d5 c2c4", 600_000, 600_000, "started").to_string());
    assert!(run.until(|r| {
        r.notices
            .iter()
            .any(|n| matches!(n, SessionNotice::Desynced(_)))
    }));
    assert_eq!(run.game().events(), &log[..]);
    assert!(run.game().snapshot().online.unwrap().desync.is_some());
    assert!(matches!(
        run.session.play(mv("g1f3")),
        Err(BoardError::Rejected(_))
    ));
}

#[test]
fn reconexao_com_backoff_registra_o_intervalo_e_detecta_fim_enquanto_desconectado() {
    let env = env_enabled();
    let first = LiveFeed::new();
    let second = LiveFeed::new();
    env.server.route_seq(
        &stream_path(),
        vec![FakeResponse::live(&first), FakeResponse::live(&second)],
    );
    let mut run = Run::new(board(&env).open_game(&human_info("white")));
    assert!(first.wait_connected(WAIT));
    first.push(&full(state("e2e4 e7e5", 600_000, 600_000, "started")).to_string());
    assert!(run.until(|r| r.session.game().is_some()));
    let dropped_at = env.clock.now();
    first.close(); // o fluxo cai com a Partida em andamento
    assert!(run.until(|r| matches!(r.session.link(), LinkStatus::Retrying { .. })));
    let LinkStatus::Retrying { attempt, at } = run.session.link() else {
        unreachable!()
    };
    assert_eq!(attempt, 1);
    let wait = at.since(dropped_at);
    assert!(
        wait >= Duration::from_secs(1) && wait <= Duration::from_secs(2),
        "backoff com jitter entre metade e o total da base: {wait:?}"
    );
    // Antes do prazo, nada de reconectar em laço.
    for _ in 0..20 {
        run.pump();
    }
    assert!(!second.is_connected());
    env.clock.advance(Duration::from_secs(30));
    assert!(run.until(|_| second.is_connected()));
    // A Partida terminou enquanto o app estava sem fluxo: o retrato completo diz.
    let ended = with(
        state(
            "e2e4 e7e5 d1h5 b8c6 f1c4 g8f6 h5f7",
            500_000,
            500_000,
            "mate",
        ),
        "winner",
        json!("white"),
    );
    second.push(&full(ended).to_string());
    assert!(run.until(|r| r.game().snapshot().is_over()));
    assert_eq!(run.plies(), 7);
    let gaps = run.session.gaps();
    assert_eq!(gaps.len(), 1);
    assert!(gaps[0].from >= dropped_at && gaps[0].to > gaps[0].from);
    assert!(!guard_engaged(&env));
    second.close();
    assert!(run.until(|r| r.session.link() == LinkStatus::Closed));
}

// ---- Começo, trava e correspondência --------------------------------------------------------

#[test]
fn variante_nao_padrao_e_recusada_antes_de_criar_a_partida() {
    let env = env_enabled();
    let feed = LiveFeed::new();
    env.server.route(&stream_path(), FakeResponse::live(&feed));
    let mut run = Run::new(board(&env).open_game(&human_info("white")));
    assert!(feed.wait_connected(WAIT));
    let chess960 = with(
        full(state("", 600_000, 600_000, "started")),
        "variant",
        json!({ "key": "chess960", "name": "Chess960", "short": "960" }),
    );
    feed.push(&chess960.to_string());
    assert!(run.until(|r| r.session.link() == LinkStatus::Closed));
    assert!(run.session.game().is_none(), "nada foi criado");
    assert!(!run.created);
    assert!(run.notices.iter().any(|n| matches!(
        n,
        SessionNotice::Closed(BoardError::UnsupportedVariant(v)) if v == "chess960"
    )));
    assert!(!guard_engaged(&env), "sem Partida, sem trava");
}

#[test]
fn contra_a_engine_do_lichess_a_trava_nao_liga() {
    let env = env_enabled();
    let feed = LiveFeed::new();
    env.server.route(&stream_path(), FakeResponse::live(&feed));
    let info: GameInfo = serde_json::from_value(info_json(
        "white",
        json!({ "id": null, "username": "Stockfish level 3", "ai": 3 }),
        "correspondence",
    ))
    .unwrap();
    assert_eq!(info.opponent.ai_level, Some(3));
    let mut run = Run::new(board(&env).open_game(&info));
    assert!(!guard_engaged(&env));
    assert!(feed.wait_connected(WAIT));
    let vs_ai = with(
        full(state("", 600_000, 600_000, "started")),
        "black",
        json!({ "aiLevel": 3 }),
    );
    feed.push(&vs_ai.to_string());
    assert!(run.until(|r| r.session.game().is_some()));
    assert!(!run.game().online_human_in_progress());
    run.pump();
    assert!(!guard_engaged(&env));
}

#[test]
fn conexao_de_engine_nunca_joga_como_pessoa() {
    let env = env_enabled();
    let refused = LichessBoard::new(
        env.net.clone(),
        env.clock.clone(),
        EXPLORER_CONNECTION,
        ConnectionRole::Engine,
    );
    assert!(matches!(refused, Err(BoardError::EngineConnection)));
}

#[test]
fn correspondencia_fecha_e_reabre_o_app_e_continua_pelo_retrato() {
    let env = env_enabled();
    let path = env.dir.join("partidas.db");
    std::fs::create_dir_all(&env.dir).unwrap();
    let correspondence = |state: Value| {
        let mut value = full(state);
        value.as_object_mut().unwrap().remove("clock");
        value["daysPerTurn"] = json!(3);
        value["speed"] = json!("correspondence");
        value
    };
    let first = LiveFeed::new();
    let second = LiveFeed::new();
    env.server.route_seq(
        &stream_path(),
        vec![FakeResponse::live(&first), FakeResponse::live(&second)],
    );
    env.server.route(&move_path("g1f3"), ok());

    // Primeira abertura do app.
    let id = {
        let mut store = Store::open(&path).unwrap();
        let mut run = Run::new(board(&env).open_game(&human_info("white")));
        assert!(first.wait_connected(WAIT));
        first.push(&correspondence(state("e2e4 e7e5", 0, 0, "started")).to_string());
        assert!(run.until(|r| r.session.game().is_some()));
        let id = store
            .create_game(run.game().setup(), &GameMetadata::default())
            .unwrap();
        run.session.play(mv("g1f3")).unwrap();
        assert!(eventually(WAIT, || posts_to(&env, &move_path("g1f3")) == 1));
        run.feed(&first, &state("e2e4 e7e5 g1f3", 0, 0, "started"), 3);
        store.append_events(id, &run.events).unwrap();
        assert!(guard_engaged(&env));
        store.close_game(id).unwrap();
        id
        // O app fecha: a sessão cai junto.
    };
    assert!(!guard_engaged(&env), "sem a sessão, ninguém segura a trava");

    // Dias depois: reabre do log e continua pelo retrato.
    env.clock.advance(Duration::from_secs(2 * 24 * 3600));
    let mut store = Store::open(&path).unwrap();
    let game = store.load_game(id).unwrap();
    assert_eq!(game.snapshot().history.len(), 3);
    let mut run = Run::new(board(&env).resume_game(game).unwrap());
    assert!(
        guard_engaged(&env),
        "a trava volta com a Partida em andamento"
    );
    assert!(second.wait_connected(WAIT));
    // O adversário jogou enquanto o app estava fechado.
    second.push(&correspondence(state("e2e4 e7e5 g1f3 b8c6", 0, 0, "started")).to_string());
    assert!(run.until(|r| r.plies() == 4));
    assert!(!run.created, "a Partida já existia");
    store.append_events(id, &run.events).unwrap();
    let reopened = store.load_game(id).unwrap();
    assert_eq!(reopened.snapshot(), run.game().snapshot());
    assert_eq!(reopened.snapshot().history[3].san, "Nc6");
}

#[test]
fn pedido_de_volta_de_lance_do_adversario_e_recusado_uma_vez() {
    let env = env_enabled();
    let decline = format!("/api/board/game/{GAME}/takeback/no");
    env.server.route(&decline, ok());
    let (mut run, feed) = started(&env, full(state("e2e4 e7e5", 600_000, 600_000, "started")));
    let asks = with(
        state("e2e4 e7e5", 600_000, 600_000, "started"),
        "btakeback",
        json!(true),
    );
    for _ in 0..3 {
        feed.push(&asks.to_string());
    }
    assert!(eventually(WAIT, || {
        run.pump();
        posts_to(&env, &decline) >= 1
    }));
    for _ in 0..20 {
        run.pump();
    }
    assert_eq!(posts_to(&env, &decline), 1);
    assert_eq!(run.plies(), 2, "nunca há desfazer");
}

#[test]
fn reclamar_vitoria_so_depois_do_prazo_do_adversario_ausente() {
    let env = env_enabled();
    let claim = format!("/api/board/game/{GAME}/claim-victory");
    env.server.route(&claim, ok());
    let (mut run, feed) = started(&env, full(state("e2e4 e7e5", 600_000, 600_000, "started")));
    assert!(matches!(
        run.session.claim_victory(),
        Err(BoardError::NotAllowed(_))
    ));
    feed.push(
        &json!({ "type": "opponentGone", "gone": true, "claimWinInSeconds": 30 }).to_string(),
    );
    assert!(run.until(|r| r.session.opponent_gone().is_some()));
    assert!(matches!(
        run.session.claim_victory(),
        Err(BoardError::NotAllowed(_))
    ));
    env.clock.advance(Duration::from_secs(31));
    run.session.claim_victory().unwrap();
    assert!(eventually(WAIT, || posts_to(&env, &claim) == 1));
    let won = with(
        state("e2e4 e7e5", 600_000, 600_000, "timeout"),
        "winner",
        json!("white"),
    );
    feed.push(&won.to_string());
    assert!(run.until(|r| r.game().snapshot().is_over()));
    assert_eq!(
        run.game().snapshot().outcome.unwrap().reason,
        EndReason::Disconnection
    );
}

#[test]
fn empate_do_lichess_tem_o_motivo_tirado_da_posicao() {
    // `draw` no Lichess não diz o motivo: repetição tripla vem da Posição; senão, acordo.
    let cycle = "g1f3 g8f6 f3g1 f6g8 g1f3 g8f6 f3g1 f6g8";
    for (moves, reason) in [
        (cycle, EndReason::ThreefoldRepetition),
        ("e2e4 e7e5", EndReason::DrawAgreement),
    ] {
        let env = env_enabled();
        let (mut run, feed) = started(&env, full(state("", 600_000, 600_000, "started")));
        feed.push(&state(moves, 500_000, 500_000, "draw").to_string());
        assert!(run.until(|r| r.game().snapshot().is_over()));
        let outcome = run.game().snapshot().outcome.unwrap();
        assert_eq!((outcome.result, outcome.reason), (GameResult::Draw, reason));
        // Os dois passos (Lances, depois o fim) saíram para o log.
        let reopened = Game::try_replay(run.game().setup().clone(), &run.events).unwrap();
        assert_eq!(reopened.snapshot(), run.game().snapshot());
        assert!(!guard_engaged(&env));
    }
}
