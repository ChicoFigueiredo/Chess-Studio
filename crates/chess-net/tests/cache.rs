//! Cache com Origem: TTL, revalidação por `ETag`, leitura offline, limpeza por Provedor e
//! chave de Posição igual à do índice local.

mod common;

use chess_net::testing::FakeResponse;
use chess_net::{
    Freshness, ImportMode, LocalProfileId, NetError, ProviderId, Request, catalog::provider,
};
use common::*;
use std::time::Duration;

const HOUR: Duration = Duration::from_secs(3600);

#[test]
fn a_second_equal_request_within_the_ttl_does_not_touch_the_network() {
    let clock = clock();
    let server = server(&clock);
    server.route("/masters", FakeResponse::json("{\"moves\":[]}"));
    let net = net_enabled(&server, &clock, &[ProviderId::Lichess]);
    let request = || {
        explorer("/masters")
            .query("play", "e2e4,e7e5")
            .query("since", "1952")
            .cache_for(HOUR)
    };

    let first = net.fetch(request()).wait().expect("rede");
    assert_eq!(first.freshness, Freshness::Network);
    assert_eq!(first.age, Duration::ZERO);

    clock.advance(Duration::from_secs(600));
    // Mesmo pedido com os parâmetros em outra ordem: a chave é normalizada.
    let reordered = explorer("/masters")
        .query("since", "1952")
        .query("play", "e2e4,e7e5")
        .cache_for(HOUR);
    let second = net.fetch(reordered).wait().expect("cache");

    assert_eq!(server.requests().len(), 1);
    assert_eq!(second.freshness, Freshness::Cache);
    assert_eq!(second.body, b"{\"moves\":[]}");
    assert_eq!(second.age, Duration::from_secs(600));
    let origin = &second.provenance;
    assert_eq!(origin.provider, ProviderId::Lichess);
    assert_eq!(origin.queried_at, T0);
    assert_eq!(origin.mode, ImportMode::Api);
    assert!(!origin.license.is_empty());
    assert!(
        origin
            .source
            .starts_with("https://explorer.lichess.org/masters?"),
        "{}",
        origin.source
    );
}

#[test]
fn an_etag_answered_with_304_renews_the_validity() {
    let clock = clock();
    let server = server(&clock);
    server.route_seq(
        "/masters",
        vec![
            FakeResponse::json("{\"v\":1}").header("ETag", "\"v1\""),
            FakeResponse::status(304).header("ETag", "\"v1\""),
        ],
    );
    let net = net_enabled(&server, &clock, &[ProviderId::Lichess]);
    let request = || masters().cache_for(Duration::from_secs(600));

    net.fetch(request()).wait().expect("rede");
    clock.advance(Duration::from_secs(660));
    let revalidated = net.fetch(request()).wait().expect("304");

    assert_eq!(revalidated.freshness, Freshness::Revalidated);
    assert_eq!(revalidated.body, b"{\"v\":1}");
    assert_eq!(
        revalidated.provenance.queried_at,
        T0.plus(Duration::from_secs(660))
    );
    let sent = &server.requests()[1];
    assert_eq!(sent.header("if-none-match"), Some("\"v1\""));

    clock.advance(Duration::from_secs(300));
    let cached = net.fetch(request()).wait().expect("cache renovado");
    assert_eq!(cached.freshness, Freshness::Cache);
    assert_eq!(server.requests().len(), 2, "validade renovada pelo 304");
}

#[test]
fn offline_the_cache_comes_back_with_its_provenance_and_age() {
    let clock = clock();
    let server = server(&clock);
    server.route("/masters", FakeResponse::json("{\"white\":10}"));
    let net = net_enabled(&server, &clock, &[ProviderId::Lichess]);
    net.fetch(masters().cache_for(Duration::from_secs(60)))
        .wait()
        .expect("rede");

    drop(server); // o computador fica sem rede: conexão recusada
    clock.advance(2 * HOUR);
    let stale = net
        .fetch(masters().cache_for(Duration::from_secs(60)))
        .wait()
        .expect("cache vencido serve offline");

    assert_eq!(stale.body, b"{\"white\":10}");
    assert_eq!(stale.age, 2 * HOUR);
    assert_eq!(stale.provenance.queried_at, T0);
    match &stale.freshness {
        Freshness::Stale { cause } => {
            assert!(matches!(cause, NetError::Unreachable { .. }), "{cause:?}")
        }
        other => panic!("esperava Stale, veio {other:?}"),
    }
}

#[test]
fn a_stale_fallback_can_be_refused() {
    let clock = clock();
    let server = server(&clock);
    server.route("/masters", FakeResponse::json("{}"));
    let net = net_enabled(&server, &clock, &[ProviderId::Lichess]);
    net.fetch(masters().cache_for(Duration::from_secs(60)))
        .wait()
        .expect("rede");
    drop(server);
    clock.advance(HOUR);

    let err = net
        .fetch(masters().cache_for(Duration::from_secs(60)).no_stale())
        .wait()
        .unwrap_err();
    assert!(matches!(err, NetError::Unreachable { .. }), "{err:?}");
}

#[test]
fn the_cache_reads_offline_even_with_the_provider_turned_off() {
    let clock = clock();
    let server = server(&clock);
    server.route("/masters", FakeResponse::json("{}"));
    let net = net_enabled(&server, &clock, &[ProviderId::Lichess]);
    net.fetch(masters().cache_for(HOUR)).wait().expect("rede");
    net.set_enabled(ProviderId::Lichess, false)
        .expect("desliga");
    clock.advance(Duration::from_secs(90));

    assert!(matches!(
        net.fetch(masters().cache_for(HOUR)).wait(),
        Err(NetError::ProviderDisabled { .. })
    ));
    let local = net
        .read_cache(&masters().cache_for(HOUR))
        .expect("lê o banco")
        .expect("está em cache");
    assert_eq!(local.freshness, Freshness::Cache);
    assert_eq!(local.age, Duration::from_secs(90));
    assert_eq!(server.requests().len(), 1);
}

#[test]
fn clearing_the_cache_is_per_provider() {
    let clock = clock();
    let server = server(&clock);
    server.route("/masters", FakeResponse::json("{}"));
    server.route("/pub/player/hikaru", FakeResponse::json("{}"));
    let net = net_enabled(
        &server,
        &clock,
        &[ProviderId::Lichess, ProviderId::ChessCom],
    );
    let chesscom = || {
        Request::get(
            ProviderId::ChessCom,
            "https://api.chess.com/pub/player/hikaru",
        )
        .cache_for(HOUR)
    };
    net.fetch(masters().cache_for(HOUR))
        .wait()
        .expect("lichess");
    net.fetch(chesscom()).wait().expect("chess.com");

    assert_eq!(net.clear_cache(ProviderId::Lichess).expect("limpa"), 1);

    assert!(
        net.read_cache(&masters().cache_for(HOUR))
            .expect("lê")
            .is_none()
    );
    assert!(net.read_cache(&chesscom()).expect("lê").is_some());
}

#[test]
fn the_cache_belongs_to_the_local_profile() {
    let clock = clock();
    let server = server(&clock);
    server.route("/masters", FakeResponse::json("{}"));
    let net = net_enabled(&server, &clock, &[ProviderId::Lichess]);
    net.fetch(masters().cache_for(HOUR)).wait().expect("rede");

    net.set_active_profile(LocalProfileId(2));
    assert!(
        net.read_cache(&masters().cache_for(HOUR))
            .expect("lê")
            .is_none()
    );
}

#[test]
fn position_requests_use_the_same_key_as_the_local_index() {
    let clock = clock();
    let server = server(&clock);
    server.route("/masters", FakeResponse::json("{}"));
    let net = net_enabled(&server, &clock, &[ProviderId::Lichess]);
    // Mesma Posição: muda só o relógio de 50 Lances, o número do Lance e um en passant que
    // nenhum peão pode capturar.
    let a = "rnbqkbnr/pppppppp/8/8/4P3/8/PPPP1PPP/RNBQKBNR b KQkq e3 0 1";
    let b = "rnbqkbnr/pppppppp/8/8/4P3/8/PPPP1PPP/RNBQKBNR b KQkq - 3 7";
    let request = |fen: &str| explorer("/masters").position("fen", fen).cache_for(HOUR);

    net.fetch(request(a)).wait().expect("rede");
    let second = net.fetch(request(b)).wait().expect("cache");

    assert_eq!(second.freshness, Freshness::Cache);
    assert_eq!(server.requests().len(), 1);
    // A chave do índice local da biblioteca (chess-store), a mesma do Explorador local.
    let key = chess_store::position_key(&chess_core::CanonicalPosition::from_fen(a).expect("fen"));
    assert_eq!(
        server.requests()[0].query_value("fen").as_deref(),
        Some(format!("{key} 0 1").as_str())
    );
}

#[test]
fn an_invalid_fen_is_refused_before_the_network() {
    let clock = clock();
    let server = server(&clock);
    let net = net_enabled(&server, &clock, &[ProviderId::Lichess]);

    let err = net
        .fetch(explorer("/masters").position("fen", "isto não é FEN"))
        .wait()
        .unwrap_err();

    assert!(matches!(err, NetError::InvalidRequest { .. }), "{err:?}");
    assert_eq!(server.connections(), 0);
}

#[test]
fn provenance_license_comes_from_the_catalog_endpoint() {
    let clock = clock();
    let server = server(&clock);
    server.route("/standard", FakeResponse::json("{}"));
    let net = net_enabled(&server, &clock, &[ProviderId::Lichess]);

    let fetched = net
        .fetch(
            Request::get(
                ProviderId::Lichess,
                "https://tablebase.lichess.org/standard",
            )
            .query("fen", "8/8/8/8/8/8/4K3/6kQ w - - 0 1"),
        )
        .wait()
        .expect("finais");

    let endpoint = provider(ProviderId::Lichess)
        .endpoints
        .iter()
        .find(|e| e.host == "tablebase.lichess.org")
        .expect("endpoint");
    assert_eq!(fetched.provenance.license, endpoint.license);
    let json = serde_json::to_string(&fetched.provenance).expect("serializa");
    assert!(json.contains("\"provider\":\"lichess\""), "{json}");
}
