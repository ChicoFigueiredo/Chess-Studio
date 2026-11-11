#![allow(clippy::unwrap_used)]
//! Transporte ICS (#237): a conexão TCP de texto do FICS passa pelo mesmo gate dos pedidos
//! HTTP. Contra o servidor ICS falso em loopback (CX-17: nada de conexão real); as linhas do
//! "servidor" são **sintéticas**, no formato documentado do FICS.

mod common;

use chess_net::testing::FakeIcsServer;
use chess_net::{
    FakeClock, IcsEvent, IcsLink, IcsRequest, MAX_ICS_COMMAND, Net, NetConfig, NetError,
    ProviderId, ProviderState, ics_endpoints,
};
use common::*;
use std::sync::Arc;
use std::time::Duration;

const WAIT: Duration = Duration::from_secs(10);

fn fics_net(server: &FakeIcsServer, clock: &Arc<FakeClock>) -> Net {
    Net::open(
        NetConfig::in_memory()
            .clock(clock.clone())
            .route("freechess.org", &server.route_url()),
    )
    .unwrap()
}

fn fics() -> IcsRequest {
    IcsRequest::to_catalog(ProviderId::Fics).unwrap()
}

/// Próximo evento que não é prompt (tempo real, curto).
fn next_event(link: &IcsLink) -> Option<IcsEvent> {
    let deadline = std::time::Instant::now() + WAIT;
    while std::time::Instant::now() < deadline {
        match link.next_timeout(Duration::from_millis(50)) {
            Some(IcsEvent::Prompt(_)) | None => {}
            Some(event) => return Some(event),
        }
    }
    None
}

/// Junta eventos até `cond` valer sobre eles.
fn collect_until(link: &IcsLink, mut cond: impl FnMut(&[IcsEvent]) -> bool) -> Vec<IcsEvent> {
    let mut seen = Vec::new();
    let deadline = std::time::Instant::now() + WAIT;
    while std::time::Instant::now() < deadline && !cond(&seen) {
        if let Some(event) = link.next_timeout(Duration::from_millis(20)) {
            seen.push(event);
        }
    }
    seen
}

#[test]
fn the_catalog_lists_only_the_plaintext_fics_port() {
    let endpoints = ics_endpoints();
    assert_eq!(endpoints.len(), 1);
    let fics = &endpoints[0];
    assert_eq!(fics.provider, ProviderId::Fics);
    assert_eq!((fics.host, fics.port), ("freechess.org", 5000));
    assert!(
        !fics.encrypted,
        "o FICS não tem canal cifrado comprovado (CX-18)"
    );
}

#[test]
fn a_disabled_provider_opens_no_ics_connection() {
    let clock = clock();
    let server = FakeIcsServer::start();
    let net = fics_net(&server, &clock);

    let link = net.connect_ics(fics());

    assert_eq!(
        link.next_timeout(WAIT),
        Some(IcsEvent::Failed(NetError::ProviderDisabled {
            provider: ProviderId::Fics
        }))
    );
    std::thread::sleep(Duration::from_millis(150));
    assert_eq!(server.connections(), 0, "nenhuma conexão pode ser aberta");
    assert_eq!(net.status().active_requests, 0);
}

#[test]
fn only_the_ics_addresses_of_the_catalog_are_reachable() {
    let clock = clock();
    let server = FakeIcsServer::start();
    let net = fics_net(&server, &clock);
    net.set_enabled(ProviderId::Fics, true).unwrap();
    net.set_enabled(ProviderId::Lichess, true).unwrap();

    for request in [
        IcsRequest::new(ProviderId::Fics, "evil.example.org", 5000),
        IcsRequest::new(ProviderId::Fics, "freechess.org", 23),
        IcsRequest::new(ProviderId::Fics, "freechess.org", 5001),
        // O endereço do FICS não é do Lichess.
        IcsRequest::new(ProviderId::Lichess, "freechess.org", 5000),
    ] {
        let link = net.connect_ics(request.clone());
        match link.next_timeout(WAIT) {
            Some(IcsEvent::Failed(NetError::EndpointNotAllowed { url, .. })) => {
                assert!(url.starts_with("ics://"), "{url}");
            }
            other => panic!("{request:?}: {other:?}"),
        }
    }
    std::thread::sleep(Duration::from_millis(150));
    assert_eq!(server.connections(), 0);
}

#[test]
fn a_route_that_is_not_loopback_is_refused_for_ics() {
    let clock = clock();
    let net = Net::open(
        NetConfig::in_memory()
            .clock(clock.clone())
            .route("freechess.org", "https://espelho.example.org"),
    )
    .unwrap();
    net.set_enabled(ProviderId::Fics, true).unwrap();

    let link = net.connect_ics(fics());

    assert!(matches!(
        next_event(&link),
        Some(IcsEvent::Failed(NetError::InvalidRequest { .. }))
    ));
}

#[test]
fn lines_flow_both_ways_and_the_link_counts_as_an_active_connection() {
    let clock = clock();
    let server = FakeIcsServer::start();
    let net = fics_net(&server, &clock);
    net.set_enabled(ProviderId::Fics, true).unwrap();

    let link = net.connect_ics(fics());
    assert_eq!(link.next_timeout(WAIT), Some(IcsEvent::Connected));
    assert!(server.wait_connected(WAIT));
    assert_eq!(net.status().active_requests, 1);
    assert_eq!(
        *net.status().state(ProviderId::Fics),
        ProviderState::Available
    );

    // Sintético: boas-vindas e o prompt de login, sem fim de linha.
    server.send("Welcome to the Free Internet Chess Server\n\nlogin: ");
    let seen = collect_until(&link, |s| {
        s.iter()
            .any(|e| matches!(e, IcsEvent::Prompt(p) if p == "login: "))
    });
    assert_eq!(
        seen[0],
        IcsEvent::Line("Welcome to the Free Internet Chess Server".into())
    );
    assert!(
        seen.contains(&IcsEvent::Prompt("login: ".into())),
        "{seen:?}"
    );

    link.send_line("guest").unwrap();
    link.send_line("").unwrap();
    assert!(server.wait_lines(2, WAIT));
    assert_eq!(server.received(), b"guest\n\n");

    link.close();
    assert!(server.wait_disconnected(WAIT));
    assert!(eventually(WAIT, || net.status().active_requests == 0));
}

#[test]
fn a_command_never_hides_another_line_or_a_control_byte() {
    let clock = clock();
    let server = FakeIcsServer::start();
    let net = fics_net(&server, &clock);
    net.set_enabled(ProviderId::Fics, true).unwrap();
    let link = net.connect_ics(fics());
    assert_eq!(link.next_timeout(WAIT), Some(IcsEvent::Connected));

    for bad in [
        "guest\nsenha123".to_owned(),
        "guest\rsenha123".to_owned(),
        "observe 1\u{1b}".to_owned(),
        "tell guest olá".to_owned(),
        format!("x{}", "\u{ff}"),
        "a".repeat(MAX_ICS_COMMAND + 1),
    ] {
        assert!(
            matches!(link.send_line(&bad), Err(NetError::InvalidRequest { .. })),
            "{bad:?}"
        );
    }
    link.send_line("finger").unwrap();
    assert!(server.wait_line("finger", WAIT));
    assert_eq!(server.received(), b"finger\n", "só o comando válido saiu");
}

#[test]
fn hostile_input_neither_hangs_nor_kills_the_connection() {
    let clock = clock();
    let server = FakeIcsServer::start();
    let net = fics_net(&server, &clock);
    net.set_enabled(ProviderId::Fics, true).unwrap();
    let link = net.connect_ics(fics().max_line(4096));
    assert_eq!(link.next_timeout(WAIT), Some(IcsEvent::Connected));

    // Linha gigante (2 MiB sem fim de linha), telnet no meio do texto, bytes inválidos e
    // controles; depois uma linha normal.
    server.send_raw(&vec![b'A'; 2 * 1024 * 1024]);
    server.send_raw(b"\n\r");
    server.send_raw(&[
        0xff, 0xfb, 0x01, b'o', 0xff, 0xfa, 0x18, 0x01, 0xff, 0xf0, b'k',
    ]);
    server.send_raw(&[0xc3, 0x28, 0x07, 0x1b, b'!', b'\n', b'\r']);
    server.send("fim\n");

    let seen = collect_until(&link, |s| s.contains(&IcsEvent::Line("fim".into())));
    let too_long = seen
        .iter()
        .filter(|e| matches!(e, IcsEvent::LineTooLong { limit: 4096 }))
        .count();
    assert_eq!(too_long, 1, "{seen:?}");
    let lines: Vec<&str> = seen
        .iter()
        .filter_map(|e| match e {
            IcsEvent::Line(l) => Some(l.as_str()),
            _ => None,
        })
        .collect();
    assert_eq!(lines, ["ok\u{FFFD}(!", "fim"]);
    // A conexão segue viva: o comando sai.
    link.send_line("date").unwrap();
    assert!(server.wait_line("date", WAIT));
}

#[test]
fn turning_the_provider_off_closes_the_open_connection() {
    let clock = clock();
    let server = FakeIcsServer::start();
    let net = fics_net(&server, &clock);
    net.set_enabled(ProviderId::Fics, true).unwrap();
    let link = net.connect_ics(fics());
    assert_eq!(link.next_timeout(WAIT), Some(IcsEvent::Connected));
    assert!(server.wait_connected(WAIT));

    net.set_enabled(ProviderId::Fics, false).unwrap();

    assert_eq!(
        next_event(&link),
        Some(IcsEvent::Failed(NetError::ProviderDisabled {
            provider: ProviderId::Fics
        }))
    );
    assert!(server.wait_disconnected(WAIT));
    assert!(eventually(WAIT, || net.status().active_requests == 0));
}

#[test]
fn the_server_closing_ends_the_link() {
    let clock = clock();
    let server = FakeIcsServer::start();
    let net = fics_net(&server, &clock);
    net.set_enabled(ProviderId::Fics, true).unwrap();
    let link = net.connect_ics(fics());
    assert_eq!(link.next_timeout(WAIT), Some(IcsEvent::Connected));
    assert!(server.wait_connected(WAIT));

    server.disconnect();

    assert_eq!(next_event(&link), Some(IcsEvent::Ended));
}

#[test]
fn a_refused_connection_marks_the_provider_failing() {
    let clock = clock();
    // Uma porta de loopback que ninguém escuta.
    let port = {
        let probe = std::net::TcpListener::bind(("127.0.0.1", 0)).unwrap();
        probe.local_addr().unwrap().port()
    };
    let net = Net::open(
        NetConfig::in_memory()
            .clock(clock.clone())
            .route("freechess.org", &format!("http://127.0.0.1:{port}")),
    )
    .unwrap();
    net.set_enabled(ProviderId::Fics, true).unwrap();

    let link = net.connect_ics(fics());

    assert!(matches!(
        next_event(&link),
        Some(IcsEvent::Failed(NetError::Unreachable {
            provider: ProviderId::Fics,
            ..
        }))
    ));
    assert!(matches!(
        net.status().state(ProviderId::Fics),
        ProviderState::Failing { .. }
    ));
}

#[test]
fn dropping_the_link_closes_the_connection() {
    let clock = clock();
    let server = FakeIcsServer::start();
    let net = fics_net(&server, &clock);
    net.set_enabled(ProviderId::Fics, true).unwrap();
    let link = net.connect_ics(fics());
    assert_eq!(link.next_timeout(WAIT), Some(IcsEvent::Connected));
    assert!(server.wait_connected(WAIT));

    drop(link);

    assert!(server.wait_disconnected(WAIT));
    assert!(eventually(WAIT, || net.status().active_requests == 0));
}
