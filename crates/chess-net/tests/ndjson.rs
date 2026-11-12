//! Leitor NDJSON incremental (fluxos do Lichess): linhas partidas entre leituras, heartbeat,
//! linha acima do máximo e fim de fluxo; e o fluxo pela rede, com encerramento controlado.

mod common;

use chess_net::testing::FakeResponse;
use chess_net::{
    NdjsonError, NdjsonItem, NdjsonReader, NetError, ProviderId, Request, StreamEvent,
};
use common::*;
use std::collections::VecDeque;
use std::io::Read;
use std::time::Duration;

/// `Read` que entrega exatamente os pedaços dados, um por leitura.
struct Chunks(VecDeque<Vec<u8>>);

impl Chunks {
    fn new(parts: &[&[u8]]) -> Self {
        Self(parts.iter().map(|p| p.to_vec()).collect())
    }
}

impl Read for Chunks {
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        let Some(mut part) = self.0.pop_front() else {
            return Ok(0);
        };
        let n = part.len().min(buf.len());
        buf[..n].copy_from_slice(&part[..n]);
        if n < part.len() {
            self.0.push_front(part.split_off(n));
        }
        Ok(n)
    }
}

fn all(reader: &mut NdjsonReader<Chunks>) -> Vec<Result<NdjsonItem, NdjsonError>> {
    let mut out = Vec::new();
    loop {
        match reader.next_item() {
            Ok(None) => return out,
            item => out.push(item.map(|i| i.expect("item"))),
        }
        assert!(out.len() < 100, "não terminou");
    }
}

fn line(text: &str) -> Result<NdjsonItem, NdjsonError> {
    Ok(NdjsonItem::Line(text.to_owned()))
}

#[test]
fn lines_split_across_reads_are_joined() {
    let mut reader = NdjsonReader::new(
        Chunks::new(&[
            b"{\"type\":\"ga",
            b"meStart\"}\n{\"a\"",
            b":1}\n{\"b\":2}\n{\"c\":3}\n",
        ]),
        1024,
    );
    assert_eq!(
        all(&mut reader),
        vec![
            line("{\"type\":\"gameStart\"}"),
            line("{\"a\":1}"),
            line("{\"b\":2}"),
            line("{\"c\":3}"),
        ]
    );
}

#[test]
fn empty_lines_are_heartbeats_and_crlf_is_accepted() {
    let mut reader = NdjsonReader::new(Chunks::new(&[b"\n", b"\r\n{\"x\":1}\r\n", b"\n"]), 64);
    assert_eq!(
        all(&mut reader),
        vec![
            Ok(NdjsonItem::Heartbeat),
            Ok(NdjsonItem::Heartbeat),
            line("{\"x\":1}"),
            Ok(NdjsonItem::Heartbeat),
        ]
    );
}

#[test]
fn a_line_above_the_limit_is_reported_and_the_reader_resyncs() {
    let long = format!("{{\"pgn\":\"{}\"}}\n", "e4 ".repeat(40));
    let (head, tail) = long.as_bytes().split_at(30);
    let mut reader = NdjsonReader::new(Chunks::new(&[head, tail, b"{\"ok\":1}\n"]), 32);
    assert_eq!(
        all(&mut reader),
        vec![
            Err(NdjsonError::LineTooLong { limit: 32 }),
            line("{\"ok\":1}")
        ]
    );
}

#[test]
fn a_clean_end_of_stream_and_a_truncated_one_are_told_apart() {
    let mut clean = NdjsonReader::new(Chunks::new(&[b"{\"a\":1}\n"]), 64);
    assert_eq!(all(&mut clean), vec![line("{\"a\":1}")]);
    assert_eq!(clean.next_item(), Ok(None), "fim de fluxo é estável");

    let mut cut = NdjsonReader::new(Chunks::new(&[b"{\"a\":1}\n{\"b\":"]), 64);
    assert_eq!(
        all(&mut cut),
        vec![line("{\"a\":1}"), Err(NdjsonError::Truncated { bytes: 5 })]
    );
}

#[test]
fn invalid_utf8_is_an_error_not_a_panic() {
    let mut reader = NdjsonReader::new(Chunks::new(&[b"\xff\xfe\n{\"a\":1}\n"]), 64);
    assert_eq!(
        all(&mut reader),
        vec![Err(NdjsonError::InvalidUtf8), line("{\"a\":1}")]
    );
}

#[test]
fn a_stream_delivers_lines_and_then_ends() {
    let clock = clock();
    let server = server(&clock);
    server.route(
        "/api/games/user/someone",
        FakeResponse::stream(&[b"{\"id\":\"a\"}\n\n{\"id\"", b":\"b\"}\n"]),
    );
    let net = net_enabled(&server, &clock, &[ProviderId::Lichess]);
    let request = Request::get(
        ProviderId::Lichess,
        "https://lichess.org/api/games/user/someone",
    )
    .accept("application/x-ndjson");

    let stream = net.stream(request);
    let mut events = Vec::new();
    while let Some(event) = stream.next_timeout(Duration::from_secs(30)) {
        let end = matches!(event, StreamEvent::Ended | StreamEvent::Failed(_));
        events.push(event);
        if end {
            break;
        }
    }

    assert_eq!(
        events,
        vec![
            StreamEvent::Line("{\"id\":\"a\"}".to_owned()),
            StreamEvent::Line("{\"id\":\"b\"}".to_owned()),
            StreamEvent::Ended,
        ]
    );
    assert_eq!(
        server.requests()[0].header("accept"),
        Some("application/x-ndjson")
    );
}

#[test]
fn a_flood_of_lines_waits_in_a_bounded_queue_and_nothing_is_lost() {
    // SEC-07: a fila do fluxo tinha `mpsc::channel()` sem teto.
    const LINES: usize = 6000;
    let clock = clock();
    let server = server(&clock);
    let body = "{\"a\":1}\n".repeat(LINES);
    server.route(
        "/api/games/user/someone",
        FakeResponse::stream(&[body.as_bytes()]),
    );
    let net = net_enabled(&server, &clock, &[ProviderId::Lichess]);
    let request = Request::get(
        ProviderId::Lichess,
        "https://lichess.org/api/games/user/someone",
    )
    .accept("application/x-ndjson");

    let stream = net.stream(request);
    std::thread::sleep(Duration::from_millis(500));
    let mut at_once = 0;
    while let Some(StreamEvent::Line(_)) = stream.try_next() {
        at_once += 1;
    }
    assert!(
        at_once <= 2 * chess_net::MAX_STREAM_PENDING,
        "{at_once} linhas esperando de uma vez"
    );

    let mut total = at_once;
    loop {
        match stream.next_timeout(Duration::from_secs(5)) {
            Some(StreamEvent::Line(_)) => total += 1,
            Some(StreamEvent::Ended) => break,
            other => panic!("{other:?}"),
        }
    }
    assert_eq!(total, LINES);
}

#[test]
fn a_stream_obeys_the_same_gate_as_any_request() {
    let clock = clock();
    let server = server(&clock);
    let net = net(&server, &clock);

    let stream = net.stream(Request::get(
        ProviderId::Lichess,
        "https://lichess.org/api/games/user/someone",
    ));

    assert_eq!(
        stream.next_timeout(Duration::from_secs(30)),
        Some(StreamEvent::Failed(NetError::ProviderDisabled {
            provider: ProviderId::Lichess
        }))
    );
    assert_eq!(server.connections(), 0);
}

#[test]
fn closing_a_stream_stops_its_thread() {
    let clock = clock();
    let server = server(&clock);
    server.route(
        "/api/games/user/someone",
        FakeResponse::stream_forever(b"\n"),
    );
    let net = net_enabled(&server, &clock, &[ProviderId::Lichess]);

    let stream = net.stream(Request::get(
        ProviderId::Lichess,
        "https://lichess.org/api/games/user/someone",
    ));
    assert!(server.wait_for_requests(1, Duration::from_secs(30)));
    assert!(eventually(Duration::from_secs(30), || net
        .status()
        .active_requests
        == 1));
    stream.close();

    assert!(
        eventually(Duration::from_secs(30), || net.status().active_requests
            == 0),
        "a thread do fluxo ficou presa"
    );
}
