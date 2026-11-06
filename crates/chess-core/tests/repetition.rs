use chess_core::{CanonicalPosition, Move};
use std::collections::HashSet;

fn pos(fen: &str) -> CanonicalPosition {
    CanonicalPosition::from_fen(fen).expect("FEN válido")
}

fn play(p: &CanonicalPosition, moves: &[&str]) -> CanonicalPosition {
    moves.iter().fold(p.clone(), |acc, m| {
        acc.play(m.parse::<Move>().expect("UCI válido"))
            .expect("lance legal")
    })
}

#[test]
fn same_position_reached_again_has_equal_key_regardless_of_clocks() {
    let start = CanonicalPosition::start();
    let again = play(&start, &["g1f3", "g8f6", "f3g1", "f6g8"]);
    assert_ne!(start, again, "relógios e número do lance diferem");
    assert_eq!(start.repetition_key(), again.repetition_key());
}

#[test]
fn key_is_usable_in_hash_sets() {
    let start = CanonicalPosition::start();
    let mut set = HashSet::new();
    set.insert(start.repetition_key());
    let again = play(&start, &["g1f3", "g8f6", "f3g1", "f6g8"]);
    assert!(!set.insert(again.repetition_key()));
    assert!(set.insert(play(&start, &["e2e4"]).repetition_key()));
}

#[test]
fn side_to_move_is_part_of_the_key() {
    let w = pos("4k3/8/8/8/8/8/8/R3K3 w - - 0 1");
    let b = pos("4k3/8/8/8/8/8/8/R3K3 b - - 0 1");
    assert_ne!(w.repetition_key(), b.repetition_key());
}

#[test]
fn castling_rights_are_part_of_the_key() {
    let all = pos("r3k2r/8/8/8/8/8/8/R3K2R w KQkq - 0 1");
    let fewer = pos("r3k2r/8/8/8/8/8/8/R3K2R w Kkq - 0 1");
    assert_ne!(all.repetition_key(), fewer.repetition_key());
}

#[test]
fn losing_and_returning_does_not_restore_castling_rights() {
    let start = pos("r3k2r/8/8/8/8/8/8/R3K2R w KQkq - 0 1");
    let back = play(&start, &["e1f1", "e8f8", "f1e1", "f8e8"]);
    assert_eq!(
        back.to_fen().split(' ').next(),
        start.to_fen().split(' ').next()
    );
    assert_ne!(start.repetition_key(), back.repetition_key());
}

#[test]
fn capturable_en_passant_is_part_of_the_key() {
    // O peão preto em d4 pode capturar en passant em e3.
    let with_ep = pos("rnbqkbnr/ppp1pppp/8/8/3pP3/8/PPPP1PPP/RNBQKBNR b KQkq e3 0 3");
    let without_ep = pos("rnbqkbnr/ppp1pppp/8/8/3pP3/8/PPPP1PPP/RNBQKBNR b KQkq - 0 3");
    assert_ne!(with_ep.repetition_key(), without_ep.repetition_key());
}

#[test]
fn uncapturable_en_passant_is_ignored() {
    // Nenhum peão preto ao lado de e4: a casa e3 do FEN não conta.
    let after_e4 = play(&CanonicalPosition::start(), &["e2e4"]);
    assert!(after_e4.to_fen().contains(" e3 "));
    let no_ep = pos("rnbqkbnr/pppppppp/8/8/4P3/8/PPPP1PPP/RNBQKBNR b KQkq - 0 1");
    assert_eq!(after_e4.repetition_key(), no_ep.repetition_key());
}

#[test]
fn illegal_en_passant_because_of_pin_does_not_count() {
    // A captura d4xe3 retiraria os dois peões da fileira 4 e exporia o rei em h4 à torre a4.
    let pinned = pos("8/8/8/8/R2pP2k/8/8/4K3 b - e3 0 1");
    assert!(!pinned.legal_moves().iter().any(|m| m.to_string() == "d4e3"));
    let plain = pos("8/8/8/8/R2pP2k/8/8/4K3 b - - 0 1");
    assert_eq!(pinned.repetition_key(), plain.repetition_key());
}
