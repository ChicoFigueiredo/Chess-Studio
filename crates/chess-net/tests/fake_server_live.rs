#![allow(clippy::unwrap_used)]
//! Apoio de teste da Partida online (#230): fluxo alimentado pelo teste na hora e queda de
//! conexão no meio de um `POST` (que nunca é repetido pela camada de rede).

mod common;

use chess_net::testing::{FakeResponse, LiveFeed};
use chess_net::{NetError, ProviderId, Request, StreamEvent};
use common::*;
use std::time::Duration;

const WAIT: Duration = Duration::from_secs(10);

fn next_line(stream: &chess_net::Stream) -> Option<StreamEvent> {
    stream.next_timeout(WAIT)
}

#[test]
fn fluxo_vivo_entrega_cada_linha_quando_o_teste_empurra_e_termina_ao_fechar() {
    let clock = clock();
    let server = server(&clock);
    let feed = LiveFeed::new();
    server.route("/api/stream/event", FakeResponse::live(&feed));
    let net = net_enabled(&server, &clock, &[ProviderId::Lichess]);
    let stream = net.stream(
        Request::get(ProviderId::Lichess, "https://lichess.org/api/stream/event")
            .connection(EXPLORER_CONNECTION),
    );
    assert!(feed.wait_connected(WAIT));
    feed.push(r#"{"type":"gameStart"}"#);
    assert_eq!(
        next_line(&stream),
        Some(StreamEvent::Line(r#"{"type":"gameStart"}"#.into()))
    );
    feed.push("");
    feed.push(r#"{"type":"gameFinish"}"#);
    assert_eq!(
        next_line(&stream),
        Some(StreamEvent::Line(r#"{"type":"gameFinish"}"#.into()))
    );
    feed.close();
    assert_eq!(next_line(&stream), Some(StreamEvent::Ended));
}

#[test]
fn queda_no_meio_do_post_volta_como_erro_e_o_post_nao_se_repete() {
    let clock = clock();
    let server = server(&clock);
    let path = "/api/board/game/abcd1234/move/e2e4";
    server.route(path, FakeResponse::drop_connection());
    let net = net_enabled(&server, &clock, &[ProviderId::Lichess]);
    let result = net
        .fetch(
            Request::post(ProviderId::Lichess, format!("https://lichess.org{path}"))
                .connection(EXPLORER_CONNECTION),
        )
        .wait();
    assert!(
        matches!(result, Err(NetError::Unreachable { .. })),
        "{result:?}"
    );
    let posts = server.requests().iter().filter(|r| r.path == path).count();
    assert_eq!(posts, 1, "POST nunca se repete às cegas");
}
