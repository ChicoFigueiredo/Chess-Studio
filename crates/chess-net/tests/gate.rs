//! O ponto único (gate): endereço no catálogo, HTTPS, Trava de fair play, autorização do
//! Cofre presa ao Provedor e aos endereços autenticados do catálogo, User-Agent genérico e
//! segredo fora de `Debug`/log.

mod common;

use chess_net::testing::{FakeResponse, FakeServer};
use chess_net::{
    Authorization, AuthorizationSource, Capability, ConnectionId, FairPlayGuard, LocalProfileId,
    Net, NetError, ProviderId, Request, Secret, USER_AGENT,
};
use common::*;
use std::sync::Arc;

const TOKEN: &str = "lip_SEGREDO_NAO_PODE_VAZAR";

/// Cofre de teste: a Conexão 7 do Lichess tem um token.
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
            .then(|| Authorization::bearer(ProviderId::Lichess, &Secret::new(TOKEN)))
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

/// Ramo de segurança (NFR-10, #269): porta explícita e hosts fora de `[a-z0-9.-]` nunca chegam
/// à rede, mesmo no host certo do catálogo.
#[test]
fn an_explicit_port_or_a_malformed_host_is_refused_before_the_network() {
    let clock = clock();
    let server = server(&clock);
    let net = net_enabled(&server, &clock, &[ProviderId::Lichess]);

    for (url, why) in [
        ("https://lichess.org:8443/api/user/x", "porta explícita"),
        (
            "https://lichess.org:443/api/user/x",
            "porta, ainda que a padrão",
        ),
        (
            "https://lichess.org:abc/api/user/x",
            "porta que não é número",
        ),
        ("https://lichess_org/api/user/x", "sublinhado no host"),
        (
            "https://.lichess.org/api/user/x",
            "host que começa com ponto",
        ),
        (
            "https://lichess.org./api/user/x",
            "host que termina com ponto",
        ),
        ("https:///api/user/x", "host vazio"),
        ("https://lichess.org/api/user/x y", "espaço no caminho"),
        (
            "https://lichess.org/api/user/x\r\nHost: evil",
            "quebra de linha",
        ),
    ] {
        let err = net
            .fetch(Request::get(ProviderId::Lichess, url))
            .wait()
            .unwrap_err();
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
        stream.next_timeout(std::time::Duration::from_secs(30)),
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
    assert!(server.wait_for_requests(1, std::time::Duration::from_secs(30)));
    let queued = net.fetch(
        Request::get(ProviderId::Lichess, "https://explorer.lichess.org/lichess")
            .connection(EXPLORER_CONNECTION),
    );
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
fn the_authorization_goes_only_to_the_authenticated_endpoints_of_its_provider() {
    let clock = clock();
    let lichess = server(&clock);
    let explorer = FakeServer::with_clock(clock.clone());
    let others = FakeServer::with_clock(clock.clone());
    lichess.route("/api/account", FakeResponse::json("{\"id\":\"eu\"}"));
    lichess.route("/api/cloud-eval", FakeResponse::json("{}"));
    lichess.route("/api/games/user/someone", FakeResponse::json("{}"));
    explorer.route("/masters", FakeResponse::json("{}"));
    others.route("/standard", FakeResponse::json("{}"));
    others.route("/pub/player/someone", FakeResponse::json("{}"));
    let net = Net::open(
        chess_net::NetConfig::in_memory()
            .clock(clock.clone())
            .route("lichess.org", &lichess.base_url())
            .route("explorer.lichess.org", &explorer.base_url())
            .route("tablebase.lichess.org", &others.base_url())
            .route("api.chess.com", &others.base_url())
            .vault(Arc::new(TestVault)),
    )
    .expect("abre");
    for p in [ProviderId::Lichess, ProviderId::ChessCom] {
        net.set_enabled(p, true).expect("liga");
    }
    let c = ConnectionId(7);
    let kpk = "8/8/8/8/8/8/4K3/6kQ w - - 0 1";
    for request in [
        Request::get(ProviderId::Lichess, "https://lichess.org/api/account").connection(c),
        masters().connection(c),
        Request::get(ProviderId::Lichess, "https://lichess.org/api/cloud-eval")
            .query("fen", kpk)
            .connection(c),
        Request::get(
            ProviderId::Lichess,
            "https://lichess.org/api/games/user/someone",
        )
        .connection(c),
        Request::get(
            ProviderId::Lichess,
            "https://tablebase.lichess.org/standard",
        )
        .query("fen", kpk)
        .connection(c),
        Request::get(
            ProviderId::ChessCom,
            "https://api.chess.com/pub/player/someone",
        )
        .connection(c),
    ] {
        net.fetch(request).wait().expect("resposta");
    }

    let bearer = format!("Bearer {TOKEN}");
    let lichess_sent = lichess.requests();
    assert_eq!(lichess_sent.len(), 3);
    for r in &lichess_sent {
        let expected = (r.path == "/api/account").then_some(bearer.as_str());
        assert_eq!(r.header("authorization"), expected, "{}", r.path);
    }
    assert_eq!(
        explorer.requests()[0].header("authorization"),
        Some(bearer.as_str()),
        "o Explorador exige a Conexão desde 2026-11-11"
    );
    let other_sent = others.requests();
    assert_eq!(other_sent.len(), 2);
    for r in &other_sent {
        assert_eq!(r.header("authorization"), None, "{}", r.path);
    }
}

/// Cofre defeituoso: devolve o token do Lichess para qualquer Provedor e Conexão.
#[derive(Debug)]
struct LeakyVault;

impl AuthorizationSource for LeakyVault {
    fn authorization(
        &self,
        _profile: LocalProfileId,
        _provider: ProviderId,
        _connection: ConnectionId,
    ) -> Option<Authorization> {
        Some(Authorization::bearer(
            ProviderId::Lichess,
            &Secret::new(TOKEN),
        ))
    }
}

#[test]
fn a_token_of_one_provider_never_reaches_another_provider_even_from_a_faulty_vault() {
    let clock = clock();
    let server = server(&clock);
    server.route("/api/board/game/x/move/e2e4", FakeResponse::json("{}"));
    let net = Net::open(config(&server, &clock).vault(Arc::new(LeakyVault))).expect("abre");
    net.set_enabled(ProviderId::PlayStrategy, true)
        .expect("liga");

    let err = net
        .fetch(
            Request::post(
                ProviderId::PlayStrategy,
                "https://playstrategy.org/api/board/game/x/move/e2e4",
            )
            .connection(ConnectionId(7)),
        )
        .wait()
        .unwrap_err();

    assert!(matches!(err, NetError::NotAuthorized { .. }), "{err:?}");
    assert_eq!(
        server.connections(),
        0,
        "o token do Lichess não sai para outro Provedor"
    );
}

#[test]
fn naming_a_connection_without_a_secret_never_goes_out_unauthenticated() {
    let clock = clock();
    let server = server(&clock);
    let net = net_enabled(&server, &clock, &[ProviderId::Lichess]);

    for request in [
        Request::get(ProviderId::Lichess, "https://explorer.lichess.org/masters")
            .connection(ConnectionId(99)),
        Request::delete(ProviderId::Lichess, "https://lichess.org/api/token")
            .connection(ConnectionId(99)),
    ] {
        let err = net.fetch(request).wait().unwrap_err();
        assert!(matches!(err, NetError::NotAuthorized { .. }), "{err:?}");
    }
    // Sem Conexão, o Explorador exige uma (o Provedor responde 401 sem token).
    let err = net
        .fetch(Request::get(
            ProviderId::Lichess,
            "https://explorer.lichess.org/lichess",
        ))
        .wait()
        .unwrap_err();
    assert_eq!(
        err,
        NetError::NotAuthorized {
            provider: ProviderId::Lichess,
            capability: Capability::Explore
        }
    );
    // Outro Perfil local não usa a Conexão do Perfil 1.
    net.set_active_profile(LocalProfileId(2));
    net.set_enabled(ProviderId::Lichess, true).expect("liga");
    let err = net.fetch(masters()).wait().unwrap_err();
    assert!(matches!(err, NetError::NotAuthorized { .. }), "{err:?}");
    assert_eq!(server.connections(), 0);
}

/// Cofre que anota as recusas do Provedor.
#[derive(Debug, Default)]
struct RecordingVault {
    rejected: std::sync::Mutex<Vec<(ConnectionId, u16)>>,
}

impl AuthorizationSource for RecordingVault {
    fn authorization(
        &self,
        profile: LocalProfileId,
        provider: ProviderId,
        connection: ConnectionId,
    ) -> Option<Authorization> {
        TestVault.authorization(profile, provider, connection)
    }

    fn rejected(
        &self,
        _profile: LocalProfileId,
        _provider: ProviderId,
        connection: ConnectionId,
        status: u16,
    ) {
        self.rejected
            .lock()
            .expect("trava")
            .push((connection, status));
    }
}

#[test]
fn a_401_or_403_with_the_token_marks_the_connection_and_is_typed() {
    let clock = clock();
    let server = server(&clock);
    server.route("/api/account", FakeResponse::status(401));
    server.route("/api/board/game/x/move/e2e4", FakeResponse::status(403));
    server.route("/api/user/someone", FakeResponse::status(401));
    let vault = Arc::new(RecordingVault::default());
    let net = Net::open(config(&server, &clock).vault(vault.clone())).expect("abre");
    net.set_enabled(ProviderId::Lichess, true).expect("liga");

    let err = net
        .fetch(
            Request::get(ProviderId::Lichess, "https://lichess.org/api/account")
                .connection(ConnectionId(7)),
        )
        .wait()
        .unwrap_err();
    assert_eq!(
        err,
        NetError::ConnectionRejected {
            provider: ProviderId::Lichess,
            status: 401
        }
    );
    assert_eq!(err.http_status(), Some(401));
    assert!(err.to_string().contains("expirou ou foi revogada"), "{err}");

    let err = net
        .fetch(
            Request::post(
                ProviderId::Lichess,
                "https://lichess.org/api/board/game/x/move/e2e4",
            )
            .connection(ConnectionId(7)),
        )
        .wait()
        .unwrap_err();
    assert!(
        matches!(err, NetError::ConnectionRejected { status: 403, .. }),
        "{err:?}"
    );

    // Sem token, um 401 é só a resposta do Provedor: nenhuma Conexão é marcada.
    let err = net
        .fetch(Request::get(
            ProviderId::Lichess,
            "https://lichess.org/api/user/someone",
        ))
        .wait()
        .unwrap_err();
    assert_eq!(
        err,
        NetError::Http {
            provider: ProviderId::Lichess,
            status: 401
        }
    );
    assert_eq!(
        *vault.rejected.lock().expect("trava"),
        vec![(ConnectionId(7), 401), (ConnectionId(7), 403)]
    );
}

#[test]
fn the_scopes_of_the_token_come_back_only_on_authenticated_requests() {
    let clock = clock();
    let server = server(&clock);
    server.route(
        "/api/account",
        FakeResponse::json("{\"id\":\"eu\"}").header("x-oauth-scopes", "board:play"),
    );
    server.route(
        "/api/user/someone",
        FakeResponse::json("{}").header("x-oauth-scopes", "board:play"),
    );
    let net = Net::open(config(&server, &clock).vault(Arc::new(TestVault))).expect("abre");
    net.set_enabled(ProviderId::Lichess, true).expect("liga");

    let fetched = net
        .fetch(
            Request::get(ProviderId::Lichess, "https://lichess.org/api/account")
                .connection(ConnectionId(7)),
        )
        .wait()
        .expect("conta");
    assert_eq!(fetched.oauth_scopes.as_deref(), Some("board:play"));
    let public = net
        .fetch(Request::get(
            ProviderId::Lichess,
            "https://lichess.org/api/user/someone",
        ))
        .wait()
        .expect("perfil");
    assert_eq!(public.oauth_scopes, None);
}

#[test]
fn revocation_goes_out_as_delete_with_the_token() {
    let clock = clock();
    let server = server(&clock);
    server.route("/api/token", FakeResponse::status(204));
    let net = Net::open(config(&server, &clock).vault(Arc::new(TestVault))).expect("abre");
    net.set_enabled(ProviderId::Lichess, true).expect("liga");

    let fetched = net
        .fetch(
            Request::delete(ProviderId::Lichess, "https://lichess.org/api/token")
                .connection(ConnectionId(7)),
        )
        .wait()
        .expect("revogado");

    assert_eq!(fetched.status, 204);
    let sent = &server.requests()[0];
    assert_eq!(sent.method, "DELETE");
    assert_eq!(sent.path, "/api/token");
    assert_eq!(
        sent.header("authorization"),
        Some(format!("Bearer {TOKEN}").as_str())
    );
}

#[test]
fn the_debug_of_a_request_hides_the_body() {
    let request = Request::post(ProviderId::Lichess, "https://lichess.org/api/token").body(
        "application/x-www-form-urlencoded",
        b"code=CODIGO_SECRETO&code_verifier=VERIFICADOR_SECRETO".to_vec(),
    );
    let text = format!("{request:?}");
    assert!(!text.contains("SECRETO"), "{text}");
    assert!(text.contains("bytes"), "{text}");
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
    let auth = Authorization::bearer(ProviderId::Lichess, &Secret::new(TOKEN));
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
