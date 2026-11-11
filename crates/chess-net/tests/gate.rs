//! O ponto único (gate): endereço no catálogo, HTTPS, Trava de fair play, autorização do
//! Cofre presa ao host, User-Agent genérico e segredo fora de `Debug`/log.

mod common;

use chess_net::testing::{FakeResponse, FakeServer};
use chess_net::{
    Authorization, AuthorizationSource, Capability, ConnectionId, FairPlayGuard, LocalProfileId,
    Net, NetError, ProviderId, Request, Secret, USER_AGENT,
};
use common::*;
use std::sync::Arc;

const TOKEN: &str = "lip_SEGREDO_NAO_PODE_VAZAR";

/// Cofre de teste: a Conexão 7 do Lichess tem um token para `lichess.org`.
#[derive(Debug)]
struct TestVault;

impl AuthorizationSource for TestVault {
    fn authorization(
        &self,
        _profile: LocalProfileId,
        provider: ProviderId,
        connection: ConnectionId,
    ) -> Option<Authorization> {
        (provider == ProviderId::Lichess && connection == ConnectionId(7))
            .then(|| Authorization::bearer("lichess.org", Secret::new(TOKEN)))
    }
}

fn computing_requests() -> Vec<Request> {
    vec![
        masters(),
        Request::get(ProviderId::Lichess, "https://lichess.org/api/cloud-eval")
            .query("fen", "8/8/8/8/8/8/4K3/6kQ w - - 0 1"),
        Request::get(
            ProviderId::Lichess,
            "https://tablebase.lichess.org/standard",
        )
        .query("fen", "8/8/8/8/8/8/4K3/6kQ w - - 0 1"),
        Request::post(ProviderId::ChessApi, "https://chess-api.com/v1").body(
            "application/json",
            br#"{"fen":"8/8/8/8/8/8/4K3/6kQ w - - 0 1"}"#.to_vec(),
        ),
    ]
}

#[test]
fn each_request_is_classified_by_the_catalog_as_computing_or_not() {
    for request in computing_requests() {
        let capability = request.capability().expect("no catálogo");
        assert!(capability.computes(), "{request:?}");
    }
    for (url, expected) in [
        (
            "https://lichess.org/api/games/user/someone",
            Capability::Query,
        ),
        (
            "https://lichess.org/api/board/game/x/move/e2e4",
            Capability::Play,
        ),
        (
            "https://lichess.org/api/bot/game/x/move/e2e4",
            Capability::OnlineEngine,
        ),
        (
            "https://database.lichess.org/lichess_db_puzzle.csv.zst",
            Capability::Download,
        ),
    ] {
        let capability = Request::get(ProviderId::Lichess, url).capability();
        assert_eq!(capability, Some(expected), "{url}");
        assert!(!expected.computes());
    }
    assert_eq!(
        Request::get(ProviderId::Lichess, "https://lichess.org/account").capability(),
        None
    );
}

#[test]
fn an_address_outside_the_catalog_is_refused_before_the_network() {
    let clock = clock();
    let server = server(&clock);
    let net = net_enabled(
        &server,
        &clock,
        &[ProviderId::Lichess, ProviderId::ChessCom],
    );

    for (request, why) in [
        (
            Request::get(ProviderId::Lichess, "https://api.chess.com/pub/player/x"),
            "host de outro Provedor",
        ),
        (
            Request::get(ProviderId::Lichess, "https://lichess.org/account/security"),
            "caminho fora do catálogo",
        ),
        (
            Request::get(ProviderId::Lichess, "http://explorer.lichess.org/masters"),
            "sem HTTPS",
        ),
        (
            Request::get(
                ProviderId::Lichess,
                "https://lichess.org@evil.example/api/user/x",
            ),
            "userinfo no host",
        ),
        (
            Request::get(ProviderId::ChessCom, "https://www.chess.com/play/online"),
            "site, não API",
        ),
    ] {
        let err = net.fetch(request).wait().unwrap_err();
        assert!(
            matches!(
                err,
                NetError::EndpointNotAllowed { .. } | NetError::InvalidRequest { .. }
            ),
            "{why}: {err:?}"
        );
    }
    assert_eq!(server.connections(), 0);
}

#[test]
fn with_the_fair_play_guard_engaged_nothing_that_computes_leaves() {
    let clock = clock();
    let server = server(&clock);
    let guard = FairPlayGuard::new();
    let net = Net::open(config(&server, &clock).fair_play_guard(guard.clone())).expect("abre");
    for p in [ProviderId::Lichess, ProviderId::ChessApi] {
        net.set_enabled(p, true).expect("liga");
    }
    guard.engage("Partida online contra uma pessoa em andamento");

    for request in computing_requests() {
        let err = net.fetch(request).wait().unwrap_err();
        assert!(matches!(err, NetError::FairPlayLocked { .. }), "{err:?}");
        assert!(err.to_string().contains("Trava de fair play"), "{err}");
    }
    let stream = net.stream(masters());
    assert!(matches!(
        stream.next_timeout(std::time::Duration::from_secs(5)),
        Some(chess_net::StreamEvent::Failed(
            NetError::FairPlayLocked { .. }
        ))
    ));
    assert_eq!(server.connections(), 0, "a trava age antes da rede");
}

#[test]
fn the_guard_lets_through_what_does_not_compute_and_releases_at_the_end() {
    let clock = clock();
    let server = server(&clock);
    server.route("/api/games/user/someone", FakeResponse::json("{}"));
    server.route("/masters", FakeResponse::json("{}"));
    let guard = FairPlayGuard::new();
    let net = Net::open(config(&server, &clock).fair_play_guard(guard.clone())).expect("abre");
    net.set_enabled(ProviderId::Lichess, true).expect("liga");

    guard.engage("Partida online");
    let import = Request::get(
        ProviderId::Lichess,
        "https://lichess.org/api/games/user/someone",
    );
    net.fetch(import).wait().expect("importar não calcula");
    assert!(net.fetch(masters()).wait().is_err());

    guard.release();
    net.fetch(masters())
        .wait()
        .expect("liberado no fim da Partida");
    assert!(!net.fair_play_guard().is_engaged());
}

#[test]
fn the_guard_is_consulted_again_right_before_the_network() {
    let clock = clock();
    let server = server(&clock);
    server.route("/masters", FakeResponse::stall());
    server.route("/lichess", FakeResponse::json("{}"));
    let guard = FairPlayGuard::new();
    let net = Net::open(config(&server, &clock).fair_play_guard(guard.clone())).expect("abre");
    net.set_enabled(ProviderId::Lichess, true).expect("liga");

    let first = net.fetch(masters().timeout(std::time::Duration::from_millis(300)));
    assert!(server.wait_for_requests(1, std::time::Duration::from_secs(5)));
    let queued = net.fetch(Request::get(
        ProviderId::Lichess,
        "https://explorer.lichess.org/lichess",
    ));
    guard.engage("a Partida online começou com o pedido na fila");

    let _ = first.wait();
    assert!(matches!(
        queued.wait(),
        Err(NetError::FairPlayLocked { .. })
    ));
    assert_eq!(server.requests().len(), 1);
}

#[test]
fn a_capability_that_needs_a_connection_is_refused_without_one() {
    let clock = clock();
    let server = server(&clock);
    let net = net_enabled(&server, &clock, &[ProviderId::Lichess]);

    let err = net
        .fetch(Request::get(
            ProviderId::Lichess,
            "https://lichess.org/api/account",
        ))
        .wait()
        .unwrap_err();
    assert_eq!(
        err,
        NetError::NotAuthorized {
            provider: ProviderId::Lichess,
            capability: Capability::Play
        }
    );

    let err = net
        .fetch(
            Request::get(ProviderId::Lichess, "https://lichess.org/api/account")
                .connection(ConnectionId(7)),
        )
        .wait()
        .unwrap_err();
    assert!(
        matches!(err, NetError::NotAuthorized { .. }),
        "sem Cofre: {err:?}"
    );
    assert_eq!(server.connections(), 0);
}

#[test]
fn the_authorization_goes_only_to_its_own_host() {
    let clock = clock();
    let lichess = server(&clock);
    let explorer = FakeServer::with_clock(clock.clone());
    lichess.route("/api/account", FakeResponse::json("{\"id\":\"eu\"}"));
    explorer.route("/masters", FakeResponse::json("{}"));
    let net = Net::open(
        chess_net::NetConfig::in_memory()
            .clock(clock.clone())
            .route("lichess.org", &lichess.base_url())
            .route("explorer.lichess.org", &explorer.base_url())
            .vault(Arc::new(TestVault)),
    )
    .expect("abre");
    net.set_enabled(ProviderId::Lichess, true).expect("liga");

    net.fetch(
        Request::get(ProviderId::Lichess, "https://lichess.org/api/account")
            .connection(ConnectionId(7)),
    )
    .wait()
    .expect("autorizado");
    net.fetch(masters().connection(ConnectionId(7)))
        .wait()
        .expect("Explorador sem o token");

    assert_eq!(
        lichess.requests()[0].header("authorization"),
        Some(format!("Bearer {TOKEN}").as_str())
    );
    assert_eq!(explorer.requests()[0].header("authorization"), None);
}

#[test]
fn a_redirect_to_another_host_is_not_followed() {
    let clock = clock();
    let server = server(&clock);
    server.route(
        "/api/user/someone",
        FakeResponse::status(302).header("Location", "https://evil.example/steal"),
    );
    let net = net_enabled(&server, &clock, &[ProviderId::Lichess]);

    let err = net
        .fetch(Request::get(
            ProviderId::Lichess,
            "https://lichess.org/api/user/someone",
        ))
        .wait()
        .unwrap_err();

    assert!(matches!(err, NetError::RedirectRefused { .. }), "{err:?}");
    assert_eq!(server.requests().len(), 1);
}

#[test]
fn the_user_agent_is_the_generic_one_of_the_app() {
    let clock = clock();
    let server = server(&clock);
    server.route("/masters", FakeResponse::json("{}"));
    let net = net_enabled(&server, &clock, &[ProviderId::Lichess]);

    net.fetch(masters()).wait().expect("rede");

    let sent = &server.requests()[0];
    assert_eq!(sent.header("user-agent"), Some(USER_AGENT));
    assert!(USER_AGENT.starts_with("ChessStudio/"));
    assert_eq!(sent.header("cookie"), None);
}

#[test]
fn authorization_and_cookies_cannot_be_set_by_hand() {
    let clock = clock();
    let server = server(&clock);
    let net = net_enabled(&server, &clock, &[ProviderId::Lichess]);

    for name in ["Authorization", "cookie", "Proxy-Authorization"] {
        let err = net.fetch(masters().header(name, TOKEN)).wait().unwrap_err();
        assert!(
            matches!(err, NetError::InvalidRequest { .. }),
            "{name}: {err:?}"
        );
        assert!(!err.to_string().contains(TOKEN));
    }
    assert_eq!(server.connections(), 0);
}

#[test]
fn no_secret_shows_up_in_debug_errors_or_the_operation_log() {
    let clock = clock();
    let server = server(&clock);
    server.route("/api/account", FakeResponse::status(401));
    let net = Net::open(config(&server, &clock).vault(Arc::new(TestVault))).expect("abre");
    net.set_enabled(ProviderId::Lichess, true).expect("liga");
    let request = Request::get(ProviderId::Lichess, "https://lichess.org/api/account")
        .connection(ConnectionId(7));

    let err = net.fetch(request.clone()).wait().unwrap_err();
    assert_eq!(err.http_status(), Some(401));

    let secret = Secret::new(TOKEN);
    let auth = Authorization::bearer("lichess.org", Secret::new(TOKEN));
    let texts = [
        format!("{secret:?}"),
        format!("{auth:?}"),
        format!("{request:?}"),
        format!("{err:?}"),
        err.to_string(),
        format!("{net:?}"),
        format!("{:?}", net.status()),
        format!("{:?}", net.recent_operations()),
        format!(
            "{:?}",
            TestVault.authorization(LocalProfileId(1), ProviderId::Lichess, ConnectionId(7))
        ),
    ];
    for text in texts {
        assert!(!text.contains(TOKEN), "segredo vazou: {text}");
        assert!(!text.contains("SEGREDO"), "segredo vazou: {text}");
    }
    let ops = net.recent_operations();
    assert_eq!(ops.len(), 1);
    assert_eq!(ops[0].provider, ProviderId::Lichess);
    assert_eq!(ops[0].path, "/api/account");
    assert_eq!(ops[0].http_status, Some(401));
}

#[test]
fn a_route_must_be_https_or_loopback() {
    let err =
        Net::open(chess_net::NetConfig::in_memory().route("lichess.org", "http://evil.example"))
            .unwrap_err();
    assert!(matches!(err, NetError::InvalidRequest { .. }), "{err:?}");
    for ok in [
        "https://espelho.example",
        "http://127.0.0.1:8080",
        "http://localhost:9",
    ] {
        Net::open(chess_net::NetConfig::in_memory().route("lichess.org", ok)).expect(ok);
    }
}
