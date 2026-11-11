//! Respostas gravadas (CX-17): o formato é a saída de `curl -i` (linha de status, cabeçalhos,
//! linha em branco, corpo), com comentários `#` no topo. O servidor falso as serve como se
//! fossem do Provedor.

mod common;

use chess_net::testing::FakeResponse;
use chess_net::{Freshness, NdjsonItem, NdjsonReader, ProviderId, Request};
use common::*;
use std::path::Path;
use std::time::Duration;

fn fixture(name: &str) -> FakeResponse {
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures")
        .join(name);
    FakeResponse::recorded(&path).expect("resposta gravada válida")
}

#[test]
fn every_recorded_fixture_parses() {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures");
    let mut count = 0;
    for entry in std::fs::read_dir(dir).expect("fixtures") {
        let path = entry.expect("entrada").path();
        if path.extension().is_some_and(|e| e == "http") {
            FakeResponse::recorded(&path).unwrap_or_else(|e| panic!("{}: {e}", path.display()));
            count += 1;
        }
    }
    assert!(count >= 3);
}

#[test]
fn the_recording_format_is_curl_dash_i() {
    let response = FakeResponse::parse_recording(
        "# comentário\nHTTP/1.1 429 Too Many Requests\nRetry-After: 60\r\nContent-Length: 2\n\nno",
    )
    .expect("formato");
    assert_eq!(response.status_code(), 429);
    assert_eq!(response.header_value("retry-after"), Some("60"));
    assert_eq!(
        response.header_value("content-length"),
        None,
        "recalculado ao servir"
    );
    assert_eq!(response.body_bytes(), b"no");

    assert!(FakeResponse::parse_recording("sem linha de status").is_err());
}

#[test]
fn a_recorded_explorer_answer_flows_through_the_whole_layer() {
    let clock = clock();
    let server = server(&clock);
    server.route("/masters", fixture("lichess-explorer-masters.http"));
    let net = net_enabled(&server, &clock, &[ProviderId::Lichess]);

    let fetched = net
        .fetch(masters().cache_for(Duration::from_secs(3600)))
        .wait()
        .expect("resposta gravada");

    assert_eq!(fetched.status, 200);
    assert_eq!(fetched.freshness, Freshness::Network);
    let json: serde_json::Value = serde_json::from_slice(&fetched.body).expect("json");
    assert_eq!(json["opening"]["eco"], "B00");
    assert_eq!(fetched.content_type.as_deref(), Some("application/json"));
}

#[test]
fn a_recorded_ndjson_export_reads_line_by_line() {
    let response = fixture("lichess-games-user.http");
    let mut reader = NdjsonReader::new(response.body_bytes(), 4096);
    let mut lines = 0;
    let mut heartbeats = 0;
    while let Some(item) = reader.next_item().expect("item") {
        match item {
            NdjsonItem::Line(text) => {
                serde_json::from_str::<serde_json::Value>(&text).expect("json por linha");
                lines += 1;
            }
            NdjsonItem::Heartbeat => heartbeats += 1,
        }
    }
    assert_eq!((lines, heartbeats), (2, 1));
}

#[test]
fn a_recorded_chess_com_answer_is_served_with_its_validators() {
    let clock = clock();
    let server = server(&clock);
    server.route(
        "/pub/player/usuario/games/archives",
        fixture("chesscom-archives.http"),
    );
    let net = net_enabled(&server, &clock, &[ProviderId::ChessCom]);

    net.fetch(
        Request::get(
            ProviderId::ChessCom,
            "https://api.chess.com/pub/player/usuario/games/archives",
        )
        .cache_for(Duration::from_secs(60)),
    )
    .wait()
    .expect("rede");
    clock.advance(Duration::from_secs(120));
    let _ = net
        .fetch(
            Request::get(
                ProviderId::ChessCom,
                "https://api.chess.com/pub/player/usuario/games/archives",
            )
            .cache_for(Duration::from_secs(60)),
        )
        .wait();

    let second = &server.requests()[1];
    assert_eq!(second.header("if-none-match"), Some("W/\"archives-1\""));
    assert_eq!(
        second.header("if-modified-since"),
        Some("Tue, 10 Nov 2026 08:00:00 GMT")
    );
}
