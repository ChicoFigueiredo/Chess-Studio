//! Respostas gravadas: a saída do `curl -i` (linha de status, cabeçalhos, linha em branco,
//! corpo), com comentários `#` no topo dizendo de onde e quando veio. Para gravar uma
//! resposta real, fora da suíte (CX-17):
//!
//! ```text
//! curl -sS -i 'https://explorer.lichess.org/masters?play=e2e4' > tests/fixtures/masters.http
//! ```
//!
//! Os cabeçalhos de enquadramento (`Content-Length`, `Transfer-Encoding`, `Connection`) são
//! descartados: o servidor falso recalcula ao servir. Não grave respostas com dados
//! privados nem cabeçalhos `Set-Cookie`.

use super::server::FakeResponse;

/// Cabeçalhos que dependem de como a resposta trafegou, não do conteúdo.
const FRAMING: [&str; 5] = [
    "content-length",
    "transfer-encoding",
    "connection",
    "keep-alive",
    "set-cookie",
];

fn split_line(text: &str) -> (&str, &str) {
    match text.find('\n') {
        Some(i) => (text[..i].trim_end_matches('\r'), &text[i + 1..]),
        None => (text.trim_end_matches('\r'), ""),
    }
}

pub(super) fn parse(text: &str) -> Result<FakeResponse, String> {
    let mut rest = text;
    let status_line = loop {
        if rest.is_empty() {
            return Err("sem linha de status HTTP".to_owned());
        }
        let (line, after) = split_line(rest);
        rest = after;
        if line.trim().is_empty() || line.starts_with('#') {
            continue;
        }
        break line;
    };
    if !status_line.starts_with("HTTP/") {
        return Err(format!("linha de status inválida: {status_line:?}"));
    }
    let status: u16 = status_line
        .split_whitespace()
        .nth(1)
        .and_then(|code| code.parse().ok())
        .ok_or_else(|| format!("status inválido: {status_line:?}"))?;
    let mut response = FakeResponse::status(status);
    while !rest.is_empty() {
        let (line, after) = split_line(rest);
        rest = after;
        if line.is_empty() {
            break;
        }
        let (name, value) = line
            .split_once(':')
            .ok_or_else(|| format!("cabeçalho sem ':': {line:?}"))?;
        let name = name.trim().to_ascii_lowercase();
        if !FRAMING.contains(&name.as_str()) {
            response = response.header(&name, value.trim());
        }
    }
    Ok(response.with_body(rest.as_bytes().to_vec()))
}
