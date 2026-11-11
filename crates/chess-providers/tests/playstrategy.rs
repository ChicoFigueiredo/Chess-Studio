#![allow(clippy::unwrap_used)]
//! PlayStrategy como Provedor da família do Lichess (#234): o mesmo cliente com o perfil do
//! PlayStrategy (perfil, Partidas públicas, Board, autorização), contra o servidor falso
//! (CX-17: nenhuma chamada real). As respostas são **sintéticas**, no formato da família de
//! API do Lichess que a documentação do PlayStrategy descreve (https://playstrategy.org/api,
//! lida em 11/11/2026; a página é uma aplicação que não entrega o texto das rotas sem
//! navegador, então os formatos vêm do contrato da família). Nenhum teste do Lichess muda.

mod common;

use chess_game::PolicyKind;
use chess_net::testing::{FakeResponse, LiveFeed};
use chess_net::{ConnectionId, ProviderId, Secret};
use chess_providers::{
    Backoff, BoardError, ConnectionRole, GameInfo, GamesEvent, GamesFilter, LichessAuth,
    LichessBoard, LichessBot, MastersFilter, ProviderError, ProviderProfile, SessionNotice,
};
use common::*;
use serde_json::{Value, json};
use std::time::Duration;

const WAIT: Duration = Duration::from_secs(10);
const GAME: &str = "Ps4rT9xQ";

fn env_ps() -> Env {
    let env = env();
    store_stage5_tokens(&env);
    env.net.set_enabled(ProviderId::PlayStrategy, true).unwrap();
    env
}

fn board(env: &Env) -> LichessBoard {
    LichessBoard::with_profile(
        ProviderProfile::PLAYSTRATEGY,
        env.net.clone(),
        env.clock.clone(),
        PLAYSTRATEGY_CONNECTION,
        ConnectionRole::Player,
    )
    .unwrap()
    .backoff(Backoff {
        base: Duration::from_secs(2),
        max: Duration::from_secs(16),
    })
    .jitter_seed(3)
}

fn info(variant: &str) -> GameInfo {
    serde_json::from_value(json!({
        "fullId": format!("{GAME}ab12"),
        "gameId": GAME,
        "fen": "rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 0 1",
        "color": "white",
        "lastMove": "",
        "source": "lobby",
        "status": { "id": 20, "name": "started" },
        "variant": { "key": variant, "name": variant },
        "speed": "rapid",
        "perf": "rapid",
        "rated": false,
        "hasMoved": false,
        "opponent": { "id": "jorge", "username": "Jorge", "rating": 1500 },
        "isMyTurn": true,
        "secondsLeft": 600,
        "id": GAME,
    }))
    .unwrap()
}

fn full(variant: &str, moves: &str) -> Value {
    json!({
        "type": "gameFull",
        "id": GAME,
        "variant": { "key": variant, "name": variant },
        "speed": "rapid",
        "rated": false,
        "white": { "id": "ana", "name": "Ana", "rating": 1450 },
        "black": { "id": "jorge", "name": "Jorge", "rating": 1500 },
        "initialFen": "startpos",
        "clock": { "initial": 600_000, "increment": 5_000 },
        "state": {
            "type": "gameState", "moves": moves, "wtime": 600_000, "btime": 600_000,
            "winc": 5_000, "binc": 5_000, "status": "started"
        },
    })
}

// ---- Perfil e Partidas públicas -------------------------------------------------------------

#[test]
fn profile_comes_from_the_playstrategy_host_with_its_own_provenance() {
    let env = env_ps();
    env.server.route(
        "/api/user/ana",
        FakeResponse::json(
            r#"{"id":"ana","username":"Ana","perfs":{"rapid":{"games":10,"rating":1450,"prov":false}},"count":{"all":12}}"#,
        ),
    );

    let answer = env.playstrategy.profile("ana").wait().expect("perfil");

    assert_eq!(answer.value.username, "Ana");
    assert_eq!(answer.value.games, 12);
    assert_eq!(answer.provenance.provider, ProviderId::PlayStrategy);
    assert!(
        answer.provenance.source.contains("playstrategy.org"),
        "{}",
        answer.provenance.source
    );
    // Público: nenhum token sai.
    assert!(
        env.server
            .requests()
            .iter()
            .all(|r| r.header("authorization").is_none())
    );
}

#[test]
fn each_provider_switch_is_independent() {
    let env = env();
    env.net.set_enabled(ProviderId::Lichess, true).unwrap();
    let error = env.playstrategy.profile("ana").wait().unwrap_err();
    assert_eq!(
        error,
        ProviderError::Disabled {
            provider: ProviderId::PlayStrategy
        }
    );
    assert!(env.server.requests().is_empty(), "nada saiu");
}

#[test]
fn public_games_skip_variants_that_are_not_standard_chess() {
    let env = env_ps();
    let body = concat!(
        "{\"id\":\"aaaaaaaa\",\"variant\":\"standard\",\"pgn\":\"[Event \\\"x\\\"]\\n\\n1. e4 *\\n\\n\"}\n",
        "{\"id\":\"bbbbbbbb\",\"variant\":\"shogi\",\"pgn\":\"[Event \\\"y\\\"]\\n\\n1. P-76 *\\n\\n\"}\n",
        "{\"id\":\"cccccccc\",\"variant\":\"crazyhouse\",\"pgn\":\"[Event \\\"z\\\"]\\n\\n1. e4 *\\n\\n\"}\n",
    );
    env.server.route(
        "/api/games/user/ana",
        FakeResponse::ok(body).header("content-type", "application/x-ndjson"),
    );

    let mut feed = env.playstrategy.games("ana", &GamesFilter::default());
    let mut chunks = Vec::new();
    let mut last = None;
    assert!(eventually(WAIT, || {
        while let Some(event) = feed.try_next() {
            match event {
                GamesEvent::Chunk(pgn) => chunks.push(pgn),
                other => {
                    last = Some(other);
                    return true;
                }
            }
        }
        false
    }));

    assert_eq!(last, Some(GamesEvent::Finished));
    assert_eq!(chunks.len(), 1, "só o xadrez padrão");
    assert_eq!(feed.progress().skipped, 2);
    assert_eq!(feed.provenance().provider, ProviderId::PlayStrategy);
}

#[test]
fn what_playstrategy_does_not_offer_is_refused_before_any_request() {
    let env = env_ps();
    let not_offered = |what: &'static str| ProviderError::NotOffered {
        provider: ProviderId::PlayStrategy,
        what,
    };
    assert_eq!(
        env.playstrategy
            .masters(START, &MastersFilter::default())
            .wait()
            .unwrap_err(),
        not_offered("o Explorador de aberturas")
    );
    assert_eq!(
        env.playstrategy.cloud_eval(START, 1).wait().unwrap_err(),
        not_offered("a avaliação em cache")
    );
    assert_eq!(
        env.playstrategy.tablebase(KPK).wait().unwrap_err(),
        not_offered("os finais exatos")
    );
    assert_eq!(
        env.playstrategy.fide_player(1503014).wait().unwrap_err(),
        not_offered("o rating FIDE")
    );
    let message = env
        .playstrategy
        .fide_search("carlsen")
        .wait()
        .unwrap_err()
        .to_string();
    assert!(message.contains("PlayStrategy") && message.contains("nada foi enviado"));
    assert!(env.server.requests().is_empty(), "nenhuma rede");
}

// ---- Board ----------------------------------------------------------------------------------

#[test]
fn simulated_board_game_plays_one_move_through_the_playstrategy_board_api() {
    let env = env_ps();
    let feed = LiveFeed::new();
    env.server.route(
        &format!("/api/board/game/stream/{GAME}"),
        FakeResponse::live(&feed),
    );
    env.server.route(
        &format!("/api/board/game/{GAME}/move/e2e4"),
        FakeResponse::json(r#"{"ok":true}"#),
    );
    let mut session = board(&env).open_game(&info("standard"));
    assert!(feed.wait_connected(WAIT));
    feed.push(&full("standard", "").to_string());
    assert!(eventually(WAIT, || {
        session.poll();
        session.game().is_some()
    }));

    let game = session.game().unwrap();
    assert_eq!(game.setup().policy(), PolicyKind::Online);
    let online = game.setup().online_game().unwrap();
    assert_eq!(online.provider, "playstrategy", "chave do catálogo");
    assert!(!online.is_engine_game());
    assert!(
        env.net.fair_play_guard().is_engaged(),
        "contra pessoa: a trava liga"
    );

    session.play("e2e4".parse().unwrap()).unwrap();
    assert!(eventually(WAIT, || {
        env.server
            .requests()
            .iter()
            .filter(|r| r.method == "POST" && r.path.ends_with("/move/e2e4"))
            .count()
            == 1
    }));
    let post = env
        .server
        .requests()
        .into_iter()
        .find(|r| r.method == "POST")
        .unwrap();
    assert_eq!(
        post.header("authorization"),
        Some(format!("Bearer {PLAYSTRATEGY_TOKEN}").as_str()),
        "pela Conexão do PlayStrategy"
    );

    feed.push(&json!({ "type": "gameState", "moves": "e2e4", "wtime": 600_000, "btime": 600_000, "status": "started" }).to_string());
    assert!(eventually(WAIT, || {
        session.poll();
        session.game().unwrap().snapshot().history.len() == 1
    }));
    assert!(session.game().unwrap().sent_move().is_none());
}

#[test]
fn non_standard_playstrategy_variant_is_refused_and_nothing_is_created() {
    let env = env_ps();
    for variant in ["shogi", "xiangqi", "international", "chess960"] {
        let feed = LiveFeed::new();
        env.server.route(
            &format!("/api/board/game/stream/{GAME}"),
            FakeResponse::live(&feed),
        );
        let mut session = board(&env).open_game(&info(variant));
        assert!(feed.wait_connected(WAIT), "{variant}");
        feed.push(&full(variant, "").to_string());
        let mut notices = Vec::new();
        assert!(eventually(WAIT, || {
            notices.extend(session.poll().notices);
            notices
                .iter()
                .any(|n| matches!(n, SessionNotice::Closed(_)))
        }));
        assert!(session.game().is_none(), "{variant}: nada criado");
        assert!(notices.iter().any(|n| matches!(
            n,
            SessionNotice::Closed(BoardError::UnsupportedVariant(v)) if v == variant
        )));
        assert!(!env.net.fair_play_guard().is_engaged());
    }
}

#[test]
fn player_and_engine_connections_stay_apart_on_playstrategy_too() {
    let env = env_ps();
    let as_engine = LichessBoard::with_profile(
        ProviderProfile::PLAYSTRATEGY,
        env.net.clone(),
        env.clock.clone(),
        PLAYSTRATEGY_ENGINE_CONNECTION,
        ConnectionRole::Engine,
    );
    assert!(matches!(as_engine, Err(BoardError::EngineConnection)));
    let as_player = LichessBot::with_profile(
        ProviderProfile::PLAYSTRATEGY,
        env.net.clone(),
        env.clock.clone(),
        PLAYSTRATEGY_CONNECTION,
        ConnectionRole::Player,
    );
    assert!(matches!(as_player, Err(BoardError::PlayerConnection)));
}

#[test]
fn challenge_in_another_variant_is_refused_without_a_request() {
    let env = env_ps();
    let challenge: chess_providers::Challenge = serde_json::from_value(json!({
        "id": "chal1234",
        "challenger": { "id": "jorge", "name": "Jorge", "rating": 1500 },
        "variant": { "key": "shogi", "name": "Shogi" },
        "rated": false,
        "speed": "rapid",
        "timeControl": { "type": "clock", "limit": 600, "increment": 5 },
        "color": "random",
    }))
    .unwrap();
    let error = board(&env).accept_challenge(&challenge).wait().unwrap_err();
    assert_eq!(
        error,
        ProviderError::VariantRefused {
            provider: ProviderId::PlayStrategy,
            variant: "shogi".to_owned()
        }
    );
    assert!(env.server.requests().is_empty());
}

// ---- Autorização ----------------------------------------------------------------------------

#[test]
fn authorization_uses_the_playstrategy_endpoints_and_scopes() {
    let env = env_ps();
    let auth = LichessAuth::with_profile(
        ProviderProfile::PLAYSTRATEGY,
        env.net.clone(),
        env.vault.clone(),
        env.clock.clone(),
    );
    let flow = auth
        .authorize(ConnectionId(9), ConnectionRole::Engine)
        .unwrap();
    assert!(
        flow.url().starts_with("https://playstrategy.org/oauth?"),
        "{}",
        flow.url()
    );
    assert!(flow.url().contains("scope=bot%3Aplay"), "{}", flow.url());
    assert!(!flow.url().contains("lichess"));
    flow.cancel();
}

#[test]
fn personal_token_is_checked_at_playstrategy_and_errors_name_the_provider() {
    let env = env_ps();
    let auth = LichessAuth::with_profile(
        ProviderProfile::PLAYSTRATEGY,
        env.net.clone(),
        env.vault.clone(),
        env.clock.clone(),
    );
    env.server.route(
        "/api/account",
        FakeResponse::json(r#"{"id":"ana","username":"Ana"}"#).header("x-oauth-scopes", "bot:play"),
    );
    // Conexão de jogador com token de Engine: recusado, com o nome do PlayStrategy.
    let mut task = auth.personal_token(
        ConnectionId(10),
        ConnectionRole::Player,
        Secret::new("ps_token_de_engine_123"),
    );
    let error = task.wait_timeout(WAIT).unwrap().unwrap_err();
    assert!(matches!(
        error,
        chess_providers::AuthError::WrongScopes { .. }
    ));
    let text = error.describe(auth.provider_profile());
    assert!(text.contains("board:play"), "{text}");
    let disabled = chess_providers::AuthError::Disabled.describe(auth.provider_profile());
    assert!(disabled.contains("PlayStrategy") && !disabled.contains("Lichess"));
}

#[test]
fn the_lichess_defaults_did_not_change() {
    let env = env();
    assert_eq!(*env.lichess.provider_profile(), ProviderProfile::LICHESS);
    assert_eq!(
        ProviderProfile::of(ProviderId::PlayStrategy),
        Some(ProviderProfile::PLAYSTRATEGY)
    );
    assert_eq!(ProviderProfile::of(ProviderId::ChessCom), None);
    assert_eq!(ConnectionRole::Player.scope(), "board:play");
    assert_eq!(ConnectionRole::Engine.scope(), "bot:play");
}
