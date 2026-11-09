#![allow(clippy::unwrap_used)]
//! Comportamento externo do livro de aberturas.

use chess_core::CanonicalPosition;
use chess_openings::OpeningBook;

fn play(sans: &str) -> CanonicalPosition {
    let mut p = CanonicalPosition::start();
    for san in sans.split_whitespace().filter(|t| !t.ends_with('.')) {
        let mv = p.parse_san(san).unwrap();
        p = p.play(mv).unwrap();
    }
    p
}

fn name(book: &OpeningBook, sans: &str) -> Option<String> {
    book.lookup(&play(sans))
        .map(|o| format!("{} {}", o.eco, o.name))
}

#[test]
fn embedded_book_has_thousands_of_named_positions() {
    assert!(OpeningBook::embedded().len() > 3000);
}

#[test]
fn names_a_known_opening() {
    let book = OpeningBook::embedded();
    let o = book.lookup(&play("1. e4 e5 2. Nf3 Nc6 3. Bb5")).unwrap();
    assert_eq!(o.eco, "C60");
    assert!(o.name.starts_with("Ruy Lopez"), "{}", o.name);
    assert_eq!(o.plies, 5);
}

#[test]
fn start_position_has_no_name() {
    assert!(
        OpeningBook::embedded()
            .lookup(&CanonicalPosition::start())
            .is_none()
    );
}

#[test]
fn transposition_gets_the_same_name() {
    let book = OpeningBook::embedded();
    let a = name(book, "1. d4 Nf6 2. c4 e6 3. Nc3 Bb4");
    let b = name(book, "1. c4 e6 2. Nc3 Nf6 3. d4 Bb4");
    assert!(a.as_deref().is_some_and(|n| n.contains("Nimzo-Indian")));
    assert_eq!(a, b);
}

#[test]
fn clocks_do_not_matter() {
    let book = OpeningBook::embedded();
    let fen = "r1bqkbnr/pppp1ppp/2n5/1B2p3/4P3/5N2/PPPP1PPP/RNBQK2R b KQkq - 99 77";
    let p = CanonicalPosition::from_fen(fen).unwrap();
    assert_eq!(book.lookup(&p).unwrap().eco, "C60");
}

#[test]
fn longest_line_wins_and_ties_keep_the_first() {
    // Mesma Posição por transposição e mesmo tamanho: a primeira do arquivo fica.
    let tsv = "eco\tname\tpgn\nA00\tPrimeira\t1. e4 e5 2. Nf3\nB00\tSegunda\t1. Nf3 e5 2. e4\n";
    let book = OpeningBook::from_tsv([tsv]);
    assert_eq!(book.len(), 1);
    assert_eq!(
        book.lookup(&play("1. e4 e5 2. Nf3")).unwrap().name,
        "Primeira"
    );
    // Linha mais longa (com lances de ida e volta) sobre a mesma Posição vence a curta.
    let tsv = "A00\tCurta\t1. Nf3 Nf6\nB00\tLonga\t1. Nf3 Nf6 2. Ng1 Ng8 3. Nf3 Nf6\n";
    let book = OpeningBook::from_tsv([tsv]);
    assert_eq!(book.len(), 1);
    assert_eq!(book.lookup(&play("1. Nf3 Nf6")).unwrap().name, "Longa");
}

#[test]
fn malformed_and_illegal_lines_are_skipped() {
    let tsv = "eco\tname\tpgn\nZ\nA00\tRuim\t1. e5\nA00\tBoa\t1. e4\n";
    let book = OpeningBook::from_tsv([tsv]);
    assert_eq!(book.len(), 1);
    assert_eq!(name(&book, "1. e4").as_deref(), Some("A00 Boa"));
}

#[test]
fn along_keeps_the_last_named_position_of_the_line() {
    let book = OpeningBook::embedded();
    let line = [
        CanonicalPosition::start(),
        play("1. e4"),
        play("1. e4 e5"),
        play("1. e4 e5 2. Nf3"),
        play("1. e4 e5 2. Nf3 a6"),
        play("1. e4 e5 2. Nf3 a6 3. h3 h6 4. a3 a5 5. b3 b5"),
    ];
    let last_named = book.lookup(&line[4]).or_else(|| book.lookup(&line[3]));
    assert_eq!(book.along(line.iter()), last_named);
    assert!(book.along([CanonicalPosition::start()].iter()).is_none());
}
