//! Leitor NDJSON incremental para os fluxos dos Provedores (eventos e Partidas do Lichess):
//! junta linhas partidas entre leituras, entrega linhas vazias como heartbeat, recusa linha
//! acima do máximo (e se ressincroniza na seguinte) e distingue fim limpo de fluxo cortado.
//! Não interpreta o JSON: cada adaptador lê as suas mensagens.

use std::io::{ErrorKind, Read};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum NdjsonItem {
    /// Uma mensagem (sem o `\n` nem o `\r` final).
    Line(String),
    /// Linha vazia: o Provedor só avisou que o fluxo segue vivo.
    Heartbeat,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum NdjsonError {
    /// A linha passou de `limit` bytes; ela é descartada até o próximo `\n`.
    LineTooLong { limit: usize },
    /// A linha não é UTF-8; descartada.
    InvalidUtf8,
    /// O fluxo acabou no meio de uma linha (`bytes` sem `\n`).
    Truncated { bytes: usize },
    /// Erro de leitura (tempo esgotado, conexão caída).
    Io(ErrorKind),
}

impl std::fmt::Display for NdjsonError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::LineTooLong { limit } => write!(f, "linha NDJSON acima de {limit} bytes"),
            Self::InvalidUtf8 => f.write_str("linha NDJSON fora de UTF-8"),
            Self::Truncated { bytes } => {
                write!(f, "fluxo NDJSON cortado ({bytes} bytes sem fim de linha)")
            }
            Self::Io(kind) => write!(f, "erro de leitura do fluxo: {kind}"),
        }
    }
}

impl std::error::Error for NdjsonError {}

/// Leitor incremental sobre qualquer `Read` (o corpo HTTP, um arquivo, um teste).
#[derive(Debug)]
pub struct NdjsonReader<R> {
    inner: R,
    max_line: usize,
    buf: Vec<u8>,
    /// Bytes de `buf` já examinados sem achar `\n`.
    scanned: usize,
    /// Descartando o resto de uma linha longa demais.
    discarding: bool,
    eof: bool,
}

const READ_CHUNK: usize = 8 * 1024;

impl<R: Read> NdjsonReader<R> {
    pub fn new(inner: R, max_line: usize) -> Self {
        Self {
            inner,
            max_line: max_line.max(1),
            buf: Vec::new(),
            scanned: 0,
            discarding: false,
            eof: false,
        }
    }

    /// Próximo item; `Ok(None)` no fim limpo do fluxo (e daí em diante).
    pub fn next_item(&mut self) -> Result<Option<NdjsonItem>, NdjsonError> {
        loop {
            if let Some(i) = self.buf[self.scanned..].iter().position(|&b| b == b'\n') {
                let end = self.scanned + i;
                let mut line: Vec<u8> = self.buf.drain(..=end).collect();
                self.scanned = 0;
                line.pop();
                if line.last() == Some(&b'\r') {
                    line.pop();
                }
                if std::mem::take(&mut self.discarding) {
                    continue;
                }
                if line.len() > self.max_line {
                    return Err(NdjsonError::LineTooLong {
                        limit: self.max_line,
                    });
                }
                if line.iter().all(u8::is_ascii_whitespace) {
                    return Ok(Some(NdjsonItem::Heartbeat));
                }
                return String::from_utf8(line)
                    .map(|text| Some(NdjsonItem::Line(text)))
                    .map_err(|_| NdjsonError::InvalidUtf8);
            }
            self.scanned = self.buf.len();
            if !self.discarding && self.buf.len() > self.max_line {
                self.buf.clear();
                self.scanned = 0;
                self.discarding = true;
                return Err(NdjsonError::LineTooLong {
                    limit: self.max_line,
                });
            }
            if self.discarding {
                self.buf.clear();
                self.scanned = 0;
            }
            if self.eof {
                if self.buf.is_empty() || self.discarding {
                    self.buf.clear();
                    return Ok(None);
                }
                let bytes = self.buf.len();
                self.buf.clear();
                self.scanned = 0;
                return Err(NdjsonError::Truncated { bytes });
            }
            self.fill()?;
        }
    }

    fn fill(&mut self) -> Result<(), NdjsonError> {
        let mut chunk = [0u8; READ_CHUNK];
        loop {
            match self.inner.read(&mut chunk) {
                Ok(0) => {
                    self.eof = true;
                    return Ok(());
                }
                Ok(n) => {
                    self.buf.extend_from_slice(&chunk[..n]);
                    return Ok(());
                }
                Err(e) if e.kind() == ErrorKind::Interrupted => continue,
                Err(e) => return Err(NdjsonError::Io(e.kind())),
            }
        }
    }

    pub fn into_inner(self) -> R {
        self.inner
    }
}

impl<R: Read> Iterator for NdjsonReader<R> {
    type Item = Result<NdjsonItem, NdjsonError>;

    fn next(&mut self) -> Option<Self::Item> {
        self.next_item().transpose()
    }
}
