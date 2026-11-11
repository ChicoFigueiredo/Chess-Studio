//! Estado da rede para a barra de status: por Provedor (desligado, disponível, limitado até,
//! falha) e geral (offline, n Provedores ligados).

mod common;

use chess_net::testing::FakeResponse;
use chess_net::{NetError, ProviderId, ProviderState};
use common::*;

#[test]
fn the_network_starts_offline_with_every_provider_disabled() {
    let clock = clock();
    let server = server(&clock);
    let net = net(&server, &clock);

    let status = net.status();
    assert!(status.is_offline());
    assert_eq!(status.providers.len(), chess_net::catalog().len());
    assert!(
        status
            .providers
            .iter()
            .all(|(_, s)| *s == ProviderState::Disabled)
    );
    assert_eq!(status.active_requests, 0);
}

#[test]
fn enabled_providers_are_counted_and_available() {
    let clock = clock();
    let server = server(&clock);
    let net = net_enabled(
        &server,
        &clock,
        &[ProviderId::Lichess, ProviderId::ChessCom],
    );

    let status = net.status();
    assert!(!status.is_offline());
    assert_eq!(status.enabled_count(), 2);
    assert_eq!(status.state(ProviderId::Lichess), &ProviderState::Available);
    assert_eq!(status.state(ProviderId::Fide), &ProviderState::Disabled);
}

#[test]
fn a_failure_shows_until_the_next_success() {
    let clock = clock();
    let server = server(&clock);
    server.route("/pub/player/x", FakeResponse::json("{}"));
    let net = net_enabled(
        &server,
        &clock,
        &[ProviderId::Lichess, ProviderId::ChessCom],
    );
    let unreachable = chess_net::Net::open(
        chess_net::NetConfig::in_memory()
            .clock(clock.clone())
            .route("explorer.lichess.org", "http://127.0.0.1:9"),
    )
    .expect("abre");
    unreachable
        .set_enabled(ProviderId::Lichess, true)
        .expect("liga");

    let err = unreachable.fetch(masters()).wait().unwrap_err();
    assert!(matches!(err, NetError::Unreachable { .. }), "{err:?}");
    match unreachable.status().state(ProviderId::Lichess) {
        ProviderState::Failing { since, error } => {
            assert_eq!(*since, T0);
            assert_eq!(error, &err);
        }
        other => panic!("esperava falha, veio {other:?}"),
    }
    assert_eq!(unreachable.status().failing_count(), 1);

    let ok = chess_net::Request::get(ProviderId::ChessCom, "https://api.chess.com/pub/player/x");
    net.fetch(ok).wait().expect("Chess.com responde");
    assert_eq!(
        net.status().state(ProviderId::ChessCom),
        &ProviderState::Available
    );
}

#[test]
fn a_limited_provider_becomes_available_when_the_deadline_passes() {
    let clock = clock();
    let server = server(&clock);
    server.route(
        "/masters",
        FakeResponse::status(429).header("Retry-After", "5"),
    );
    let net = net_enabled(&server, &clock, &[ProviderId::Lichess]);
    let _ = net.fetch(masters()).wait();

    assert_eq!(net.status().limited_count(), 1);
    clock.advance(secs(6));
    assert_eq!(
        net.status().state(ProviderId::Lichess),
        &ProviderState::Available
    );
}
