#![allow(clippy::unwrap_used)]
//! Board API do Lichess sem Partida aberta (#230): fluxo de eventos da Conexão, desafios,
//! Engine do Lichess, Partidas em andamento e busca de adversário com os limites da API.
//! Respostas **sintéticas** no formato da documentação oficial (lichess-org/api, 11/11/2026),
//! contra o servidor falso (CX-17).

mod common;

use chess_core::Color;
use chess_net::testing::{FakeResponse, LiveFeed};
use chess_providers::{
    Backoff, BoardError, ChallengeSpec, ConnectionEvent, ConnectionRole, DeclineReason,
    LichessBoard, OnlineTime, ProviderError, Seek, SeekState,
};
use common::*;
use serde_json::json;
use std::time::Duration;

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
    .jitter_seed(5)
}

fn body_of(env: &Env, path: &str) -> String {
    let request = env
        .server
        .requests()
        .into_iter()
        .rev()
        .find(|r| r.path == path)
        .unwrap_or_else(|| panic!("sem pedido para {path}"));
    assert_eq!(request.method, "POST");
    assert_eq!(
        request.header("authorization"),
        Some(format!("Bearer {EXPLORER_TOKEN}").as_str()),
        "pela Conexão do jogador"
    );
    String::from_utf8(request.body).unwrap()
}

fn challenge_json(id: &str, variant: &str) -> serde_json::Value {
    json!({
        "id": id,
        "url": format!("https://lichess.org/{id}"),
        "status": "created",
        "challenger": { "id": "adriana", "name": "Adriana", "rating": 1627, "provisional": true },
        "destUser": { "id": "gabriela", "name": "Gabriela", "rating": 1603, "online": true },
        "variant": { "key": variant, "name": variant, "short": "Std" },
        "rated": false,
        "speed": "rapid",
        "timeControl": { "type": "clock", "limit": 600, "increment": 5, "show": "10+5" },
        "color": "random",
        "finalColor": "black",
        "perf": { "icon": "", "name": "Rapid" },
    })
}

fn game_info(id: &str) -> serde_json::Value {
    json!({
        "fullId": format!("{id}91cB"),
        "gameId": id,
        "fen": "rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 0 1",
        "color": "black",
        "lastMove": "",
        "source": "friend",
        "status": { "id": 20, "name": "started" },
        "variant": { "key": "standard", "name": "Standard" },
        "speed": "correspondence",
        "perf": "correspondence",
        "rated": true,
        "hasMoved": false,
        "opponent": { "id": "aaron", "username": "Aaron", "rating": 1879 },
        "isMyTurn": false,
        "secondsLeft": 259_200,
        "compat": { "bot": false, "board": true },
        "id": id,
    })
}

#[test]
fn fluxo_de_eventos_traduz_desafios_e_partidas_e_reconecta_com_backoff() {
    let env = env_enabled();
    let first = LiveFeed::new();
    let second = LiveFeed::new();
    env.server.route_seq(
        "/api/stream/event",
        vec![FakeResponse::live(&first), FakeResponse::live(&second)],
    );
    let mut events = board(&env).events();
    assert!(first.wait_connected(WAIT));
    first.push(&json!({ "type": "challenge", "challenge": challenge_json("qUfdwN17", "standard"), "compat": { "bot": false, "board": true } }).to_string());
    first.push(&json!({ "type": "challengeCanceled", "challenge": challenge_json("cancel01", "standard") }).to_string());
    first.push(&json!({ "type": "challengeDeclined", "challenge": challenge_json("declin01", "standard") }).to_string());
    first.push(&json!({ "type": "gameStart", "game": game_info("abcd1234") }).to_string());
    let mut finish = game_info("abcd1234");
    finish["status"] = json!({ "id": 31, "name": "resign" });
    finish["winner"] = json!("white");
    first.push(&json!({ "type": "gameFinish", "game": finish }).to_string());
    first.push(&json!({ "type": "algoNovo", "x": 1 }).to_string());

    let mut got = Vec::new();
    assert!(eventually(WAIT, || {
        got.extend(events.poll());
        got.len() >= 5
    }));
    match &got[0] {
        ConnectionEvent::Challenge(c) => {
            assert_eq!(c.id, "qUfdwN17");
            assert_eq!(c.challenger.name, "Adriana");
            assert_eq!(
                c.time,
                OnlineTime::RealTime {
                    base_s: 600,
                    increment_s: 5
                }
            );
            assert_eq!(c.variant, "standard");
        }
        other => panic!("{other:?}"),
    }
    assert_eq!(
        got[1],
        ConnectionEvent::ChallengeCanceled {
            id: "cancel01".into()
        }
    );
    assert!(matches!(&got[2], ConnectionEvent::ChallengeDeclined { id, .. } if id == "declin01"));
    match &got[3] {
        ConnectionEvent::GameStarted(info) => {
            assert_eq!(info.game_id, "abcd1234");
            assert_eq!(info.color, Color::Black);
            assert_eq!(info.opponent.name, "Aaron");
            assert_eq!(info.speed, "correspondence");
        }
        other => panic!("{other:?}"),
    }
    match &got[4] {
        ConnectionEvent::GameFinished(info) => {
            assert_eq!(info.status.as_deref(), Some("resign"));
            assert_eq!(info.winner, Some(Color::White));
        }
        other => panic!("{other:?}"),
    }
    // O fluxo cai: espera o backoff (relógio falso) e reabre, sem laço.
    first.close();
    assert!(eventually(WAIT, || {
        events.poll();
        events.retry_at().is_some()
    }));
    for _ in 0..10 {
        events.poll();
    }
    assert!(!second.is_connected());
    env.clock.advance(Duration::from_secs(20));
    assert!(eventually(WAIT, || {
        events.poll();
        second.is_connected()
    }));
}

#[test]
fn desafios_engine_e_partidas_em_andamento_vao_aos_enderecos_documentados() {
    let env = env_enabled();
    let board = board(&env);
    env.server.route(
        "/api/challenge/Aaron",
        FakeResponse::json(&challenge_json("chal0001", "standard").to_string()),
    );
    let spec = ChallengeSpec {
        time: OnlineTime::RealTime {
            base_s: 600,
            increment_s: 5,
        },
        rated: false,
        color: Some(Color::White),
    };
    let challenge = board.challenge("Aaron", &spec).wait().unwrap();
    assert_eq!(challenge.value.id, "chal0001");
    let body = body_of(&env, "/api/challenge/Aaron");
    for part in [
        "clock.limit=600",
        "clock.increment=5",
        "rated=false",
        "color=white",
        "variant=standard",
    ] {
        assert!(body.contains(part), "{part} em {body}");
    }
    // Nome com caracteres de caminho: recusado antes da rede.
    assert!(matches!(
        board.challenge("../api/account", &spec).wait(),
        Err(ProviderError::InvalidInput { .. })
    ));

    for (path, call) in [
        ("/api/challenge/chal0001/cancel", 0),
        ("/api/challenge/chal0002/decline", 1),
    ] {
        env.server.route(path, FakeResponse::json(r#"{"ok":true}"#));
        let query = if call == 0 {
            board.cancel_challenge("chal0001")
        } else {
            board.decline_challenge("chal0002", DeclineReason::Later)
        };
        query.wait().unwrap();
    }
    assert!(body_of(&env, "/api/challenge/chal0002/decline").contains("reason=later"));

    // Aceitar: só xadrez padrão.
    let incoming: chess_providers::Challenge =
        serde_json::from_value(challenge_json("chal0003", "standard")).unwrap();
    env.server.route(
        "/api/challenge/chal0003/accept",
        FakeResponse::json(r#"{"ok":true}"#),
    );
    board.accept_challenge(&incoming).wait().unwrap();
    let chess960: chess_providers::Challenge =
        serde_json::from_value(challenge_json("chal0004", "chess960")).unwrap();
    assert!(matches!(
        board.accept_challenge(&chess960).wait(),
        Err(ProviderError::VariantRefused { variant, .. }) if variant == "chess960"
    ));
    assert!(
        !env.server
            .requests()
            .iter()
            .any(|r| r.path == "/api/challenge/chal0004/accept"),
        "variante recusada antes da rede"
    );

    // Engine do Lichess.
    env.server.route(
        "/api/challenge/ai",
        FakeResponse::json(
            r#"{"id":"aiGame01","variant":{"key":"standard","name":"Standard"},"speed":"correspondence","perf":"correspondence","rated":false,"fen":"rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 0 1","turns":0,"source":"ai","status":{"id":20,"name":"started"},"createdAt":1794398400000,"player":"white","fullId":"aiGame01abcd"}"#,
        ),
    );
    let ai_spec = ChallengeSpec {
        time: OnlineTime::Correspondence { days: 3 },
        rated: false,
        color: None,
    };
    assert_eq!(
        board.challenge_engine(3, &ai_spec).wait().unwrap().value,
        "aiGame01"
    );
    let body = body_of(&env, "/api/challenge/ai");
    assert!(
        body.contains("level=3") && body.contains("days=3"),
        "{body}"
    );
    assert!(matches!(
        board.challenge_engine(9, &ai_spec).wait(),
        Err(ProviderError::InvalidInput { .. })
    ));

    // Partidas em andamento, para retomar a correspondência.
    env.server.route(
        "/api/account/playing",
        FakeResponse::json(
            &json!({ "nowPlaying": [game_info("corr0001"), game_info("corr0002")] }).to_string(),
        ),
    );
    let playing = board.playing().wait().unwrap().value;
    assert_eq!(playing.len(), 2);
    assert_eq!(playing[1].game_id, "corr0002");
    assert!(!playing[0].is_my_turn);
}

#[test]
fn um_game_id_hostil_do_provedor_nao_abre_fluxo_nenhum() {
    // SEC-06: o `game_id` entra no caminho da URL; `/`, `..`, `?`, `#` e `%` são recusados.
    for hostile in [
        "x/../../api/account",
        "..",
        "a?b=1",
        "a#b",
        "a%2e%2e",
        "",
        "abcdefghijklmnopqrstu",
    ] {
        let env = env_enabled();
        let board = board(&env);
        env.server.route(
            "/api/account/playing",
            FakeResponse::json(&json!({ "nowPlaying": [game_info(hostile)] }).to_string()),
        );

        let error = board.playing().wait().expect_err(hostile);

        assert!(
            matches!(error, ProviderError::Malformed { .. }),
            "{hostile}: {error:?}"
        );
        let paths: Vec<String> = env
            .server
            .requests()
            .iter()
            .map(|r| r.path.clone())
            .collect();
        assert_eq!(paths, ["/api/account/playing"], "{hostile}");
    }
}

#[test]
fn um_despejo_de_linhas_nao_prende_a_consulta_e_nada_se_perde() {
    // SEC-07: cada consulta (a cada quadro, na interface) trata um número limitado de linhas.
    let env = env_enabled();
    let feed = LiveFeed::new();
    env.server
        .route("/api/stream/event", FakeResponse::live(&feed));
    let mut events = board(&env).events();
    assert!(feed.wait_connected(WAIT));
    let line =
        json!({ "type": "challengeCanceled", "challenge": challenge_json("cancel01", "standard") })
            .to_string();
    for _ in 0..3000 {
        feed.push(&line);
    }
    std::thread::sleep(Duration::from_millis(300));

    let first = events.poll();
    assert!(
        !first.is_empty() && first.len() <= 256,
        "a primeira consulta tratou {} linhas",
        first.len()
    );
    let mut total = first.len();
    assert!(eventually(WAIT, || {
        total += events.poll().len();
        total >= 3000
    }));
    assert_eq!(total, 3000);
}

#[test]
fn busca_de_adversario_respeita_os_limites_da_board_api() {
    let env = env_enabled();
    let board = board(&env);
    // Blitz e mais rápido: a Board API só aceita por desafio direto.
    assert!(matches!(
        board.seek(&Seek::real_time(180, 2)),
        Err(BoardError::TooFast)
    ));
    // Correspondência: dias da lista documentada; resposta imediata com o id.
    assert!(matches!(
        board.seek(&Seek::correspondence(4)),
        Err(BoardError::Provider(ProviderError::InvalidInput { .. }))
    ));
    env.server.route(
        "/api/board/seek",
        FakeResponse::json(r#"{"id":"7EtWq30e"}"#),
    );
    let mut corr = board.seek(&Seek::correspondence(3)).unwrap();
    assert!(eventually(WAIT, || corr.poll()
        == SeekState::Posted {
            id: "7EtWq30e".into()
        }));
    assert!(body_of(&env, "/api/board/seek").contains("days=3"));
    drop(corr);

    // Tempo real: uma busca por vez; fechar cancela.
    let feed = LiveFeed::new();
    env.server
        .route("/api/board/seek", FakeResponse::live(&feed));
    let seeking = board.seek(&Seek::real_time(900, 10)).unwrap();
    assert!(matches!(
        board.seek(&Seek::real_time(900, 10)),
        Err(BoardError::SeekInProgress)
    ));
    assert!(feed.wait_connected(WAIT));
    seeking.cancel();
    drop(seeking);
    // O Lichess manda linhas vazias periódicas na busca; na próxima, a conexão fecha.
    assert!(eventually(WAIT, || {
        feed.push("");
        !feed.is_connected()
    }));

    // 429: o Provedor pede pausa; a busca falha tipada e nada sai antes do prazo.
    env.server.route(
        "/api/board/seek",
        FakeResponse::status(429).header("retry-after", "60"),
    );
    let mut limited = board.seek(&Seek::real_time(900, 10)).unwrap();
    assert!(eventually(WAIT, || matches!(
        limited.poll(),
        SeekState::Failed(ProviderError::RateLimited { .. })
    )));
    drop(limited);
    let before = env.server.requests().len();
    let mut again = board.seek(&Seek::real_time(900, 10)).unwrap();
    assert!(eventually(WAIT, || matches!(
        again.poll(),
        SeekState::Failed(ProviderError::RateLimited { .. })
    )));
    assert_eq!(env.server.requests().len(), before, "limitado: nada saiu");
}
