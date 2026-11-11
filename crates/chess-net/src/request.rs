//! O pedido: Provedor + endereço oficial + opções. Quem chama escreve a URL real do
//! Provedor (`https://explorer.lichess.org/masters`); o gate confere o endereço no catálogo,
//! deriva o que o pedido faz e, nos testes, troca o host pelo servidor falso.

use crate::catalog::{Capability, ProviderId};
use crate::error::NetError;
use crate::secret::ConnectionId;
use std::time::Duration;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Method {
    Get,
    Post,
}

impl Method {
    pub(crate) const fn as_str(self) -> &'static str {
        match self {
            Self::Get => "GET",
            Self::Post => "POST",
        }
    }
}

/// Cabeçalhos que nunca vêm de quem chama: autorização é do Cofre, cookie não se usa.
const FORBIDDEN_HEADERS: [&str; 6] = [
    "authorization",
    "proxy-authorization",
    "cookie",
    "host",
    "if-none-match",
    "if-modified-since",
];

/// Um pedido a um Provedor. Construa com [`Request::get`] ou [`Request::post`] e envie com
/// `Net::fetch` ou `Net::stream`.
#[derive(Debug, Clone)]
pub struct Request {
    pub(crate) provider: ProviderId,
    pub(crate) method: Method,
    pub(crate) url: String,
    pub(crate) query: Vec<(String, String)>,
    pub(crate) headers: Vec<(String, String)>,
    pub(crate) body: Option<(String, Vec<u8>)>,
    pub(crate) connection: Option<ConnectionId>,
    pub(crate) cache_ttl: Option<Duration>,
    pub(crate) allow_stale: bool,
    pub(crate) patience: Duration,
    pub(crate) timeout: Option<Duration>,
    pub(crate) max_body: Option<usize>,
    /// Erro de construção (FEN inválido, cabeçalho proibido), devolvido no envio.
    pub(crate) invalid: Option<String>,
}

impl Request {
    pub fn get(provider: ProviderId, url: impl Into<String>) -> Self {
        Self::new(provider, Method::Get, url.into())
    }

    pub fn post(provider: ProviderId, url: impl Into<String>) -> Self {
        Self::new(provider, Method::Post, url.into())
    }

    fn new(provider: ProviderId, method: Method, url: String) -> Self {
        Self {
            provider,
            method,
            url,
            query: Vec::new(),
            headers: Vec::new(),
            body: None,
            connection: None,
            cache_ttl: None,
            allow_stale: true,
            patience: Duration::ZERO,
            timeout: None,
            max_body: None,
            invalid: None,
        }
    }

    pub fn provider(&self) -> ProviderId {
        self.provider
    }

    /// O que o pedido faz, segundo o catálogo (`None` se o endereço não está nele). É a mesma
    /// classificação que o gate usa: [`Capability::computes`] diz se a Trava de fair play o
    /// bloqueia. Quem pede não declara nem muda isso.
    pub fn capability(&self) -> Option<Capability> {
        let target = self.target().ok()?;
        if target.port.is_some() {
            return None;
        }
        self.provider
            .info()
            .endpoint_for(&target.host, &target.path)
            .map(|e| e.capability)
    }

    /// Parâmetro de consulta (codificado na hora do envio; a ordem não muda a chave do cache).
    pub fn query(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        self.query.push((key.into(), value.into()));
        self
    }

    /// Parâmetro com uma Posição: o FEN é validado e normalizado pela mesma chave do índice
    /// local ([`chess_core::position_key`]: sem contadores, en passant só se capturável), e
    /// enviado como FEN completo (`<chave> 0 1`). Assim o Explorador local e o do Provedor
    /// concordam e Posições iguais caem no mesmo cache. Não use para finais, em que o relógio
    /// de 50 Lances muda a resposta: aí vai o FEN como está, por [`Request::query`].
    pub fn position(mut self, key: impl Into<String>, fen: &str) -> Self {
        match chess_core::CanonicalPosition::from_fen(fen.trim()) {
            Ok(position) => {
                let normalized = format!("{} 0 1", chess_core::position_key(&position));
                self.query.push((key.into(), normalized));
            }
            Err(e) => self.invalid = Some(format!("FEN inválido ({e})")),
        }
        self
    }

    /// Cabeçalho extra. `Authorization`, `Cookie` e os de revalidação são recusados: a
    /// autorização vem do Cofre e a revalidação, do cache.
    pub fn header(mut self, name: impl Into<String>, value: impl Into<String>) -> Self {
        let name = name.into();
        let lower = name.to_ascii_lowercase();
        if FORBIDDEN_HEADERS.contains(&lower.as_str()) {
            self.invalid = Some(format!(
                "o cabeçalho {name} não pode ser definido por quem pede"
            ));
            return self;
        }
        self.headers.push((lower, value.into()));
        self
    }

    pub fn accept(self, media_type: impl Into<String>) -> Self {
        self.header("accept", media_type)
    }

    /// Corpo (POST), com o tipo de conteúdo.
    pub fn body(mut self, content_type: impl Into<String>, bytes: Vec<u8>) -> Self {
        self.body = Some((content_type.into(), bytes));
        self
    }

    /// Pedido em nome de uma Conexão: o `Authorization` vem do Cofre, preso ao host.
    pub fn connection(mut self, connection: ConnectionId) -> Self {
        self.connection = Some(connection);
        self
    }

    /// Guarda a resposta 200 no cache por `ttl` (validade por tipo de pedido).
    pub fn cache_for(mut self, ttl: Duration) -> Self {
        self.cache_ttl = Some(ttl);
        self
    }

    /// Sem rede, não devolver o cache vencido (o padrão devolve, marcado `Stale`).
    pub fn no_stale(mut self) -> Self {
        self.allow_stale = false;
        self
    }

    /// Quanto o pedido aceita esperar por uma pausa pedida pelo Provedor (`429`). O padrão é
    /// zero: falha na hora com `RateLimited` e o estado "limitado até".
    pub fn patience(mut self, wait: Duration) -> Self {
        self.patience = wait;
        self
    }

    /// Prazo total do pedido (padrão da [`crate::NetConfig`]).
    pub fn timeout(mut self, timeout: Duration) -> Self {
        self.timeout = Some(timeout);
        self
    }

    /// Limite de bytes da resposta (padrão da [`crate::NetConfig`]).
    pub fn max_body(mut self, bytes: usize) -> Self {
        self.max_body = Some(bytes);
        self
    }

    /// Endereço validado: HTTPS, host e caminho bem formados, sem userinfo nem consulta crua.
    pub(crate) fn target(&self) -> Result<Target, NetError> {
        if let Some(reason) = &self.invalid {
            return Err(NetError::invalid(reason.clone()));
        }
        parse_https(&self.url).ok_or_else(|| NetError::EndpointNotAllowed {
            provider: self.provider,
            url: sanitized(&self.url),
        })
    }

    /// Consulta codificada, em ordem estável pela chave.
    pub(crate) fn encoded_query(&self) -> String {
        let mut pairs: Vec<&(String, String)> = self.query.iter().collect();
        pairs.sort_by(|a, b| a.0.cmp(&b.0));
        pairs
            .iter()
            .map(|(k, v)| format!("{}={}", encode(k), encode(v)))
            .collect::<Vec<_>>()
            .join("&")
    }

    /// URL oficial (sem segredo): a da Origem e a da chave do cache.
    pub(crate) fn canonical_url(&self, target: &Target) -> String {
        let query = self.encoded_query();
        let base = format!("https://{}{}", target.authority(), target.path);
        if query.is_empty() {
            base
        } else {
            format!("{base}?{query}")
        }
    }

    /// Chave do cache: método, URL normalizada, cabeçalhos, Conexão e resumo do corpo.
    pub(crate) fn cache_key(&self, target: &Target) -> String {
        let mut headers = self.headers.clone();
        headers.sort();
        let headers: Vec<String> = headers.iter().map(|(k, v)| format!("{k}:{v}")).collect();
        let body = self
            .body
            .as_ref()
            .map(|(t, b)| format!("{t}:{:016x}", fnv1a(b)))
            .unwrap_or_default();
        let connection = self.connection.map(|c| c.0.to_string()).unwrap_or_default();
        format!(
            "{} {}\nheaders={}\nconnection={connection}\nbody={body}",
            self.method.as_str(),
            self.canonical_url(target),
            headers.join(","),
        )
    }
}

/// Host, porta e caminho de uma URL HTTPS já validada.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Target {
    pub host: String,
    pub port: Option<u16>,
    pub path: String,
}

impl Target {
    pub fn authority(&self) -> String {
        match self.port {
            Some(port) => format!("{}:{port}", self.host),
            None => self.host.clone(),
        }
    }
}

/// Aceita só `https://host[:porta]/caminho`, com host em minúsculas de `[a-z0-9.-]`, sem
/// userinfo, consulta (vai por `query`) ou fragmento.
pub(crate) fn parse_https(url: &str) -> Option<Target> {
    let rest = url.strip_prefix("https://")?;
    let (authority, path) = match rest.find('/') {
        Some(i) => (&rest[..i], &rest[i..]),
        None => (rest, "/"),
    };
    let (host, port) = match authority.rsplit_once(':') {
        Some((h, p)) => (h, Some(p.parse::<u16>().ok()?)),
        None => (authority, None),
    };
    let host = host.to_ascii_lowercase();
    let host_ok = !host.is_empty()
        && host
            .bytes()
            .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'.' || b == b'-')
        && !host.starts_with('.')
        && !host.ends_with('.');
    let path_ok = path
        .bytes()
        .all(|b| b.is_ascii_alphanumeric() || b"-._~!$&'()*+,;=:@/%".contains(&b));
    (host_ok && path_ok).then(|| Target {
        host,
        port,
        path: path.to_owned(),
    })
}

/// URL para mensagens: sem consulta (que pode ter nomes de usuário) e cortada.
pub(crate) fn sanitized(url: &str) -> String {
    let base = url.split(['?', '#']).next().unwrap_or_default();
    base.chars().take(120).collect()
}

/// Percent-encoding de componente (RFC 3986): só os não reservados passam.
pub(crate) fn encode(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    for b in text.bytes() {
        if b.is_ascii_alphanumeric() || matches!(b, b'-' | b'.' | b'_' | b'~') {
            out.push(char::from(b));
        } else {
            out.push_str(&format!("%{b:02X}"));
        }
    }
    out
}

fn fnv1a(bytes: &[u8]) -> u64 {
    let mut hash: u64 = 0xcbf2_9ce4_8422_2325;
    for b in bytes {
        hash ^= u64::from(*b);
        hash = hash.wrapping_mul(0x0100_0000_01b3);
    }
    hash
}
