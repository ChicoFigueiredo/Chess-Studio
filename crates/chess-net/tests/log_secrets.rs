#![allow(clippy::unwrap_used)]
//! Revisão de segurança do V4 (#273): nenhum segredo vai para o log, nem com o log no nível
//! mais detalhado. O `ureq-proto` despeja em `TRACE` os bytes do pedido e da resposta (em
//! hexadecimal e em texto), com o `Authorization`, o corpo da troca do código (código e
//! verificador PKCE) e a resposta com o token; quem roda com `RUST_LOG=trace` para
//! diagnosticar e cola o log num relato publicaria o token. O teste liga um registrador
//! que guarda tudo, no nível `TRACE`, e procura os segredos em claro e no despejo
//! hexadecimal. Fica sozinho neste arquivo: o registrador do `log` é global ao binário.

mod common;

use chess_net::testing::FakeResponse;
use chess_net::{ProviderId, Request};
use common::*;
use log::{LevelFilter, Log, Metadata, Record};
use std::sync::Mutex;

const VERIFIER: &str = "VERIFICADOR_PKCE_QUE_NAO_PODE_VAZAR_0123456789";
const CODE: &str = "CODIGO_DE_AUTORIZACAO_QUE_NAO_PODE_VAZAR";
const NEW_TOKEN: &str = "lio_TOKEN_NOVO_QUE_NAO_PODE_VAZAR_9876543210";

/// Guarda o texto de todos os registros, de todos os níveis.
struct Capture(Mutex<Vec<String>>);

impl Log for Capture {
    fn enabled(&self, _metadata: &Metadata<'_>) -> bool {
        true
    }

    fn log(&self, record: &Record<'_>) {
        let text = format!("{} {}", record.target(), record.args());
        self.0.lock().unwrap().push(text);
    }

    fn flush(&self) {}
}

static CAPTURE: Capture = Capture(Mutex::new(Vec::new()));

/// Os bytes do despejo hexadecimal do `ureq-proto` (linhas `4745 5420 …  GET.…`), em ordem:
/// um segredo partido entre duas linhas do despejo reaparece inteiro aqui.
fn dumped_bytes(lines: &[String]) -> Vec<u8> {
    let mut out = Vec::new();
    for line in lines {
        let message = line.split_once(' ').map_or("", |(_, m)| m);
        let groups: Vec<&str> = message.split(' ').take(8).collect();
        let is_row = groups.len() == 8
            && groups
                .iter()
                .all(|g| g.len() == 4 && g.chars().all(|c| c.is_ascii_hexdigit() || c == '-'));
        if !is_row {
            continue;
        }
        for group in groups {
            for pair in [&group[..2], &group[2..]] {
                if let Ok(byte) = u8::from_str_radix(pair, 16) {
                    out.push(byte);
                }
            }
        }
    }
    out
}

fn contains(haystack: &[u8], needle: &str) -> bool {
    haystack
        .windows(needle.len())
        .any(|w| w == needle.as_bytes())
}

#[test]
fn no_secret_reaches_the_log_even_at_trace_level() {
    log::set_logger(&CAPTURE).unwrap();
    log::set_max_level(LevelFilter::Trace);

    let clock = clock();
    let server = server(&clock);
    server.route("/masters", FakeResponse::json(r#"{"moves":[]}"#));
    server.route(
        "/api/token",
        FakeResponse::json(&format!(
            r#"{{"token_type":"Bearer","access_token":"{NEW_TOKEN}","expires_in":31536000}}"#
        )),
    );
    let net = net_enabled(&server, &clock, &[ProviderId::Lichess]);

    // Pedido com o token da Conexão no `Authorization`.
    net.fetch(masters()).wait().unwrap();
    // Troca do código: o corpo leva código e verificador; a resposta traz o token novo.
    let body = format!(
        "grant_type=authorization_code&code={CODE}&code_verifier={VERIFIER}&client_id=chess-studio"
    );
    net.fetch(
        Request::post(ProviderId::Lichess, "https://lichess.org/api/token")
            .body("application/x-www-form-urlencoded", body.into_bytes()),
    )
    .wait()
    .unwrap();
    assert_eq!(server.requests().len(), 2, "os dois pedidos saíram");

    let lines = CAPTURE.0.lock().unwrap().clone();
    assert!(
        !lines.is_empty(),
        "o registrador recebeu algo (o teste não é vazio)"
    );
    let text = lines.join("\n");
    let dumped = dumped_bytes(&lines);
    for secret in [EXPLORER_TOKEN, CODE, VERIFIER, NEW_TOKEN] {
        assert!(!text.contains(secret), "{secret} apareceu em claro no log");
        assert!(
            !contains(&dumped, secret),
            "{secret} apareceu no despejo hexadecimal do log"
        );
    }
}
