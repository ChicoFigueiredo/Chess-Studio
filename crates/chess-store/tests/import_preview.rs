#![allow(clippy::unwrap_used)]
//! #270: prévia da importação de PGN (lê só o começo), detecção da fonte pelas tags e licença
//! restrita.

mod common;

use chess_store::{Estimate, ImportSource, preview_pgn_file};
use common::TempDb;
use std::io::Write;

fn lichess_game(n: usize) -> String {
    format!(
        "[Event \"Rated Blitz game\"]\n[Site \"https://lichess.org/abcd{n:04}\"]\n[Date \"2026.01.01\"]\n[White \"alice\"]\n[Black \"bob{n}\"]\n[Result \"1-0\"]\n[LichessURL \"https://lichess.org/abcd{n:04}\"]\n\n1. e4 e5 2. Nf3 Nc6 3. Bb5 a6 1-0\n\n"
    )
}

fn plain_game(n: usize) -> String {
    format!(
        "[Event \"Torneio {n}\"]\n[Site \"?\"]\n[Date \"2020.01.01\"]\n[White \"A{n}\"]\n[Black \"B{n}\"]\n[Result \"1/2-1/2\"]\n\n1. d4 d5 2. c4 e6 1/2-1/2\n\n"
    )
}

fn write(db: &TempDb, name: &str, text: &str) -> std::path::PathBuf {
    let path = db.dir().join(name);
    std::fs::write(&path, text).unwrap();
    path
}

#[test]
fn a_small_file_is_counted_exactly_and_its_source_is_detected_from_the_tags() {
    let db = TempDb::new("prev-small");
    let text: String = (0..3).map(lichess_game).collect();
    let preview = preview_pgn_file(&write(&db, "a.pgn", &text), 64 * 1024).unwrap();
    assert_eq!(preview.estimate, Estimate::Exact(3));
    assert_eq!(preview.detected, Some(ImportSource::Lichess));
    assert_eq!(preview.games.len(), 3);
    assert_eq!(preview.games[0].white, "alice");
    assert_eq!(preview.games[0].black, "bob0");
    assert_eq!(preview.games[0].event, "Rated Blitz game");
}

#[test]
fn a_big_file_is_only_read_at_the_start_and_the_total_is_an_estimate() {
    let db = TempDb::new("prev-big");
    let text: String = (0..2000).map(plain_game).collect();
    let path = write(&db, "big.pgn", &text);
    let preview = preview_pgn_file(&path, 8 * 1024).unwrap();
    assert!(preview.bytes_read <= 8 * 1024, "{}", preview.bytes_read);
    let Estimate::About(n) = preview.estimate else {
        panic!("{:?}", preview.estimate)
    };
    assert!((1800..=2200).contains(&n), "estimativa {n}");
    assert_eq!(preview.detected, None);
    assert!(preview.games.len() <= 8, "a lista é curta");
}

#[test]
fn a_compressed_file_says_at_least_what_it_saw() {
    let db = TempDb::new("prev-gz");
    let text: String = (0..2000).map(lichess_game).collect();
    let mut enc = flate2::write::GzEncoder::new(Vec::new(), flate2::Compression::default());
    enc.write_all(text.as_bytes()).unwrap();
    let path = db.dir().join("big.pgn.gz");
    std::fs::write(&path, enc.finish().unwrap()).unwrap();
    let preview = preview_pgn_file(&path, 8 * 1024).unwrap();
    assert!(matches!(preview.estimate, Estimate::AtLeast(n) if n > 10));
    assert_eq!(preview.detected, Some(ImportSource::Lichess));
}

#[test]
fn a_missing_file_is_an_error_and_nothing_is_written() {
    let db = TempDb::new("prev-missing");
    assert!(preview_pgn_file(&db.dir().join("nao-existe.pgn"), 1024).is_err());
}

#[test]
fn the_source_is_detected_from_site_and_link_tags() {
    use chess_pgn::parse_pgn;
    let detect = |tags: &str| {
        let games = parse_pgn(&format!("{tags}\n\n1. e4 *\n")).unwrap();
        ImportSource::detect(&games[0])
    };
    assert_eq!(
        detect("[Site \"https://lichess.org/AbCdEfGh\"]"),
        Some(ImportSource::Lichess)
    );
    assert_eq!(
        detect("[Site \"?\"]\n[LichessURL \"https://lichess.org/AbCdEfGh\"]"),
        Some(ImportSource::Lichess)
    );
    assert_eq!(
        detect("[Site \"Chess.com\"]\n[Link \"https://www.chess.com/game/live/123\"]"),
        Some(ImportSource::ChessCom)
    );
    assert_eq!(
        detect("[Site \"https://playstrategy.org/AbCdEfGh\"]"),
        Some(ImportSource::PlayStrategy)
    );
    assert_eq!(detect("[Site \"Wijk aan Zee\"]"), None);
}

#[test]
fn only_twic_pgn_mentor_and_chessbase_are_restricted() {
    let restricted: Vec<_> = ImportSource::ALL
        .into_iter()
        .filter(|s| s.is_restricted())
        .collect();
    assert_eq!(
        restricted,
        vec![
            ImportSource::ChessBase,
            ImportSource::Twic,
            ImportSource::PgnMentor
        ]
    );
}
