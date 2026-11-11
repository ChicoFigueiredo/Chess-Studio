//! Limites por Provedor: 429 com `Retry-After`, token bucket e backoff com jitter.
//! O relógio é falso: esperar avança o tempo na hora, e o servidor carimba cada pedido.

mod common;

use chess_net::testing::FakeResponse;
use chess_net::{NetError, ProviderId, ProviderState, RateLimit, RetryPolicy, Timestamp};
use common::*;
use std::time::Duration;

fn arrivals(server: &chess_net::testing::FakeServer) -> Vec<Duration> {
    server
        .requests()
        .iter()
        .map(|r| r.at.expect("carimbo").since(T0))
        .collect()
}

#[test]
fn retry_after_marks_the_provider_limited_and_nothing_is_sent_before_the_deadline() {
    let clock = clock();
    let server = server(&clock);
    server.route_seq(
        "/masters",
        vec![
            FakeResponse::status(429).header("Retry-After", "30"),
            FakeResponse::json("{\"white\":1}"),
        ],
    );
    let net = net_enabled(&server, &clock, &[ProviderId::Lichess]);
    let until = T0.plus(secs(30));

    let err = net.fetch(masters()).wait().unwrap_err();
    assert_eq!(
        err,
        NetError::RateLimited {
            provider: ProviderId::Lichess,
            until
        }
    );
    assert_eq!(
        net.status().state(ProviderId::Lichess),
        &ProviderState::Limited { until }
    );

    clock.advance(secs(10));
    let again = net.fetch(masters()).wait().unwrap_err();
    assert!(matches!(again, NetError::RateLimited { .. }));
    assert_eq!(server.requests().len(), 1, "não martela durante o prazo");

    clock.advance(secs(21));
    let ok = net.fetch(masters()).wait().expect("depois do prazo");
    assert_eq!(ok.body, b"{\"white\":1}");
    assert_eq!(server.requests().len(), 2);
    assert_eq!(
        net.status().state(ProviderId::Lichess),
        &ProviderState::Available
    );
}

#[test]
fn a_patient_request_waits_the_announced_time_and_retries_once() {
    let clock = clock();
    let server = server(&clock);
    server.route_seq(
        "/masters",
        vec![
            FakeResponse::status(429).header("Retry-After", "30"),
            FakeResponse::json("{}"),
        ],
    );
    let net = net_enabled(&server, &clock, &[ProviderId::Lichess]);

    net.fetch(masters().patience(secs(60)))
        .wait()
        .expect("espera e repete");

    let at = arrivals(&server);
    assert_eq!(at.len(), 2);
    assert!(at[1] - at[0] >= secs(30), "repetiu antes do prazo: {at:?}");
}

#[test]
fn without_retry_after_the_pause_is_one_minute() {
    let clock = clock();
    let server = server(&clock);
    server.route("/masters", FakeResponse::status(429));
    let net = net_enabled(&server, &clock, &[ProviderId::Lichess]);

    let err = net.fetch(masters()).wait().unwrap_err();

    assert_eq!(
        err,
        NetError::RateLimited {
            provider: ProviderId::Lichess,
            until: T0.plus(secs(60))
        }
    );
}

#[test]
fn retry_after_as_an_http_date_is_understood() {
    let clock = clock();
    let server = server(&clock);
    server.route(
        "/masters",
        FakeResponse::status(429).header("Retry-After", "Wed, 11 Nov 2026 12:02:00 GMT"),
    );
    let net = net_enabled(&server, &clock, &[ProviderId::Lichess]);

    let err = net.fetch(masters()).wait().unwrap_err();

    assert_eq!(
        err,
        NetError::RateLimited {
            provider: ProviderId::Lichess,
            until: Timestamp::from_millis(T0.as_millis() + 120_000)
        }
    );
}

#[test]
fn the_token_bucket_spaces_a_burst_to_the_configured_rate() {
    let clock = clock();
    let server = server(&clock);
    server.route("/masters", FakeResponse::json("{}"));
    let net = chess_net::Net::open(
        config(&server, &clock).rate_limit(ProviderId::Lichess, RateLimit::new(2, secs(10))),
    )
    .expect("abre");
    net.set_enabled(ProviderId::Lichess, true).expect("liga");

    let pending: Vec<_> = (0..5).map(|_| net.fetch(masters())).collect();
    for p in pending {
        p.wait().expect("todos passam, espaçados");
    }

    assert_eq!(
        arrivals(&server),
        vec![secs(0), secs(0), secs(10), secs(20), secs(30)]
    );
}

#[test]
fn server_errors_are_retried_with_exponential_backoff_and_jitter() {
    let clock = clock();
    let server = server(&clock);
    server.route_seq(
        "/masters",
        vec![
            FakeResponse::status(503),
            FakeResponse::status(503),
            FakeResponse::json("{}"),
        ],
    );
    let net = chess_net::Net::open(
        config(&server, &clock)
            .rate_limit(
                ProviderId::Lichess,
                RateLimit::new(10, Duration::from_millis(1)),
            )
            .retry(RetryPolicy {
                max_retries: 2,
                base_delay: secs(1),
                max_delay: secs(30),
            }),
    )
    .expect("abre");
    net.set_enabled(ProviderId::Lichess, true).expect("liga");

    net.fetch(masters()).wait().expect("terceira tentativa");

    let at = arrivals(&server);
    assert_eq!(at.len(), 3);
    let (first, second) = (at[1] - at[0], at[2] - at[1]);
    assert!(
        (Duration::from_millis(500)..=secs(1)).contains(&first),
        "{first:?}"
    );
    assert!((secs(1)..=secs(2)).contains(&second), "{second:?}");
}

#[test]
fn retries_are_bounded_and_the_provider_shows_as_failing() {
    let clock = clock();
    let server = server(&clock);
    server.route("/masters", FakeResponse::status(503));
    let net = chess_net::Net::open(config(&server, &clock).retry(RetryPolicy {
        max_retries: 2,
        base_delay: secs(1),
        max_delay: secs(30),
    }))
    .expect("abre");
    net.set_enabled(ProviderId::Lichess, true).expect("liga");

    let err = net.fetch(masters()).wait().unwrap_err();

    assert_eq!(
        err,
        NetError::Http {
            provider: ProviderId::Lichess,
            status: 503
        }
    );
    assert_eq!(server.requests().len(), 3);
    assert!(matches!(
        net.status().state(ProviderId::Lichess),
        ProviderState::Failing { .. }
    ));
}

#[test]
fn client_errors_are_not_retried() {
    let clock = clock();
    let server = server(&clock);
    server.route("/api/cloud-eval", FakeResponse::status(404));
    let net = net_enabled(&server, &clock, &[ProviderId::Lichess]);

    let err = net
        .fetch(
            chess_net::Request::get(ProviderId::Lichess, "https://lichess.org/api/cloud-eval")
                .query("fen", "8/8/8/8/8/8/4K3/6kQ w - - 0 1"),
        )
        .wait()
        .unwrap_err();

    assert_eq!(err.http_status(), Some(404), "404 = sem avaliação em cache");
    assert_eq!(server.requests().len(), 1);
    assert_eq!(
        net.status().state(ProviderId::Lichess),
        &ProviderState::Available
    );
}
