#![allow(clippy::unwrap_used)]
//! Fronteira de Partida no fluxo: o estado léxico (chaves, `;`, `%`, variações) atravessa
//! linhas e um `[` no começo de uma linha de comentário nunca divide a Partida.

use chess_pgn::{PgnErrorKind, PgnStream, StreamItem};
use std::io::{BufReader, Cursor};

fn collect(bytes: &[u8]) -> (Vec<chess_pgn::PgnGame>, Vec<chess_pgn::PgnError>) {
    let stream = PgnStream::new(BufReader::new(Cursor::new(bytes.to_vec())));
    let (mut games, mut errors) = (Vec::new(), Vec::new());
    for item in stream {
        match item.unwrap() {
            StreamItem::Game(g) => games.push(*g),
            StreamItem::Error(e) => errors.push(e),
        }
    }
    (games, errors)
}

const HEAD_A: &str = "[Event \"A\"]\n[White \"Ana\"]\n[Black \"Beto\"]\n\n";
const NEXT: &str = "\n[Event \"Seguinte\"]\n[White \"C\"]\n[Black \"D\"]\n\n1. d4 d5 *\n";

#[test]
fn multiline_comments_with_embedded_commands_do_not_split_the_game() {
    let text = format!(
        "{HEAD_A}1. e4 {{ comentario longo\n[%clk 0:05:00]\n[%eval 0.3]\n[%cal Gd2d4]\n[%csl Rd4] fim }} e5\n\
         2. Nf3 {{ [%clk 0:04:58] }} Nc6 *\n{NEXT}"
    );
    let (games, errors) = collect(text.as_bytes());
    assert!(errors.is_empty(), "{errors:?}");
    assert_eq!(games.len(), 2);
    assert_eq!(games[0].mainline_sans(), ["e4", "e5", "Nf3", "Nc6"]);
}

#[test]
fn a_fake_header_inside_braces_is_text() {
    let text =
        format!("{HEAD_A}1. e4 {{ veja\n[Event \"falso\"]\n[White \"Fulano\"]\n}} e5 *\n{NEXT}");
    let (games, errors) = collect(text.as_bytes());
    assert!(errors.is_empty(), "{errors:?}");
    assert_eq!(games.len(), 2);
    assert_eq!(games[0].tag("Event"), Some("A"));
    assert_eq!(games[1].tag("Event"), Some("Seguinte"));
}

#[test]
fn semicolon_comment_and_percent_escape_lines_are_not_boundaries() {
    let text = format!("{HEAD_A}1. e4 ; [Event \"x\"] so comentario\n%[Event \"y\"]\ne5 *\n{NEXT}");
    let (games, errors) = collect(text.as_bytes());
    assert!(errors.is_empty(), "{errors:?}");
    assert_eq!(games.len(), 2);
}

#[test]
fn nested_variations_across_lines_stay_in_one_game() {
    let text = format!(
        "{HEAD_A}1. e4 e5 (1... c5 2. Nf3 (2. c3\n{{ [%cal Gd2d4] }}\nd5) 2... d6)\n2. Nf3 *\n{NEXT}"
    );
    let (games, errors) = collect(text.as_bytes());
    assert!(errors.is_empty(), "{errors:?}");
    assert_eq!(games.len(), 2);
    assert_eq!(games[0].variation_count(), 2);
}

#[test]
fn an_unclosed_brace_hits_the_size_limit_and_the_next_game_is_recovered() {
    let mut bytes = HEAD_A.as_bytes().to_vec();
    bytes.extend_from_slice(b"1. e4 { nunca fecha\n");
    let filler = vec![b'x'; 900_000];
    for _ in 0..20 {
        bytes.extend_from_slice(&filler);
        bytes.push(b'\n');
    }
    bytes.extend_from_slice(NEXT.as_bytes());
    let (games, errors) = collect(&bytes);
    assert_eq!(errors.len(), 1);
    assert_eq!(errors[0].kind, PgnErrorKind::GameTooLarge);
    assert_eq!(games.len(), 1, "a Partida seguinte é recuperada");
    assert_eq!(games[0].tag("Event"), Some("Seguinte"));
}
