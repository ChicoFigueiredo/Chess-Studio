//! Timeout e cancelamento: erro tipado na hora e nenhuma thread presa. Aqui o servidor falso
//! segura a resposta de verdade, então os prazos são em tempo real (curtos).

mod common;

use chess_net::testing::FakeResponse;
use chess_net::{NetError, ProviderId};
use common::*;
use std::time::{Duration, Instant};

#[test]
fn a_stalled_server_ends_in_a_typed_timeout() {
    let clock = clock();
    let server = server(&clock);
    server.route("/masters", FakeResponse::stall());
    let net = net_enabled(&server, &clock, &[ProviderId::Lichess]);

    let start = Instant::now();
    let err = net
        .fetch(masters().timeout(Duration::from_millis(300)))
        .wait()
        .unwrap_err();

    assert_eq!(
        err,
        NetError::Timeout {
            provider: ProviderId::Lichess
        }
    );
    assert!(start.elapsed() < Duration::from_secs(5));
    assert!(
        eventually(Duration::from_secs(30), || net.status().active_requests
            == 0),
        "a thread do Provedor ficou presa"
    );
}

#[test]
fn cancelling_answers_at_once_and_the_worker_is_released() {
    let clock = clock();
    let server = server(&clock);
    server.route("/masters", FakeResponse::stall());
    server.route("/lichess", FakeResponse::json("{}"));
    let net = net_enabled(&server, &clock, &[ProviderId::Lichess]);

    let pending = net.fetch(masters().timeout(Duration::from_millis(800)));
    assert!(server.wait_for_requests(1, Duration::from_secs(30)));
    let start = Instant::now();
    pending.cancel();
    assert_eq!(pending.wait(), Err(NetError::Cancelled));
    assert!(start.elapsed() < Duration::from_millis(600));

    assert!(
        eventually(Duration::from_secs(30), || net.status().active_requests
            == 0),
        "a thread do Provedor ficou presa"
    );
    let next = explorer("/lichess");
    net.fetch(next)
        .wait()
        .expect("o Provedor volta a responder");
}

#[test]
fn a_request_cancelled_while_queued_never_reaches_the_network() {
    let clock = clock();
    let server = server(&clock);
    server.route("/masters", FakeResponse::stall());
    server.route("/lichess", FakeResponse::json("{}"));
    let net = net_enabled(&server, &clock, &[ProviderId::Lichess]);

    let first = net.fetch(masters().timeout(Duration::from_millis(400)));
    assert!(server.wait_for_requests(1, Duration::from_secs(30)));
    let mut queued = net.fetch(explorer("/lichess"));
    queued.cancel();
    assert_eq!(queued.try_take(), Some(Err(NetError::Cancelled)));
    drop(queued);

    assert!(matches!(first.wait(), Err(NetError::Timeout { .. })));
    assert!(eventually(Duration::from_secs(30), || net
        .status()
        .active_requests
        == 0));
    std::thread::sleep(Duration::from_millis(100));
    assert_eq!(server.requests().len(), 1, "o pedido cancelado não saiu");
}

#[test]
fn dropping_the_pending_handle_cancels_the_request() {
    let clock = clock();
    let server = server(&clock);
    server.route("/masters", FakeResponse::stall());
    server.route("/lichess", FakeResponse::json("{}"));
    let net = net_enabled(&server, &clock, &[ProviderId::Lichess]);

    let first = net.fetch(masters().timeout(Duration::from_millis(400)));
    assert!(server.wait_for_requests(1, Duration::from_secs(30)));
    drop(net.fetch(explorer("/lichess")));

    let _ = first.wait();
    assert!(eventually(Duration::from_secs(30), || net
        .status()
        .active_requests
        == 0));
    assert_eq!(server.requests().len(), 1);
}

#[test]
fn polling_never_blocks() {
    let clock = clock();
    let server = server(&clock);
    server.route("/masters", FakeResponse::stall());
    let net = net_enabled(&server, &clock, &[ProviderId::Lichess]);

    let mut pending = net.fetch(masters().timeout(Duration::from_millis(500)));
    let start = Instant::now();
    for _ in 0..100 {
        assert_eq!(pending.try_take(), None);
    }
    assert!(
        start.elapsed() < Duration::from_millis(400),
        "poll bloqueou"
    );
    pending.cancel();
}
