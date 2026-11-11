//! Transporte HTTP (`ureq` + rustls). É o único arquivo do crate que fala `ureq`, e só o
//! gate (`net.rs`) o chama: nenhum caminho chega aqui sem passar pelo consentimento, pelo
//! catálogo, pela Trava de fair play e pelos limites. Não segue redirecionamentos.

use crate::USER_AGENT;
use crate::request::Method;
use std::io::{ErrorKind, Read};
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

/// O pedido pronto para sair (endereço já roteado, cabeçalhos já filtrados pelo gate).
pub(crate) struct Outgoing<'a> {
    pub method: Method,
    pub url: String,
    pub headers: Vec<(String, String)>,
    /// Valor do `Authorization`, já conferido contra o host pelo gate.
    pub authorization: Option<&'a str>,
    pub body: Option<&'a (String, Vec<u8>)>,
    pub timeout: Duration,
    pub max_body: usize,
    pub cancel: &'a AtomicBool,
}

/// Cabeçalhos da resposta que a camada usa.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(crate) struct ResponseHead {
    pub status: u16,
    pub content_type: Option<String>,
    /// Tamanho do corpo anunciado (`Content-Length`), se houver.
    pub content_length: Option<u64>,
    pub etag: Option<String>,
    pub last_modified: Option<String>,
    pub retry_after: Option<String>,
    pub location: Option<String>,
    /// Escopos do token, como o Provedor os declara (`X-OAuth-Scopes`).
    pub oauth_scopes: Option<String>,
}

#[derive(Debug)]
pub(crate) enum Failure {
    Timeout,
    /// Sem rota até o Provedor (rede fora, DNS, recusada): não adianta repetir já.
    Unreachable(&'static str),
    /// Conexão caiu no meio: vale repetir.
    Interrupted(&'static str),
    BodyTooLarge,
    Cancelled,
}

pub(crate) type BodyStream = Box<dyn Read + Send>;

#[derive(Debug)]
pub(crate) struct Http {
    agent: ureq::Agent,
}

impl Http {
    pub fn new(connect_timeout: Duration) -> Self {
        let agent: ureq::Agent = ureq::Agent::config_builder()
            // Todo status chega como resposta: 304, 429 e 5xx são tratados pelo gate.
            .http_status_as_error(false)
            // Redirecionamento poderia levar a outro host (e o token junto): nunca seguir.
            .max_redirects(0)
            .user_agent(USER_AGENT)
            .timeout_connect(Some(connect_timeout))
            .build()
            .into();
        Self { agent }
    }

    /// Pedido completo: cabeçalhos e corpo (com limite e atenção ao cancelamento).
    pub fn send(&self, out: &Outgoing<'_>) -> Result<(ResponseHead, Vec<u8>), Failure> {
        let response = self.call(out, Some(out.timeout), None)?;
        let head = head(&response);
        let mut reader = response
            .into_body()
            .into_with_config()
            .limit(u64::MAX)
            .reader();
        let mut body = Vec::new();
        let mut chunk = [0u8; 16 * 1024];
        loop {
            if out.cancel.load(Ordering::Acquire) {
                return Err(Failure::Cancelled);
            }
            match reader.read(&mut chunk) {
                Ok(0) => return Ok((head, body)),
                Ok(n) => {
                    if body.len() + n > out.max_body {
                        return Err(Failure::BodyTooLarge);
                    }
                    body.extend_from_slice(&chunk[..n]);
                }
                Err(e) if e.kind() == ErrorKind::Interrupted => {}
                Err(e) => return Err(io_failure(e.kind())),
            }
        }
    }

    /// Fluxo (NDJSON): só os cabeçalhos têm prazo; cada leitura do corpo tem `read_timeout`.
    pub fn open(
        &self,
        out: &Outgoing<'_>,
        read_timeout: Duration,
    ) -> Result<(ResponseHead, BodyStream), Failure> {
        let response = self.call(out, None, Some((out.timeout, read_timeout)))?;
        let head = head(&response);
        let reader = response
            .into_body()
            .into_with_config()
            .limit(u64::MAX)
            .reader();
        Ok((head, Box::new(reader)))
    }

    fn call(
        &self,
        out: &Outgoing<'_>,
        global: Option<Duration>,
        stream: Option<(Duration, Duration)>,
    ) -> Result<ureq::http::Response<ureq::Body>, Failure> {
        if out.cancel.load(Ordering::Acquire) {
            return Err(Failure::Cancelled);
        }
        let result = match out.method {
            Method::Get | Method::Delete => {
                let mut req = if out.method == Method::Get {
                    self.agent.get(&out.url)
                } else {
                    self.agent.delete(&out.url)
                };
                for (name, value) in &out.headers {
                    req = req.header(name.as_str(), value.as_str());
                }
                if let Some(auth) = out.authorization {
                    req = req.header("authorization", auth);
                }
                let mut config = req.config().timeout_global(global);
                if let Some((headers, per_read)) = stream {
                    config = config
                        .timeout_recv_response(Some(headers))
                        .timeout_per_read(Some(per_read));
                }
                config.build().call()
            }
            Method::Post => {
                let mut req = self.agent.post(&out.url);
                for (name, value) in &out.headers {
                    req = req.header(name.as_str(), value.as_str());
                }
                if let Some(auth) = out.authorization {
                    req = req.header("authorization", auth);
                }
                let (content_type, bytes) = match out.body {
                    Some((t, b)) => (t.as_str(), b.as_slice()),
                    None => ("application/octet-stream", &[][..]),
                };
                let mut config = req
                    .header("content-type", content_type)
                    .config()
                    .timeout_global(global);
                if let Some((headers, per_read)) = stream {
                    config = config
                        .timeout_recv_response(Some(headers))
                        .timeout_per_read(Some(per_read));
                }
                config.build().send(bytes)
            }
        };
        result.map_err(failure)
    }
}

fn head(response: &ureq::http::Response<ureq::Body>) -> ResponseHead {
    let header = |name: &str| {
        response
            .headers()
            .get(name)
            .and_then(|v| v.to_str().ok())
            .map(str::to_owned)
    };
    ResponseHead {
        status: response.status().as_u16(),
        content_type: header("content-type"),
        content_length: header("content-length").and_then(|v| v.trim().parse().ok()),
        etag: header("etag"),
        last_modified: header("last-modified"),
        retry_after: header("retry-after"),
        location: header("location"),
        oauth_scopes: header("x-oauth-scopes"),
    }
}

/// Mensagens fixas: o erro do `ureq` pode citar a URL, que não vai para o diagnóstico.
fn failure(err: ureq::Error) -> Failure {
    match err {
        ureq::Error::Timeout(_) => Failure::Timeout,
        ureq::Error::HostNotFound => Failure::Unreachable("host não encontrado"),
        ureq::Error::ConnectionFailed => Failure::Unreachable("conexão recusada"),
        ureq::Error::BodyExceedsLimit(_) => Failure::BodyTooLarge,
        ureq::Error::Io(e) => io_failure(e.kind()),
        ureq::Error::Tls(_) | ureq::Error::Rustls(_) => Failure::Unreachable("falha de TLS"),
        ureq::Error::Protocol(_) => Failure::Interrupted("resposta HTTP malformada"),
        _ => Failure::Unreachable("falha de rede"),
    }
}

fn io_failure(kind: ErrorKind) -> Failure {
    match kind {
        ErrorKind::TimedOut | ErrorKind::WouldBlock => Failure::Timeout,
        ErrorKind::ConnectionRefused => Failure::Unreachable("conexão recusada"),
        ErrorKind::NotConnected
        | ErrorKind::AddrNotAvailable
        | ErrorKind::NetworkUnreachable
        | ErrorKind::HostUnreachable
        | ErrorKind::NetworkDown => Failure::Unreachable("rede indisponível"),
        _ => Failure::Interrupted("conexão interrompida"),
    }
}
