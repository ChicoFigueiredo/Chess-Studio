#![allow(clippy::unwrap_used)]
//! Engine remota (Chess-API.com, #234, CX-11) pelo contrato de análise local
//! (`AnalysisEvent`/`AnalysisUpdate`). Servidor falso: a suíte nunca chama o serviço real.
//! Respostas: `chess-api-start`, `chess-api-mate`, `chess-api-black-to-move` e
//! `chess-api-illegal-fen` são **reais** (gravadas em 2026-11-11, `curl -i`, sem chave, só o FEN
//! de posições de exemplo); `chess-api-rate-limited`, `chess-api-illegal-move`,
//! `chess-api-malformed` e `chess-api-multi` são **sintéticas** (marcadas no arquivo).

mod common;

use chess_core::Color;
use chess_net::ProviderId;
use chess_providers::{ChessApi, ProviderError, RemoteEngine};
use chess_uci::{AnalysisEvent, AnalysisUpdate, FailureKind, PositionSpec, Score};
use common::*;
use serde_json::Value;
use std::time::Duration;

const WAIT: Duration = Duration::from_secs(10);

fn setup() -> (Env, RemoteEngine) {
    let env = env();
    env.net.set_enabled(ProviderId::ChessApi, true).unwrap();
    let engine = RemoteEngine::new(ChessApi::new(env.net.clone(), env.clock.clone()));
    (env, engine)
}

/// Próximo `Update` ou `Failed` (pula o `Ready`).
fn next_result(engine: &mut RemoteEngine) -> AnalysisEvent {
    let mut found = None;
    assert!(
        eventually(WAIT, || {
            while let Some(event) = engine.poll() {
                if !matches!(event, AnalysisEvent::Ready { .. }) {
                    found = Some(event);
                    return true;
                }
            }
            false
        }),
        "nenhum resultado"
    );
    found.unwrap()
}

fn update(event: AnalysisEvent) -> AnalysisUpdate {
    match event {
        AnalysisEvent::Update(update) => update,
        other => panic!("esperava Update, veio {other:?}"),
    }
}

fn posts(env: &Env) -> Vec<chess_net::testing::ReceivedRequest> {
    env.server
        .requests()
        .into_iter()
        .filter(|r| r.method == "POST" && r.path == "/v1")
        .collect()
}

// ---- Resposta feliz (real) ------------------------------------------------------------------

#[test]
fn happy_answer_arrives_as_an_ordinary_engine_update_with_only_the_fen_sent() {
    let (env, mut engine) = setup();
    env.server.route("/v1", fixture("chess-api-start.http"));

    let id = engine.analyze(PositionSpec::startpos(), 1);

    // A Engine se apresenta como as locais: `Ready` primeiro, com o nome e o selo do Provedor.
    match engine.poll() {
        Some(AnalysisEvent::Ready { name, .. }) => {
            assert_eq!(name.as_deref(), Some("Stockfish (Chess-API.com)"));
        }
        other => panic!("esperava Ready, veio {other:?}"),
    }
    assert_eq!(engine.badge(), "Remota · Chess-API.com");
    assert_eq!(engine.provider(), ProviderId::ChessApi);

    let update = update(next_result(&mut engine));
    assert_eq!(update.id, id);
    assert_eq!(update.lines.len(), 1);
    let line = &update.lines[0];
    assert_eq!(line.multipv, 1);
    assert_eq!(line.depth, 12);
    assert_eq!(
        line.score,
        Score::Cp(35),
        "centipawns em texto, como no serviço real"
    );
    assert_eq!(
        line.pv[0].to_string(),
        "d2d4",
        "o que o serviço respondeu em 2026-11-11"
    );
    assert_eq!(line.san[0], "d4");
    assert_eq!(line.pv.len(), line.san.len());
    assert!(line.pv.len() > 5, "continuação validada lance a lance");
    assert!(!engine.is_analyzing());
    let origin = engine.last_origin().unwrap();
    assert_eq!(origin.provenance.provider, ProviderId::ChessApi);

    // O que saiu: um POST com só o FEN, a profundidade e as variantes; nenhum segredo.
    let sent = posts(&env);
    assert_eq!(sent.len(), 1);
    assert!(sent[0].header("authorization").is_none());
    let body: Value = serde_json::from_slice(&sent[0].body).unwrap();
    let mut keys: Vec<&str> = body
        .as_object()
        .unwrap()
        .keys()
        .map(String::as_str)
        .collect();
    keys.sort_unstable();
    assert_eq!(keys, ["depth", "fen", "variants"]);
    assert_eq!(
        body["fen"],
        "rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 0 1"
    );
    assert_eq!(body["depth"], 12);
}

#[test]
fn mate_comes_as_a_string_and_stays_in_the_white_point_of_view() {
    let (env, mut engine) = setup();
    env.server.route("/v1", fixture("chess-api-mate.http"));
    engine.analyze(
        PositionSpec::from_fen("6k1/5ppp/8/8/8/8/5PPP/3R2K1 w - - 0 1"),
        1,
    );
    let line = &update(next_result(&mut engine)).lines[0];
    assert_eq!(
        line.score,
        Score::Mate(1),
        "mate «1» em texto, como no serviço real"
    );
    assert_eq!(line.san[0], "Rd8#");
}

#[test]
fn black_to_move_keeps_the_white_point_of_view_like_the_local_engines() {
    let (env, mut engine) = setup();
    env.server
        .route("/v1", fixture("chess-api-black-to-move.http"));
    engine.analyze(PositionSpec::from_fen("7k/8/8/8/8/5q2/8/6K1 b - - 0 1"), 1);
    let line = &update(next_result(&mut engine)).lines[0];
    // As pretas estão ganhando: negativo para as brancas (o serviço manda `-9`, número).
    assert_eq!(line.score, Score::Mate(-9));
    assert_eq!(chess_providers::POV, Color::White);
}

#[test]
fn position_with_moves_sends_only_the_resulting_fen() {
    let (env, mut engine) = setup();
    env.server.route("/v1", fixture("chess-api-start.http"));
    let mut spec = PositionSpec::startpos();
    spec.moves = vec!["e2e4".parse().unwrap(), "e7e5".parse().unwrap()];
    engine.analyze(spec, 1);
    let _ = next_result(&mut engine);
    let body: Value = serde_json::from_slice(&posts(&env)[0].body).unwrap();
    assert_eq!(
        body["fen"],
        "rnbqkbnr/pppp1ppp/8/4p3/4P3/8/PPPP1PPP/RNBQKBNR w KQkq e6 0 2"
    );
    assert!(body.get("moves").is_none());
}

// ---- Variantes (sintética) e validação ------------------------------------------------------

#[test]
fn several_variants_are_numbered_and_the_continuation_stops_at_the_first_illegal_move() {
    let (env, mut engine) = setup();
    env.server.route("/v1", fixture("chess-api-multi.http"));
    engine.analyze(PositionSpec::startpos(), 2);
    let update = update(next_result(&mut engine));
    assert_eq!(update.lines.len(), 2, "o objeto `info` não é variante");
    assert_eq!(update.lines[0].multipv, 1);
    assert_eq!(update.lines[1].multipv, 2);
    let first: Vec<String> = update.lines[0].pv.iter().map(ToString::to_string).collect();
    assert_eq!(first, ["e2e4", "e7e5", "g1f3"], "cortada em a1a1 (ilegal)");
    assert_eq!(update.lines[0].san, ["e4", "e5", "Nf3"]);
    assert_eq!(update.lines[1].score, Score::Cp(28));
    let body: Value = serde_json::from_slice(&posts(&env)[0].body).unwrap();
    assert_eq!(body["variants"], 2);
}

#[test]
fn an_illegal_best_move_is_refused_and_nothing_is_shown() {
    let (env, mut engine) = setup();
    env.server
        .route("/v1", fixture("chess-api-illegal-move.http"));
    engine.analyze(PositionSpec::startpos(), 1);
    match next_result(&mut engine) {
        AnalysisEvent::Failed(failure) => assert_eq!(failure.kind, FailureKind::Protocol),
        other => panic!("esperava Failed, veio {other:?}"),
    }
    assert!(matches!(
        engine.problem(),
        Some(ProviderError::Malformed { detail, .. }) if detail.contains("ilegal")
    ));
    assert!(engine.poll().is_none(), "nenhuma linha vazou");
}

// ---- Limitada, malformada, recusada pelo serviço --------------------------------------------

#[test]
fn rate_limited_waits_and_sends_nothing_before_the_deadline() {
    let (env, mut engine) = setup();
    env.server
        .route("/v1", fixture("chess-api-rate-limited.http"));
    engine.analyze(PositionSpec::startpos(), 1);
    let failure = match next_result(&mut engine) {
        AnalysisEvent::Failed(f) => f,
        other => panic!("esperava Failed, veio {other:?}"),
    };
    assert_eq!(failure.kind, FailureKind::Timeout, "passa sozinho");
    assert!(
        !failure.message.contains("429") && !failure.message.to_lowercase().contains("http"),
        "{}",
        failure.message
    );
    let until = match engine.problem() {
        Some(ProviderError::RateLimited { provider, until }) => {
            assert_eq!(*provider, ProviderId::ChessApi);
            *until
        }
        other => panic!("esperava RateLimited, veio {other:?}"),
    };
    assert!(until >= T0.plus(Duration::from_secs(60)));
    assert_eq!(posts(&env).len(), 1);

    // Antes do prazo: nenhum pedido novo sai.
    engine.analyze(PositionSpec::startpos(), 1);
    let _ = next_result(&mut engine);
    assert_eq!(posts(&env).len(), 1, "respeitou a pausa");
    assert!(matches!(
        engine.problem(),
        Some(ProviderError::RateLimited { .. })
    ));
    assert!(!engine.is_dead(), "a Engine remota segue escolhível");
}

#[test]
fn malformed_answer_becomes_a_typed_problem_with_a_user_message() {
    let (env, mut engine) = setup();
    env.server.route("/v1", fixture("chess-api-malformed.http"));
    engine.analyze(PositionSpec::startpos(), 1);
    let failure = match next_result(&mut engine) {
        AnalysisEvent::Failed(f) => f,
        other => panic!("esperava Failed, veio {other:?}"),
    };
    assert!(matches!(
        engine.problem(),
        Some(ProviderError::Malformed { provider, .. }) if *provider == ProviderId::ChessApi
    ));
    assert!(!failure.message.is_empty());
}

#[test]
fn the_service_error_with_status_ok_is_a_refusal_not_a_move() {
    // Resposta REAL: FEN ilegal volta `200` com `type: "error"`.
    let (env, mut engine) = setup();
    env.server
        .route("/v1", fixture("chess-api-illegal-fen.http"));
    engine.analyze(PositionSpec::from_fen("7k/8/8/8/8/5q2/6K1/8 b - - 0 1"), 1);
    // O núcleo de regras local aceita o FEN (é só ilegal no serviço); o serviço recusa.
    let _ = next_result(&mut engine);
    assert!(matches!(
        engine.problem(),
        Some(ProviderError::InvalidInput { .. }) | Some(ProviderError::Malformed { .. })
    ));
}

#[test]
fn local_input_errors_never_reach_the_network() {
    let (env, mut engine) = setup();
    engine.analyze(PositionSpec::from_fen("isto não é um FEN"), 1);
    assert!(matches!(next_result(&mut engine), AnalysisEvent::Failed(_)));
    assert!(matches!(
        engine.problem(),
        Some(ProviderError::InvalidInput { .. })
    ));
    let mut spec = PositionSpec::startpos();
    spec.moves = vec!["e2e5".parse().unwrap()];
    engine.analyze(spec, 1);
    assert!(matches!(next_result(&mut engine), AnalysisEvent::Failed(_)));
    assert!(env.server.requests().is_empty());
}

// ---- Provedor desligado e Trava de fair play ------------------------------------------------

#[test]
fn provider_off_sends_nothing() {
    let env = env();
    let mut engine = RemoteEngine::new(ChessApi::new(env.net.clone(), env.clock.clone()));
    engine.analyze(PositionSpec::startpos(), 1);
    let failure = match next_result(&mut engine) {
        AnalysisEvent::Failed(f) => f,
        other => panic!("esperava Failed, veio {other:?}"),
    };
    assert_eq!(
        engine.problem(),
        Some(&ProviderError::Disabled {
            provider: ProviderId::ChessApi
        })
    );
    assert!(failure.message.contains("Conexões"), "{}", failure.message);
    assert!(env.server.requests().is_empty(), "nada saiu");
}

#[test]
fn fair_play_lock_refuses_the_remote_engine_because_it_computes() {
    let (env, mut engine) = setup();
    env.server.route("/v1", fixture("chess-api-start.http"));
    let hold = env
        .net
        .fair_play_guard()
        .hold("Partida online contra pessoa");
    engine.analyze(PositionSpec::startpos(), 1);
    let _ = next_result(&mut engine);
    assert_eq!(
        engine.problem(),
        Some(&ProviderError::FairPlayLocked {
            provider: ProviderId::ChessApi
        })
    );
    assert!(env.server.requests().is_empty(), "nada saiu");

    // Fim da Partida: a trava cai e a Engine responde de novo.
    drop(hold);
    engine.analyze(PositionSpec::startpos(), 1);
    let _ = update(next_result(&mut engine));
    assert_eq!(posts(&env).len(), 1);
    assert!(engine.problem().is_none());
}

// ---- Sem troca silenciosa e pedidos obsoletos -----------------------------------------------

#[test]
fn nothing_is_asked_until_the_user_calls_analyze_and_stop_discards_the_answer() {
    let (env, mut engine) = setup();
    env.server.route("/v1", fixture("chess-api-start.http"));
    // Criar e consultar a Engine não gera pedido: só `analyze` (a escolha do usuário).
    for _ in 0..5 {
        let _ = engine.poll();
    }
    assert!(env.server.requests().is_empty());
    engine.analyze(PositionSpec::startpos(), 1);
    engine.stop();
    assert!(!engine.is_analyzing());
    for _ in 0..20 {
        assert!(!matches!(engine.poll(), Some(AnalysisEvent::Update(_))));
        std::thread::sleep(Duration::from_millis(5));
    }
}

#[test]
fn a_newer_request_replaces_the_older_one() {
    let (env, mut engine) = setup();
    env.server.route("/v1", fixture("chess-api-start.http"));
    let _old = engine.analyze(PositionSpec::startpos(), 1);
    let new = engine.analyze(PositionSpec::startpos(), 1);
    let update = update(next_result(&mut engine));
    assert_eq!(update.id, new, "só vale o pedido mais novo");
    let _ = env;
}

#[test]
fn a_hostile_answer_with_a_huge_line_and_many_variants_is_cut_before_validation() {
    // SEC-07: a continuação e a lista de variantes eram validadas inteiras antes do corte.
    let (env, mut engine) = setup();
    let cycle = ["g1f3", "g8f6", "f3g1", "f6g8"];
    let continuation: Vec<&str> = cycle.iter().copied().cycle().take(3_000).collect();
    let item = serde_json::json!({
        "type": "move", "lan": "g1f3", "depth": 12, "eval": 0.2, "centipawns": "20",
        "continuationArr": continuation,
    });
    let items: Vec<&serde_json::Value> = std::iter::repeat_n(&item, 200).collect();
    env.server.route(
        "/v1",
        chess_net::testing::FakeResponse::json(&serde_json::to_string(&items).unwrap()),
    );

    engine.analyze(PositionSpec::startpos(), 3);
    let update = update(next_result(&mut engine));

    assert!(update.lines.len() <= usize::from(chess_uci::MAX_MULTIPV));
    for line in &update.lines {
        assert!(
            line.pv.len() <= 64 && line.san.len() <= 64,
            "{}",
            line.pv.len()
        );
    }
}
