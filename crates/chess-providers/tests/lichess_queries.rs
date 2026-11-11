//! Avaliação em cache, finais exatos, perfil público e FIDE via Lichess.

mod common;

use chess_net::testing::FakeResponse;
use chess_net::{Freshness, ProviderId};
use chess_providers::{Category, Color, ProviderError, Score};
use common::*;
use std::time::Duration;

const LOCKED: ProviderError = ProviderError::FairPlayLocked {
    provider: ProviderId::Lichess,
};

// ---- Avaliação em cache ------------------------------------------------------------

#[test]
fn cloud_eval_returns_depth_nodes_lines_and_provenance() {
    let env = env_enabled();
    env.server
        .route("/api/cloud-eval", fixture("lichess-cloud-eval-start.http"));

    let answer = env.lichess.cloud_eval(START, 2).wait().expect("avaliação");

    let eval = answer.value.expect("há avaliação");
    assert_eq!(eval.depth, 75);
    assert_eq!(eval.nodes, 695_524_000);
    assert_eq!(eval.lines.len(), 2);
    assert_eq!(eval.lines[0].moves[0], "e2e4");
    assert_eq!(eval.lines[0].moves.len(), 10);
    assert_eq!(eval.lines[0].score_white, Score::Centipawns(19));
    assert_eq!(answer.provenance.provider, ProviderId::Lichess);
    assert!(answer.provenance.source.contains("/api/cloud-eval"));
    let sent = &env.server.requests()[0];
    assert_eq!(sent.query_value("multiPv").as_deref(), Some("2"));
}

#[test]
fn cloud_eval_scores_are_white_point_of_view_with_explicit_conversion() {
    let env = env_enabled();
    env.server.route(
        "/api/cloud-eval",
        fixture("lichess-cloud-eval-black-to-move.http"),
    );
    // Pretas jogam e dão mate em 1: o Provedor entrega mate -1 (ponto de vista das Brancas).
    let fen = "rnbqkbnr/pppp1ppp/8/4p3/6P1/5P2/PPPPP2P/RNBQKBNR b KQkq g3 0 2";

    let eval = env
        .lichess
        .cloud_eval(fen, 1)
        .wait()
        .expect("avaliação")
        .value
        .expect("há avaliação");

    let line = &eval.lines[0];
    assert_eq!(chess_providers::POV, Color::White);
    assert_eq!(line.score_white, Score::Mate(-1));
    assert_eq!(line.score_for(Color::Black), Score::Mate(1));
    assert_eq!(line.moves, vec!["d8h4"]);
}

#[test]
fn cloud_eval_reads_a_mate_score_from_the_recorded_endgame() {
    let env = env_enabled();
    env.server.route(
        "/api/cloud-eval",
        fixture("lichess-cloud-eval-endgame-mate.http"),
    );
    let eval = env
        .lichess
        .cloud_eval("8/8/8/3k4/8/8/2K1P3/8 w - - 0 1", 1)
        .wait()
        .expect("avaliação")
        .value
        .expect("há avaliação");
    assert_eq!(eval.lines[0].score_white, Score::Mate(22));
}

#[test]
fn cloud_eval_404_is_a_valid_no_evaluation_remembered_for_a_short_time() {
    let env = env_enabled();
    env.server.route(
        "/api/cloud-eval",
        FakeResponse::status(404).header("content-type", "application/json"),
    );

    let first = env
        .lichess
        .cloud_eval(START, 1)
        .wait()
        .expect("sem avaliação é válido");
    assert_eq!(first.value, None);
    assert_eq!(first.provenance.queried_at, T0);
    assert_eq!(env.server.requests().len(), 1);

    // Dentro dos 10 minutos: nenhuma rede, mesma Origem, marcado como cache.
    env.clock.advance(Duration::from_secs(5 * 60));
    let second = env.lichess.cloud_eval(START, 1).wait().expect("lembrado");
    assert_eq!(second.value, None);
    assert_eq!(second.freshness, Freshness::Cache);
    assert_eq!(second.age, Duration::from_secs(5 * 60));
    assert_eq!(second.provenance, first.provenance);
    assert_eq!(env.server.requests().len(), 1);

    // Passado o prazo, pergunta de novo (a nuvem pode ter avaliado a Posição).
    env.clock.advance(Duration::from_secs(6 * 60));
    env.server
        .route("/api/cloud-eval", fixture("lichess-cloud-eval-start.http"));
    let third = env.lichess.cloud_eval(START, 1).wait().expect("de novo");
    assert!(third.value.is_some());
    assert_eq!(env.server.requests().len(), 2);
}

#[test]
fn cloud_eval_failures_and_gate() {
    let env = env_enabled();
    env.server
        .route("/api/cloud-eval", FakeResponse::status(429));
    assert!(matches!(
        env.lichess.cloud_eval(START, 1).wait(),
        Err(ProviderError::RateLimited { .. })
    ));

    let env = env_enabled();
    env.server
        .route("/api/cloud-eval", FakeResponse::json("[1,2]"));
    assert!(matches!(
        env.lichess.cloud_eval(START, 1).wait(),
        Err(ProviderError::Malformed { .. })
    ));
    assert!(matches!(
        env.lichess.cloud_eval(START, 0).wait(),
        Err(ProviderError::InvalidInput { .. })
    ));
    assert!(matches!(
        env.lichess.cloud_eval("lixo", 1).wait(),
        Err(ProviderError::InvalidInput { .. })
    ));

    // Desligado: nada sai.
    let off = common::env();
    assert_eq!(
        off.lichess
            .cloud_eval(START, 1)
            .wait()
            .expect_err("desligado"),
        ProviderError::Disabled {
            provider: ProviderId::Lichess
        }
    );
    assert!(off.server.requests().is_empty());
}

#[test]
fn fair_play_lock_refuses_cloud_eval_even_when_no_evaluation_is_remembered() {
    let env = env_enabled();
    env.server
        .route("/api/cloud-eval", FakeResponse::status(404));
    env.lichess
        .cloud_eval(START, 1)
        .wait()
        .expect("lembra o 404");

    env.net.fair_play_guard().engage("Partida online");
    assert_eq!(
        env.lichess.cloud_eval(START, 1).wait().expect_err("trava"),
        LOCKED
    );
    env.net.fair_play_guard().release();
    assert!(env.lichess.cloud_eval(START, 1).wait().is_ok());
}

// ---- Finais exatos -----------------------------------------------------------------

#[test]
fn tablebase_returns_category_dtz_dtm_and_ordered_moves() {
    let env = env_enabled();
    env.server
        .route("/standard", fixture("lichess-tablebase-kpk.http"));

    let answer = env.lichess.tablebase(KPK).wait().expect("finais");

    let tb = &answer.value;
    assert_eq!(tb.category, Category::Win);
    assert_eq!(tb.dtz, Some(1));
    assert_eq!(tb.dtm, Some(17));
    assert!(!tb.checkmate && !tb.stalemate && !tb.insufficient_material);
    assert_eq!(tb.moves[0].uci, "h7h8q");
    assert_eq!(tb.moves[0].san, "h8=Q+");
    assert!(tb.moves[0].zeroing);
    assert_eq!(tb.moves[0].category_after, Category::Loss);
    assert_eq!(tb.moves[0].category_for_mover(), Category::Win);
    assert!(tb.moves.len() > 3);
    assert_eq!(answer.provenance.provider, ProviderId::Lichess);
    assert!(answer.provenance.license.contains("Syzygy"));
    // O FEN vai completo: o relógio de 50 Lances importa.
    let sent = &env.server.requests()[0];
    assert_eq!(sent.query_value("fen").as_deref(), Some(KPK));
}

#[test]
fn tablebase_with_eight_pieces_creates_no_request() {
    let env = env_enabled();
    env.server
        .route("/standard", fixture("lichess-tablebase-kpk.http"));
    let eight = "4k3/ppp5/8/8/8/8/PPP5/4K3 w - - 0 1";
    let seven = "4k3/pp6/8/8/8/8/PPP5/4K3 w - - 0 1";

    assert_eq!(
        env.lichess.tablebase(eight).wait().expect_err("8 peças"),
        ProviderError::TooManyPieces { pieces: 8, max: 7 }
    );
    assert!(env.server.requests().is_empty(), "8 peças não geram pedido");

    // Sete peças ainda é válido.
    assert!(env.lichess.tablebase(seven).wait().is_ok());
    assert_eq!(env.server.requests().len(), 1);
    assert_eq!(
        env.server.requests()[0].query_value("fen").as_deref(),
        Some(seven)
    );
}

#[test]
fn tablebase_failures_and_gate() {
    let env = env_enabled();
    env.server.route("/standard", FakeResponse::status(404));
    assert_eq!(
        env.lichess.tablebase(KPK).wait().expect_err("404"),
        ProviderError::NoData
    );
    env.server.route("/standard", FakeResponse::status(429));
    assert!(matches!(
        env.lichess.tablebase(KPK).wait(),
        Err(ProviderError::RateLimited { .. })
    ));

    let env = env_enabled();
    env.server.route("/standard", FakeResponse::json("{}"));
    assert!(matches!(
        env.lichess.tablebase(KPK).wait(),
        Err(ProviderError::Malformed { .. })
    ));

    let off = common::env();
    assert!(matches!(
        off.lichess.tablebase(KPK).wait(),
        Err(ProviderError::Disabled { .. })
    ));
    assert!(off.server.requests().is_empty());

    let locked = env_enabled();
    locked.net.fair_play_guard().engage("Partida online");
    assert_eq!(
        locked.lichess.tablebase(KPK).wait().expect_err("trava"),
        LOCKED
    );
    assert!(locked.server.requests().is_empty());
}

// ---- Perfil público ----------------------------------------------------------------

#[test]
fn profile_validates_a_linked_name_and_is_not_blocked_by_fair_play() {
    let env = env_enabled();
    env.server
        .route("/api/user/lichess", fixture("lichess-user-profile.http"));
    env.net.fair_play_guard().engage("Partida online");

    let answer = env.lichess.profile("lichess").wait().expect("perfil");

    let profile = &answer.value;
    assert_eq!(profile.id, "lichess");
    assert_eq!(profile.username, "Lichess");
    assert!(!profile.closed);
    assert_eq!(profile.created_at_ms, Some(1_290_460_176_000));
    let blitz = profile
        .ratings
        .iter()
        .find(|r| r.perf == "blitz")
        .expect("blitz");
    assert_eq!(blitz.rating, 1500);
    assert!(blitz.provisional);
    assert_eq!(answer.provenance.provider, ProviderId::Lichess);
}

#[test]
fn profile_errors_are_typed() {
    let env = env_enabled();
    env.server
        .route("/api/user/fantasma", FakeResponse::status(404));
    let error = env
        .lichess
        .profile("fantasma")
        .wait()
        .expect_err("inexistente");
    assert_eq!(
        error,
        ProviderError::UnknownName {
            provider: ProviderId::Lichess,
            name: "fantasma".to_owned()
        }
    );
    assert!(error.to_string().contains("fantasma"));

    env.server.route(
        "/api/user/fechada",
        FakeResponse::json(r#"{"id":"fechada","username":"Fechada","disabled":true}"#),
    );
    assert!(
        env.lichess
            .profile("fechada")
            .wait()
            .expect("fechada")
            .value
            .closed
    );

    env.server
        .route("/api/user/torta", FakeResponse::json("não é json"));
    assert!(matches!(
        env.lichess.profile("torta").wait(),
        Err(ProviderError::Malformed { .. })
    ));
    assert!(matches!(
        env.lichess.profile("a/b?c").wait(),
        Err(ProviderError::InvalidInput { .. })
    ));

    // 429 deixa o Provedor "limitado até": por último, ou barraria os pedidos acima.
    env.server
        .route("/api/user/lenta", FakeResponse::status(429));
    assert!(matches!(
        env.lichess.profile("lenta").wait(),
        Err(ProviderError::RateLimited { .. })
    ));

    let off = common::env();
    assert!(matches!(
        off.lichess.profile("lichess").wait(),
        Err(ProviderError::Disabled { .. })
    ));
    assert!(off.server.requests().is_empty());
}

// ---- FIDE --------------------------------------------------------------------------

#[test]
fn fide_returns_standard_rapid_and_blitz_separately_with_origin() {
    let env = env_enabled();
    env.server.route(
        "/api/fide/player/1503014",
        fixture("lichess-fide-player.http"),
    );
    env.net.fair_play_guard().engage("Partida online");

    let answer = env.lichess.fide_player(1_503_014).wait().expect("FIDE");

    let fide = &answer.value;
    assert_eq!(fide.id, 1_503_014);
    assert_eq!(fide.name, "Carlsen, Magnus");
    assert_eq!(fide.federation.as_deref(), Some("NOR"));
    assert_eq!(fide.title.as_deref(), Some("GM"));
    assert_eq!(
        (fide.standard, fide.rapid, fide.blitz),
        (Some(2823), Some(2801), Some(2860))
    );
    assert!(!fide.inactive);
    assert_eq!(fide.birth_year, Some(1990));
    assert!(answer.provenance.license.contains("FIDE"));
    assert_eq!(answer.provenance.queried_at, T0);
}

#[test]
fn refreshing_a_fide_rating_asks_the_provider_again_and_moves_the_query_date() {
    let env = env_enabled();
    env.server.route(
        "/api/fide/player/1503014",
        fixture("lichess-fide-player.http"),
    );
    let first = env.lichess.fide_player(1_503_014).wait().expect("FIDE");
    assert_eq!(first.provenance.queried_at, T0);
    env.clock.advance(Duration::from_secs(3600));

    // Sem "Atualizar", o cache ainda válido responde e a data da consulta não muda.
    let cached = env.lichess.fide_player(1_503_014).wait().expect("FIDE");
    assert_eq!(cached.provenance.queried_at, T0);
    assert_eq!(env.server.requests().len(), 1);

    let fresh = env
        .lichess
        .fide_player_refreshed(1_503_014)
        .wait()
        .expect("FIDE");
    assert_eq!(env.server.requests().len(), 2, "foi ao Provedor de novo");
    assert_eq!(
        fresh.provenance.queried_at,
        T0.plus(Duration::from_secs(3600))
    );
    assert_eq!(fresh.value, first.value);
}

#[test]
fn fide_search_lists_players_and_failures_are_typed() {
    let env = env_enabled();
    env.server
        .route("/api/fide/player", fixture("lichess-fide-search.http"));

    let answer = env.lichess.fide_search("Carlsen").wait().expect("busca");

    assert!(answer.value.len() >= 2);
    assert_eq!(answer.value[0].name, "Carlsen, Magnus");
    assert!(answer.value.iter().any(|p| p.inactive));
    assert_eq!(
        env.server.requests()[0].query_value("q").as_deref(),
        Some("Carlsen")
    );

    assert_eq!(
        env.lichess
            .fide_search("x")
            .wait()
            .map(|_| ())
            .expect_err("curta")
            .to_string(),
        "Consulta inválida: a busca por nome vai de 2 a 60 caracteres."
    );
    env.server
        .route("/api/fide/player/1", FakeResponse::status(404));
    assert_eq!(
        env.lichess.fide_player(1).wait().expect_err("inexistente"),
        ProviderError::NoData
    );
    env.server
        .route("/api/fide/player/3", FakeResponse::json("{\"x\":1}"));
    assert!(matches!(
        env.lichess.fide_player(3).wait(),
        Err(ProviderError::Malformed { .. })
    ));
    // 429 deixa o Provedor "limitado até": por último.
    env.server
        .route("/api/fide/player/2", FakeResponse::status(429));
    assert!(matches!(
        env.lichess.fide_player(2).wait(),
        Err(ProviderError::RateLimited { .. })
    ));
    let off = common::env();
    assert!(matches!(
        off.lichess.fide_player(1_503_014).wait(),
        Err(ProviderError::Disabled { .. })
    ));
    assert!(off.server.requests().is_empty());
}

// ---- Cancelamento ------------------------------------------------------------------

#[test]
fn try_take_never_blocks_and_delivers_once() {
    let env = env_enabled();
    env.server
        .route("/api/user/lichess", fixture("lichess-user-profile.http"));
    let mut query = env.lichess.profile("lichess");

    let mut result = None;
    assert!(eventually(Duration::from_secs(5), || {
        result = query.try_take();
        result.is_some()
    }));
    assert!(result.expect("resultado").is_ok());
    assert!(query.is_done());
    assert!(query.try_take().is_none(), "entregue uma vez");
}
