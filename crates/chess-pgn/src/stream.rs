//! Leitura de PGN em fluxo (CX-4, #216): divide o arquivo em partidas lendo linha a linha,
//! sem nunca guardar mais que uma partida na memória, decodifica UTF-8 ou Windows-1252
//! (Latin-1) e entrega cada partida pelo mesmo parser tolerante. Entrada hostil (linha
//! gigante, partida gigante, lixo binário) vira erro tipado, nunca pânico nem travamento.

use crate::{PgnError, PgnErrorKind, PgnGame, parse_pgn_lenient};
use std::collections::VecDeque;
use std::io::{self, BufRead};

/// Maior linha aceita; o excedente é descartado e a partida vira erro.
pub const MAX_LINE_BYTES: usize = 1 << 20;
/// Maior partida aceita (texto bruto).
pub const MAX_GAME_BYTES: usize = 16 << 20;

/// Texto decodificado e se houve conversão de Windows-1252/Latin-1.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Decoded {
    pub text: String,
    /// `true` quando os bytes não eram UTF-8 válido e foram lidos como Windows-1252.
    pub converted: bool,
}

const CP1252_HIGH: [char; 32] = [
    '€', '\u{81}', '‚', 'ƒ', '„', '…', '†', '‡', 'ˆ', '‰', 'Š', '‹', 'Œ', '\u{8d}', 'Ž', '\u{8f}',
    '\u{90}', '‘', '’', '“', '”', '•', '–', '—', '˜', '™', 'š', '›', 'œ', '\u{9d}', 'ž', 'Ÿ',
];

/// UTF-8 quando válido; senão Windows-1252 (superconjunto prático do Latin-1).
pub fn decode_text(bytes: &[u8]) -> Decoded {
    match std::str::from_utf8(bytes) {
        Ok(text) => Decoded {
            text: text.to_owned(),
            converted: false,
        },
        Err(_) => Decoded {
            text: bytes
                .iter()
                .map(|&b| match b {
                    0x80..=0x9F => CP1252_HIGH[usize::from(b - 0x80)],
                    _ => char::from(b),
                })
                .collect(),
            converted: true,
        },
    }
}

/// Um item do fluxo: uma partida lida ou o erro de uma partida pulada.
#[derive(Debug)]
pub enum StreamItem {
    Game(Box<PgnGame>),
    Error(PgnError),
}

/// Iterador de partidas sobre qualquer `BufRead` (arquivo, `.gz` já descompactado...).
pub struct PgnStream<R: BufRead> {
    reader: R,
    pending: Option<(Vec<u8>, bool)>,
    queue: VecDeque<StreamItem>,
    /// Linhas já consumidas do arquivo.
    lines_read: u32,
    game_base: usize,
    converted: usize,
    peak: usize,
    done: bool,
}

#[derive(Default)]
struct RawChunk {
    bytes: Vec<u8>,
    first_line: u32,
    line_too_long: bool,
    too_large: bool,
}

impl<R: BufRead> PgnStream<R> {
    pub fn new(reader: R) -> Self {
        Self {
            reader,
            pending: None,
            queue: VecDeque::new(),
            lines_read: 0,
            game_base: 0,
            converted: 0,
            peak: 0,
            done: false,
        }
    }

    /// Partidas lidas que não eram UTF-8 e foram convertidas de Windows-1252.
    pub fn converted_games(&self) -> usize {
        self.converted
    }

    /// Maior quantidade de bytes guardada de uma só vez (prova a leitura em fluxo).
    pub fn peak_buffered_bytes(&self) -> usize {
        self.peak
    }

    /// Lê uma linha guardando no máximo [`MAX_LINE_BYTES`]. `Some(truncada)` ou `None` no fim.
    fn read_line(&mut self, out: &mut Vec<u8>) -> io::Result<Option<bool>> {
        out.clear();
        let mut truncated = false;
        let mut any = false;
        loop {
            let (used, found) = {
                let buf = self.reader.fill_buf()?;
                if buf.is_empty() {
                    if any {
                        self.lines_read = self.lines_read.saturating_add(1);
                    }
                    return Ok(any.then_some(truncated));
                }
                any = true;
                let end = buf.iter().position(|&b| b == b'\n');
                let take = end.map_or(buf.len(), |i| i + 1);
                let copy = take.min(MAX_LINE_BYTES.saturating_sub(out.len()));
                if copy < take {
                    truncated = true;
                }
                out.extend_from_slice(&buf[..copy]);
                (take, end.is_some())
            };
            self.reader.consume(used);
            if found {
                self.lines_read = self.lines_read.saturating_add(1);
                return Ok(Some(truncated));
            }
        }
    }

    /// Próxima partida em bruto. Uma tag de cabeçalho `[Nome "valor"]` fora de comentário,
    /// depois de já haver Lances, começa a partida seguinte. O estado léxico (comentário
    /// `{ }` que atravessa linhas, comentário `;`, linha de escape `%`) é rastreado linha a
    /// linha, então um `[` no começo de uma linha de comentário (`[%clk ...]`) nunca divide
    /// a Partida. Estourado o limite de tamanho (por exemplo, `{` nunca fechado), descarta
    /// o que leu e ressincroniza na próxima tag de cabeçalho depois de uma linha em branco.
    fn next_chunk(&mut self) -> io::Result<Option<RawChunk>> {
        let mut chunk = RawChunk::default();
        let mut buffer = Vec::new();
        let mut in_movetext = false;
        let mut in_brace = false;
        let mut resync = false;
        let mut prev_blank = false;
        loop {
            let (bytes, truncated) = if let Some(p) = self.pending.take() {
                p
            } else {
                match self.read_line(&mut buffer)? {
                    Some(t) => (std::mem::take(&mut buffer), t),
                    None => break,
                }
            };
            let first = bytes.iter().find(|b| !b.is_ascii_whitespace()).copied();
            let blank = first.is_none();
            let header = !in_brace && is_tag_line(&bytes);
            if resync {
                if header && prev_blank {
                    self.pending = Some((bytes, truncated));
                    break;
                }
                prev_blank = blank;
                continue;
            }
            if header && in_movetext {
                self.pending = Some((bytes, truncated));
                break;
            }
            if !header && !(bytes.first() == Some(&b'%') && !in_brace) && !blank {
                in_movetext = true;
                scan_comments(&bytes, &mut in_brace);
            } else if in_brace && !blank {
                scan_comments(&bytes, &mut in_brace);
            }
            prev_blank = blank;
            if chunk.bytes.is_empty() && !chunk.too_large {
                if blank {
                    continue; // brancos antes da partida
                }
                chunk.first_line = self.lines_read;
            }
            chunk.line_too_long |= truncated;
            if truncated {
                // O resto da linha foi descartado: o estado léxico é desconhecido. A Partida
                // vira erro e a leitura ressincroniza na próxima tag de cabeçalho.
                chunk.bytes = Vec::new();
                resync = true;
                in_brace = false;
                continue;
            }
            if chunk.bytes.len() + bytes.len() > MAX_GAME_BYTES {
                chunk.too_large = true;
                chunk.bytes = Vec::new();
                resync = true;
                in_brace = false;
            } else {
                chunk.bytes.extend_from_slice(&bytes);
            }
            self.peak = self.peak.max(chunk.bytes.len());
        }
        Ok((!chunk.bytes.is_empty() || chunk.too_large || chunk.line_too_long).then_some(chunk))
    }

    fn fill(&mut self) -> io::Result<()> {
        while self.queue.is_empty() && !self.done {
            let Some(chunk) = self.next_chunk()? else {
                self.done = true;
                break;
            };
            let first_line = chunk.first_line.max(1);
            if chunk.too_large || chunk.line_too_long {
                self.game_base += 1;
                let kind = if chunk.too_large {
                    PgnErrorKind::GameTooLarge
                } else {
                    PgnErrorKind::LineTooLong
                };
                self.queue.push_back(StreamItem::Error(PgnError {
                    game: self.game_base,
                    line: first_line,
                    column: 1,
                    kind,
                }));
                continue;
            }
            let decoded = decode_text(&chunk.bytes);
            let report = parse_pgn_lenient(&decoded.text);
            let count = report.games.len() + report.errors.len();
            if decoded.converted && count > 0 {
                self.converted += 1;
            }
            let base = self.game_base;
            self.game_base += count;
            for game in report.games {
                self.queue.push_back(StreamItem::Game(Box::new(game)));
            }
            for mut e in report.errors {
                e.game += base;
                e.line += first_line - 1;
                self.queue.push_back(StreamItem::Error(e));
            }
        }
        Ok(())
    }
}

impl<R: BufRead> std::fmt::Debug for PgnStream<R> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("PgnStream")
            .field("lines_read", &self.lines_read)
            .field("games", &self.game_base)
            .finish_non_exhaustive()
    }
}

impl<R: BufRead> Iterator for PgnStream<R> {
    /// Erro de E/S (leitura do arquivo); os erros de PGN vêm como [`StreamItem::Error`].
    type Item = io::Result<StreamItem>;

    fn next(&mut self) -> Option<Self::Item> {
        if let Err(e) = self.fill() {
            self.done = true;
            self.queue.clear();
            return Some(Err(e));
        }
        self.queue.pop_front().map(Ok)
    }
}

/// `[Nome "valor"]` no começo da linha (espaços antes permitidos): a forma de uma tag de
/// cabeçalho. `[%clk 0:05:00]` e texto solto não são.
fn is_tag_line(line: &[u8]) -> bool {
    let line = line.trim_ascii();
    let Some(rest) = line.strip_prefix(b"[") else {
        return false;
    };
    let name = rest
        .iter()
        .take_while(|b| b.is_ascii_alphanumeric() || **b == b'_')
        .count();
    if name == 0 || !line.ends_with(b"]") {
        return false;
    }
    let after = rest[name..].trim_ascii_start();
    rest.len() > name && after.first() == Some(&b'"')
}

/// Atualiza se o fim da linha está dentro de um comentário `{ }` (que não aninha e pode
/// atravessar linhas). `;` abre comentário até o fim da linha; `%` na coluna 1 é escape.
fn scan_comments(line: &[u8], in_brace: &mut bool) {
    if !*in_brace && line.first() == Some(&b'%') {
        return;
    }
    for &b in line {
        match (*in_brace, b) {
            (true, b'}') => *in_brace = false,
            (false, b'{') => *in_brace = true,
            (false, b';') => return,
            _ => {}
        }
    }
}
