#![allow(clippy::unwrap_used)]
//! Leitura em fluxo (CX-4, #216): partidas uma a uma, sem carregar o arquivo inteiro,
//! com Latin-1/Windows-1252 convertido e arquivo hostil sem pânico.

use chess_pgn::{PgnStream, StreamItem, decode_text};
use std::io::{BufReader, Cursor, Read};

fn collect(bytes: &[u8]) -> (Vec<chess_pgn::PgnGame>, Vec<chess_pgn::PgnError>, usize) {
    let mut stream = PgnStream::new(BufReader::new(Cursor::new(bytes.to_vec())));
    let (mut games, mut errors) = (Vec::new(), Vec::new());
    for item in stream.by_ref() {
        match item.unwrap() {
            StreamItem::Game(g) => games.push(*g),
            StreamItem::Error(e) => errors.push(e),
        }
    }
    (games, errors, stream.converted_games())
}

const TWO: &str = "[Event \"A\"]\n[White \"Ana\"]\n[Black \"Beto\"]\n\n1. e4 e5 2. Nf3 *\n\n\
[Event \"B\"]\n[White \"Carla\"]\n[Black \"Davi\"]\n\n1. d4 d5 1-0\n";

#[test]
fn reads_games_one_by_one() {
    let (games, errors, converted) = collect(TWO.as_bytes());
    assert_eq!(games.len(), 2);
    assert!(errors.is_empty());
    assert_eq!(converted, 0);
    assert_eq!(games[1].tag("Event"), Some("B"));
}

#[test]
fn a_bad_game_is_reported_with_the_file_line_and_the_next_one_survives() {
    let text = "[Event \"A\"]\n\n1. e4 e5 *\n\n[Event \"Ruim\"]\n\n1. e4 e5 2. Ke7 *\n\n\
[Event \"C\"]\n\n1. c4 *\n";
    let (games, errors, _) = collect(text.as_bytes());
    assert_eq!(games.len(), 2);
    assert_eq!(errors.len(), 1);
    assert_eq!(errors[0].game, 2);
    assert_eq!(errors[0].line, 7, "linha do arquivo, não do trecho da partida");
}

#[test]
fn latin1_is_converted_not_replaced() {
    // "José" em Latin-1: 0xE9 isolado é UTF-8 inválido.
    let mut bytes = b"[Event \"Copa\"]\n[White \"Jos".to_vec();
    bytes.push(0xE9);
    bytes.extend_from_slice(b"\"]\n[Black \"M");
    bytes.push(0xFC); // ü
    bytes.extend_from_slice(b"ller\"]\n\n1. e4 *\n");
    let (games, _, converted) = collect(&bytes);
    assert_eq!(games[0].tag("White"), Some("José"));
    assert_eq!(games[0].tag("Black"), Some("Müller"));
    assert_eq!(converted, 1);
}

#[test]
fn windows_1252_specials_are_mapped() {
    let decoded = decode_text(&[b'a', 0x80, 0x93, 0x94, 0x96, b'z']);
    assert!(decoded.converted);
    assert_eq!(decoded.text, "a€“”–z");
    let utf8 = decode_text("José".as_bytes());
    assert!(!utf8.converted);
    assert_eq!(utf8.text, "José");
}

#[test]
fn hostile_input_does_not_panic_or_stall() {
    // Linha gigante, tags repetidas, NUL, parênteses desbalanceados, lixo binário.
    let mut bytes = Vec::new();
    bytes.extend_from_slice(b"[Event \"x\"]\n[Event \"y\"]\n[Event \"z\"]\n\n1. e4 e5 *\n\n");
    bytes.extend_from_slice(b"[Event \"gigante\"]\n\n1. e4 {");
    bytes.extend(std::iter::repeat_n(b'a', 3 * 1024 * 1024));
    bytes.extend_from_slice(b"} e5 *\n\n");
    bytes.extend_from_slice(b"[Event \"nul\"]\n\n1. e4\0 e5 (((( *\n\n");
    bytes.extend(std::iter::repeat_n(0xFFu8, 1000));
    bytes.extend_from_slice(b"\n\n[Event \"ok\"]\n\n1. d4 *\n");
    let (games, errors, _) = collect(&bytes);
    assert!(games.iter().any(|g| g.tag("Event") == Some("ok")));
    assert!(games.iter().any(|g| g.tag("Event") == Some("x")));
    assert!(!errors.is_empty());
}

/// Gera partidas sob demanda: nunca existe o arquivo inteiro em lugar nenhum.
struct Synthetic {
    left: usize,
    next: usize,
    buf: Vec<u8>,
    pos: usize,
}

impl Read for Synthetic {
    fn read(&mut self, out: &mut [u8]) -> std::io::Result<usize> {
        if self.pos >= self.buf.len() {
            if self.left == 0 {
                return Ok(0);
            }
            self.left -= 1;
            self.next += 1;
            self.buf = format!(
                "[Event \"G{0}\"]\n[White \"W{0}\"]\n[Black \"B{0}\"]\n\n1. e4 e5 2. Nf3 Nc6 *\n\n",
                self.next
            )
            .into_bytes();
            self.pos = 0;
        }
        let n = out.len().min(self.buf.len() - self.pos);
        out[..n].copy_from_slice(&self.buf[self.pos..self.pos + n]);
        self.pos += n;
        Ok(n)
    }
}

#[test]
fn streaming_many_games_keeps_the_buffer_small() {
    let src = Synthetic {
        left: 20_000,
        next: 0,
        buf: Vec::new(),
        pos: 0,
    };
    let mut stream = PgnStream::new(BufReader::new(src));
    let mut count = 0;
    for item in stream.by_ref() {
        assert!(matches!(item.unwrap(), StreamItem::Game(_)));
        count += 1;
    }
    assert_eq!(count, 20_000);
    assert!(
        stream.peak_buffered_bytes() < 4096,
        "pico de {} bytes",
        stream.peak_buffered_bytes()
    );
}
