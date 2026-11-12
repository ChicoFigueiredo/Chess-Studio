//! Servidor de loopback do retorno da autorização pelo navegador (OAuth2 de app nativo,
//! RFC 8252 §7.3). É a **única escuta de rede do app** (exceção registrada no teste estático
//! `chess-studio/tests/no_network_without_consent.rs`), e por isso é mínimo:
//!
//! - escuta só em `127.0.0.1`, porta efêmera escolhida pelo sistema;
//! - aceita conexões só de endereço de loopback e só com `Host: 127.0.0.1:<porta>` (contra
//!   *DNS rebinding*);
//! - processa **um único** retorno (`GET` no caminho combinado); depois fecha a porta, e uma
//!   segunda requisição nem encontra quem a ouça. Requisições a outro caminho (o `favicon`
//!   do navegador) recebem `404` e não contam;
//! - tem prazo (relógio injetado) e cancelamento;
//! - responde uma página mínima, sem script nem recurso externo: "pode voltar ao Chess
//!   Studio".
//!
//! Quem valida o `state` e troca o código é o fluxo do Provedor (`chess-providers`); aqui só
//! se recebe o retorno.

use crate::clock::{Clock, Timestamp};
use std::fmt;
use std::io::{ErrorKind, Read, Write};
use std::net::{IpAddr, Ipv4Addr, SocketAddr, TcpListener, TcpStream};
use std::sync::atomic::{AtomicBool, Ordering};
use std::thread;
use std::time::{Duration, Instant};

/// Intervalo entre as consultas ao prazo e ao cancelamento (tempo real).
const POLL: Duration = Duration::from_millis(10);
/// Prazo **total** (tempo real, desde o `accept`) para uma conexão mandar o cabeçalho
/// inteiro (o navegador abre conexões de reserva que às vezes não usa). Vale também contra
/// um cliente local que mande um byte de vez em quando (SEC-04).
const HEAD_TIMEOUT: Duration = Duration::from_secs(3);
/// Fatia de cada leitura: entre uma e outra o cancelamento e o prazo da autorização são
/// conferidos, então o Cancelar responde mesmo com um cliente lento na linha.
const READ_SLICE: Duration = Duration::from_millis(50);
/// Prazo para escrever a resposta.
const WRITE_TIMEOUT: Duration = Duration::from_secs(2);
/// Cabeçalho máximo de uma requisição.
const MAX_HEAD: usize = 8 * 1024;

const PAGE: &str = "<!doctype html><html lang=\"pt-BR\"><head><meta charset=\"utf-8\">\
<title>Chess Studio</title></head><body><p>Pode voltar ao Chess Studio.</p></body></html>";

/// Por que o retorno não chegou.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CallbackError {
    /// Não foi possível escutar em `127.0.0.1`.
    Unavailable,
    /// O prazo passou sem retorno.
    Expired,
    /// Quem esperava desistiu.
    Cancelled,
}

impl fmt::Display for CallbackError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::Unavailable => {
                "não foi possível receber o retorno do navegador (porta local indisponível)"
            }
            Self::Expired => "o prazo para concluir a autorização no navegador passou",
            Self::Cancelled => "autorização cancelada",
        })
    }
}

impl std::error::Error for CallbackError {}

/// Parâmetros do retorno (`?code=…&state=…` ou `?error=…&state=…`), já decodificados. O
/// `Debug` mostra só os nomes: o código de autorização não vai para log.
#[derive(Clone, PartialEq, Eq)]
pub struct CallbackParams {
    pairs: Vec<(String, String)>,
}

impl CallbackParams {
    /// Primeiro valor de `key`.
    pub fn get(&self, key: &str) -> Option<&str> {
        self.pairs
            .iter()
            .find(|(k, _)| k == key)
            .map(|(_, v)| v.as_str())
    }

    /// Quantas vezes `key` aparece (um retorno legítimo traz cada uma no máximo uma vez).
    pub fn count(&self, key: &str) -> usize {
        self.pairs.iter().filter(|(k, _)| k == key).count()
    }
}

impl fmt::Debug for CallbackParams {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let keys: Vec<&str> = self.pairs.iter().map(|(k, _)| k.as_str()).collect();
        f.debug_struct("CallbackParams")
            .field("keys", &keys)
            .finish()
    }
}

/// O servidor de loopback, aberto e esperando um retorno. Descartar fecha a porta.
pub struct LoopbackCallback {
    listener: TcpListener,
    port: u16,
    path: String,
}

impl fmt::Debug for LoopbackCallback {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("LoopbackCallback")
            .field("port", &self.port)
            .field("path", &self.path)
            .finish()
    }
}

/// Só conexões vindas do próprio computador.
fn accepts_peer(peer: &SocketAddr) -> bool {
    peer.ip().is_loopback()
}

impl LoopbackCallback {
    /// Escuta em `127.0.0.1` numa porta efêmera; o retorno é esperado em `path` (por exemplo
    /// `/retorno`).
    pub fn bind(path: &str) -> Result<Self, CallbackError> {
        if !path.starts_with('/') || path.contains(['?', '#', ' ']) {
            return Err(CallbackError::Unavailable);
        }
        let address = SocketAddr::new(IpAddr::V4(Ipv4Addr::LOCALHOST), 0);
        let listener = TcpListener::bind(address).map_err(|_| CallbackError::Unavailable)?;
        listener
            .set_nonblocking(true)
            .map_err(|_| CallbackError::Unavailable)?;
        let port = listener
            .local_addr()
            .map_err(|_| CallbackError::Unavailable)?
            .port();
        Ok(Self {
            listener,
            port,
            path: path.to_owned(),
        })
    }

    pub fn port(&self) -> u16 {
        self.port
    }

    /// `http://127.0.0.1:<porta><caminho>`: o `redirect_uri` da autorização. Sempre o IP
    /// literal, nunca `localhost` (que pode resolver para outro endereço).
    pub fn redirect_uri(&self) -> String {
        format!("http://127.0.0.1:{}{}", self.port, self.path)
    }

    /// Espera o único retorno até `deadline` (no relógio `clock`) ou até `cancel`. Consome o
    /// servidor: ao voltar, a porta já está fechada.
    pub fn wait(
        self,
        clock: &dyn Clock,
        deadline: Timestamp,
        cancel: &AtomicBool,
    ) -> Result<CallbackParams, CallbackError> {
        loop {
            if cancel.load(Ordering::Acquire) {
                return Err(CallbackError::Cancelled);
            }
            if clock.now() >= deadline {
                return Err(CallbackError::Expired);
            }
            match self.listener.accept() {
                Ok((stream, peer)) => {
                    if !accepts_peer(&peer) {
                        continue;
                    }
                    if let Some(params) = self.serve(stream, clock, deadline, cancel) {
                        return Ok(params);
                    }
                }
                Err(e) if e.kind() == ErrorKind::WouldBlock => thread::sleep(POLL),
                Err(_) => thread::sleep(POLL),
            }
        }
    }

    /// Atende uma conexão. `Some` só para o retorno (o `GET` no caminho combinado, com o
    /// `Host` certo); o resto recebe erro e não conta.
    fn serve(
        &self,
        mut stream: TcpStream,
        clock: &dyn Clock,
        deadline: Timestamp,
        cancel: &AtomicBool,
    ) -> Option<CallbackParams> {
        let _ = stream.set_nonblocking(false);
        let _ = stream.set_read_timeout(Some(READ_SLICE));
        let _ = stream.set_write_timeout(Some(WRITE_TIMEOUT));
        let head = read_head(&mut stream, clock, deadline, cancel)?;
        let mut lines = head.split("\r\n");
        let mut first = lines.next()?.split(' ');
        let (method, target) = (first.next()?, first.next()?);
        let expected_host = format!("127.0.0.1:{}", self.port);
        let host_ok = lines
            .filter_map(|l| l.split_once(':'))
            .filter(|(k, _)| k.trim().eq_ignore_ascii_case("host"))
            .map(|(_, v)| v.trim())
            .collect::<Vec<_>>()
            == [expected_host.as_str()];
        let (path, query) = target.split_once('?').unwrap_or((target, ""));
        if method != "GET" || path != self.path || !host_ok {
            respond(&mut stream, "404 Not Found", "");
            return None;
        }
        respond(&mut stream, "200 OK", PAGE);
        Some(CallbackParams {
            pairs: parse_query(query),
        })
    }
}

/// Lê o cabeçalho com prazo total ([`HEAD_TIMEOUT`]); desiste se a autorização foi
/// cancelada ou seu prazo (relógio injetado) passou.
fn read_head(
    stream: &mut TcpStream,
    clock: &dyn Clock,
    deadline: Timestamp,
    cancel: &AtomicBool,
) -> Option<String> {
    let started = Instant::now();
    let mut data = Vec::new();
    let mut buf = [0u8; 1024];
    loop {
        if started.elapsed() >= HEAD_TIMEOUT
            || cancel.load(Ordering::Acquire)
            || clock.now() >= deadline
        {
            return None;
        }
        if let Some(end) = data.windows(4).position(|w| w == b"\r\n\r\n") {
            return String::from_utf8(data[..end].to_vec()).ok();
        }
        if data.len() > MAX_HEAD {
            return None;
        }
        match stream.read(&mut buf) {
            Ok(0) => return None,
            Ok(n) => data.extend_from_slice(&buf[..n]),
            Err(e) if matches!(e.kind(), ErrorKind::WouldBlock | ErrorKind::TimedOut) => {}
            Err(_) => return None,
        }
    }
}

fn respond(stream: &mut TcpStream, status: &str, body: &str) {
    let head = format!(
        "HTTP/1.1 {status}\r\n\
         content-type: text/html; charset=utf-8\r\n\
         content-length: {}\r\n\
         cache-control: no-store\r\n\
         referrer-policy: no-referrer\r\n\
         x-content-type-options: nosniff\r\n\
         content-security-policy: default-src 'none'\r\n\
         connection: close\r\n\r\n",
        body.len()
    );
    let _ = stream
        .write_all(head.as_bytes())
        .and_then(|()| stream.write_all(body.as_bytes()))
        .and_then(|()| stream.flush());
    let _ = stream.shutdown(std::net::Shutdown::Both);
}

fn parse_query(query: &str) -> Vec<(String, String)> {
    query
        .split('&')
        .filter(|p| !p.is_empty())
        .map(|pair| {
            let (k, v) = pair.split_once('=').unwrap_or((pair, ""));
            (decode(k), decode(v))
        })
        .collect()
}

/// Decodificação de consulta: `%XX` e `+` (espaço).
fn decode(text: &str) -> String {
    let bytes = text.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        match bytes[i] {
            b'%' => {
                if let Some(byte) = text
                    .get(i + 1..i + 3)
                    .and_then(|h| u8::from_str_radix(h, 16).ok())
                {
                    out.push(byte);
                    i += 3;
                    continue;
                }
                out.push(b'%');
            }
            b'+' => out.push(b' '),
            b => out.push(b),
        }
        i += 1;
    }
    String::from_utf8_lossy(&out).into_owned()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_loopback_peers_are_accepted() {
        for ok in ["127.0.0.1:5000", "127.8.9.10:1", "[::1]:80"] {
            let peer: SocketAddr = ok.parse().expect("endereço");
            assert!(accepts_peer(&peer), "{ok}");
        }
        for bad in [
            "192.168.0.10:5000",
            "10.0.0.1:1",
            "0.0.0.0:2",
            "[2001:db8::1]:80",
        ] {
            let peer: SocketAddr = bad.parse().expect("endereço");
            assert!(!accepts_peer(&peer), "{bad}");
        }
    }

    #[test]
    fn query_values_are_decoded() {
        let pairs = parse_query("code=a%2Fb&state=x+y&error=&solo");
        assert_eq!(
            pairs,
            vec![
                ("code".to_owned(), "a/b".to_owned()),
                ("state".to_owned(), "x y".to_owned()),
                ("error".to_owned(), String::new()),
                ("solo".to_owned(), String::new()),
            ]
        );
    }

    #[test]
    fn a_bad_path_is_refused() {
        assert!(LoopbackCallback::bind("sem-barra").is_err());
        assert!(LoopbackCallback::bind("/a?b").is_err());
    }
}
