//! Prévia de uma importação de PGN (#270, especificação §6.5): lê só o começo do arquivo,
//! sem carregá-lo inteiro e sem gravar nada, e diz quantas Partidas há (estimativa), os
//! cabeçalhos das primeiras e a fonte sugerida pelas tags.

use crate::error::{Result, StoreError};
use crate::origin::ImportSource;
use chess_pgn::{PgnGame, PgnStream, StreamItem};
use std::io::{BufRead, BufReader, Read};
use std::path::Path;

/// Quantas Partidas aparecem na lista da prévia.
pub const PREVIEW_LIST: usize = 5;

/// Bytes lidos do começo por padrão (o arquivo não é carregado inteiro).
pub const DEFAULT_PREVIEW_BYTES: u64 = 256 * 1024;

/// Quantas Partidas o arquivo tem.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Estimate {
    /// O arquivo coube na janela lida: a contagem é exata.
    Exact(usize),
    /// Estimativa pela proporção do arquivo lido.
    About(usize),
    /// Arquivo comprimido maior que a janela: pelo menos estas.
    AtLeast(usize),
}

/// Cabeçalho de uma Partida da prévia.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PreviewGame {
    pub white: String,
    pub black: String,
    pub event: String,
    pub date: String,
    pub result: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ImportPreview {
    pub estimate: Estimate,
    pub games: Vec<PreviewGame>,
    /// Fonte sugerida pelas tags (`Site`, `LichessURL`, `Link`…), quando a maioria concorda.
    pub detected: Option<ImportSource>,
    /// Partidas da janela que não puderam ser lidas.
    pub unreadable: usize,
    /// Bytes (descomprimidos) lidos do começo do arquivo.
    pub bytes_read: u64,
    pub compressed: bool,
}

fn tag(game: &PgnGame, name: &str) -> String {
    game.tag(name)
        .filter(|v| !v.is_empty())
        .unwrap_or("?")
        .to_owned()
}

/// Lê no máximo `max_bytes` do começo de `path` (`.pgn` ou `.pgn.gz`, pelos bytes iniciais).
pub fn preview_pgn_file(path: &Path, max_bytes: u64) -> Result<ImportPreview> {
    let io = |e: std::io::Error| StoreError::Io(e.to_string());
    let file = std::fs::File::open(path).map_err(io)?;
    let total = file.metadata().map_err(io)?.len();
    let mut raw = BufReader::new(file);
    let compressed = raw.fill_buf().map_err(io)?.starts_with(&[0x1f, 0x8b]);
    let window: Box<dyn Read> = if compressed {
        Box::new(flate2::bufread::MultiGzDecoder::new(raw))
    } else {
        Box::new(raw)
    };
    let counted = std::rc::Rc::new(std::cell::Cell::new(0u64));
    let mut stream = PgnStream::new(BufReader::new(Counting {
        inner: window.take(max_bytes),
        count: counted.clone(),
    }));
    let mut parsed: Vec<PgnGame> = Vec::new();
    let mut unreadable = 0;
    for item in stream.by_ref() {
        match item.map_err(io)? {
            StreamItem::Game(game) => parsed.push(*game),
            StreamItem::Error(_) => unreadable += 1,
        }
    }
    let bytes_read = counted.get();
    // Chegou ao limite: o arquivo pode ter mais, e a última Partida da janela está cortada.
    let window_hit = bytes_read >= max_bytes;
    if window_hit {
        parsed.pop();
    }
    let seen = parsed.len();
    let estimate = if !window_hit {
        Estimate::Exact(seen)
    } else if compressed || seen == 0 {
        Estimate::AtLeast(seen)
    } else {
        // Proporção: bytes por Partida na janela, aplicada ao tamanho do arquivo.
        let per_game = bytes_read as f64 / (seen as f64 + 1.0);
        Estimate::About(((total as f64 / per_game).round() as usize).max(seen))
    };
    Ok(ImportPreview {
        estimate,
        games: parsed
            .iter()
            .take(PREVIEW_LIST)
            .map(|g| PreviewGame {
                white: tag(g, "White"),
                black: tag(g, "Black"),
                event: tag(g, "Event"),
                date: tag(g, "Date"),
                result: tag(g, "Result"),
            })
            .collect(),
        detected: detect_majority(&parsed),
        unreadable,
        bytes_read,
        compressed,
    })
}

/// A fonte que a maioria das Partidas indica, se mais da metade a indica.
fn detect_majority(games: &[PgnGame]) -> Option<ImportSource> {
    let mut votes: Vec<(ImportSource, usize)> = Vec::new();
    for source in games.iter().filter_map(ImportSource::detect) {
        match votes.iter_mut().find(|(s, _)| *s == source) {
            Some((_, n)) => *n += 1,
            None => votes.push((source, 1)),
        }
    }
    let (source, n) = votes.into_iter().max_by_key(|(_, n)| *n)?;
    (n * 2 > games.len()).then_some(source)
}

struct Counting<R> {
    inner: R,
    count: std::rc::Rc<std::cell::Cell<u64>>,
}

impl<R: Read> Read for Counting<R> {
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        let n = self.inner.read(buf)?;
        self.count.set(self.count.get() + n as u64);
        Ok(n)
    }
}
