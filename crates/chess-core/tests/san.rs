use chess_core::{CanonicalPosition, Move, SanError};

fn pos(fen: &str) -> CanonicalPosition {
    CanonicalPosition::from_fen(fen).expect("FEN válido")
}

fn mv(uci: &str) -> Move {
    uci.parse().expect("UCI válido")
}

fn san(fen: &str, uci: &str) -> String {
    pos(fen).san(mv(uci)).expect("lance legal")
}

const START: &str = "rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 0 1";
const KIWIPETE: &str = "r3k2r/p1ppqpb1/bn2pnp1/3PN3/1p2P3/2N2Q1p/PPPBBPPP/R3K2R w KQkq - 0 1";
const POS3: &str = "8/2p5/3p4/KP5r/1R3p1k/8/4P1P1/8 w - - 0 1";
const POS4: &str = "r3k2r/Pppp1ppp/1b3nbN/nP6/BBP1P3/q4N2/Pp1P2PP/R2Q1RK1 w kq - 0 1";
const POS5: &str = "rnbq1k1r/pp1Pbppp/2p5/8/2B5/8/PPP1NnPP/RNBQK2R w KQ - 1 8";

#[test]
fn writes_simple_pawn_and_piece_moves() {
    assert_eq!(san(START, "e2e4"), "e4");
    assert_eq!(san(START, "g1f3"), "Nf3");
}

#[test]
fn writes_captures_with_x() {
    let fen = "rnbqkbnr/ppp1pppp/8/3p4/4P3/8/PPPP1PPP/RNBQKBNR w KQkq d6 0 2";
    assert_eq!(san(fen, "e4d5"), "exd5");
    let fen = "4k3/8/8/3p4/8/2N5/8/4K3 w - - 0 1";
    assert_eq!(san(fen, "c3d5"), "Nxd5");
    assert_eq!(san(fen, "c3b5"), "Nb5");
}

#[test]
fn disambiguates_two_knights_by_file() {
    let fen = "4k3/8/8/8/8/8/8/1N2KN2 w - - 0 1";
    assert_eq!(san(fen, "b1d2"), "Nbd2");
    assert_eq!(san(fen, "f1d2"), "Nfd2");
}

#[test]
fn disambiguates_by_rank_when_files_coincide() {
    let fen = "4k3/8/8/8/8/R7/8/R3K3 w - - 0 1";
    assert_eq!(san(fen, "a1a2"), "R1a2");
    assert_eq!(san(fen, "a3a2"), "R3a2");
}

#[test]
fn disambiguates_three_queens_by_file_rank_or_both() {
    let fen = "7k/8/8/8/8/Q7/8/Q1Q4K w - - 0 1";
    assert_eq!(san(fen, "a1c3"), "Qa1c3+");
    assert_eq!(san(fen, "a3c3"), "Q3c3+");
    assert_eq!(san(fen, "c1c3"), "Qcc3+");
}

#[test]
fn writes_castling() {
    let fen = "r3k2r/8/8/8/8/8/8/R3K2R w KQkq - 0 1";
    assert_eq!(san(fen, "e1g1"), "O-O");
    assert_eq!(san(fen, "e1c1"), "O-O-O");
    let fen = "r3k2r/8/8/8/8/8/8/R3K2R b KQkq - 0 1";
    assert_eq!(san(fen, "e8g8"), "O-O");
    assert_eq!(san(fen, "e8c8"), "O-O-O");
}

#[test]
fn writes_castling_with_check_suffix() {
    // A torre que roca para f1 dá xeque ao rei em f8.
    let fen = "5k2/8/8/8/8/8/8/4K2R w K - 0 1";
    assert_eq!(san(fen, "e1g1"), "O-O+");
}

#[test]
fn writes_promotion_with_and_without_check() {
    let fen = "8/P6k/8/8/8/8/8/K7 w - - 0 1";
    assert_eq!(san(fen, "a7a8q"), "a8=Q");
    assert_eq!(san(fen, "a7a8n"), "a8=N");
    let fen = "7k/P7/8/8/8/8/8/K7 w - - 0 1";
    assert_eq!(san(fen, "a7a8q"), "a8=Q+");
    assert_eq!(san(fen, "a7a8r"), "a8=R+");
}

#[test]
fn writes_capture_promotion() {
    let fen = "1n5k/P7/8/8/8/8/8/K7 w - - 0 1";
    assert_eq!(san(fen, "a7b8q"), "axb8=Q");
}

#[test]
fn writes_en_passant_as_pawn_capture() {
    let fen = "4k3/8/8/3pP3/8/8/8/4K3 w - d6 0 1";
    assert_eq!(san(fen, "e5d6"), "exd6");
}

#[test]
fn writes_check_and_checkmate_suffixes() {
    let fen = "r1bqkb1r/pppp1ppp/2n2n2/4p2Q/2B1P3/8/PPPP1PPP/RNB1K1NR w KQkq - 4 4";
    assert_eq!(san(fen, "h5f7"), "Qxf7#");
    assert_eq!(san(START, "d2d4"), "d4");
    let fen = "4k3/8/8/8/8/8/8/R3K3 w - - 0 1";
    assert_eq!(san(fen, "a1a8"), "Ra8+");
}

#[test]
fn san_of_illegal_move_is_error() {
    assert!(pos(START).san(mv("e2e5")).is_err());
}

#[test]
fn parses_basic_san() {
    let p = pos(START);
    assert_eq!(p.parse_san("e4"), Ok(mv("e2e4")));
    assert_eq!(p.parse_san("Nf3"), Ok(mv("g1f3")));
}

#[test]
fn parse_tolerates_suffixes_and_annotations() {
    let p = pos(START);
    assert_eq!(p.parse_san("Nf3+"), Ok(mv("g1f3")));
    assert_eq!(p.parse_san("Nf3#"), Ok(mv("g1f3")));
    assert_eq!(p.parse_san("e4!?"), Ok(mv("e2e4")));
    assert_eq!(p.parse_san("  e4 ?? "), Ok(mv("e2e4")));
    let fen = "r1bqkb1r/pppp1ppp/2n2n2/4p2Q/2B1P3/8/PPPP1PPP/RNB1K1NR w KQkq - 4 4";
    assert_eq!(pos(fen).parse_san("Qxf7"), Ok(mv("h5f7")));
}

#[test]
fn parse_accepts_zero_castling() {
    let p = pos("r3k2r/8/8/8/8/8/8/R3K2R w KQkq - 0 1");
    assert_eq!(p.parse_san("0-0"), Ok(mv("e1g1")));
    assert_eq!(p.parse_san("0-0-0"), Ok(mv("e1c1")));
    assert_eq!(p.parse_san("O-O"), Ok(mv("e1g1")));
    assert_eq!(p.parse_san("O-O-O+"), Ok(mv("e1c1")));
}

#[test]
fn parse_handles_promotion_forms() {
    let p = pos("8/P6k/8/8/8/8/8/K7 w - - 0 1");
    assert_eq!(p.parse_san("a8=Q"), Ok(mv("a7a8q")));
    assert_eq!(p.parse_san("a8Q"), Ok(mv("a7a8q")));
    assert_eq!(p.parse_san("a8=n"), Ok(mv("a7a8n")));
    // Falta a peça de promoção: nenhum lance legal corresponde.
    assert!(matches!(
        p.parse_san("a8"),
        Err(SanError::NoMatchingMove(_))
    ));
}

#[test]
fn parse_handles_disambiguation_forms() {
    let p = pos("4k3/8/8/8/8/8/8/1N2KN2 w - - 0 1");
    assert_eq!(p.parse_san("Nbd2"), Ok(mv("b1d2")));
    assert_eq!(p.parse_san("Nfd2"), Ok(mv("f1d2")));
    let p = pos("7k/8/8/8/8/Q7/8/Q1Q4K w - - 0 1");
    assert_eq!(p.parse_san("Qa1c3"), Ok(mv("a1c3")));
    assert_eq!(p.parse_san("Q3c3"), Ok(mv("a3c3")));
    assert_eq!(p.parse_san("Qcc3"), Ok(mv("c1c3")));
}

#[test]
fn parse_rejects_ambiguous_san() {
    let p = pos("4k3/8/8/8/8/8/8/1N2KN2 w - - 0 1");
    match p.parse_san("Nd2") {
        Err(SanError::Ambiguous(mut moves)) => {
            moves.sort_by_key(|m| m.to_string());
            assert_eq!(moves, vec![mv("b1d2"), mv("f1d2")]);
        }
        other => panic!("esperava Ambiguous, veio {other:?}"),
    }
}

#[test]
fn parse_rejects_illegal_and_malformed_san() {
    let p = pos(START);
    assert!(matches!(
        p.parse_san("Nf6"),
        Err(SanError::NoMatchingMove(_))
    ));
    assert!(matches!(
        p.parse_san("e5"),
        Err(SanError::NoMatchingMove(_))
    ));
    assert!(matches!(
        p.parse_san("O-O"),
        Err(SanError::NoMatchingMove(_))
    ));
    assert_eq!(p.parse_san(""), Err(SanError::Empty));
    assert_eq!(p.parse_san("  "), Err(SanError::Empty));
    assert!(matches!(p.parse_san("Zz9"), Err(SanError::Syntax(_))));
    assert!(matches!(p.parse_san("N"), Err(SanError::Syntax(_))));
    // Rei por duas casas só é notação de roque, nunca "Kg1".
    let castle = pos("r3k2r/8/8/8/8/8/8/R3K2R w KQkq - 0 1");
    assert!(matches!(
        castle.parse_san("Kg1"),
        Err(SanError::NoMatchingMove(_))
    ));
}

#[test]
fn parse_distinguishes_pawn_b_from_bishop_b() {
    let p = pos("rnbqkbnr/pppp1ppp/8/4p3/4P3/8/PPPP1PPP/RNBQKBNR w KQkq - 0 2");
    assert_eq!(p.parse_san("b4"), Ok(mv("b2b4")));
    assert_eq!(p.parse_san("Bb5"), Ok(mv("f1b5")));
}

/// Ida e volta SAN <-> Lance em todos os lances legais até a profundidade dada.
fn roundtrip(p: &CanonicalPosition, depth: u32) {
    let mut seen = std::collections::HashSet::new();
    for m in p.legal_moves() {
        let text = p.san(m).expect("lance legal tem SAN");
        assert!(seen.insert(text.clone()), "SAN repetida {text} em {p}");
        assert_eq!(
            p.parse_san(&text),
            Ok(m),
            "ida e volta falhou: {text} em {p}"
        );
        if depth > 1 {
            roundtrip(&p.play(m).unwrap(), depth - 1);
        }
    }
}

#[test]
fn roundtrip_all_legal_moves_of_reference_positions() {
    for fen in [START, KIWIPETE, POS3, POS4, POS5] {
        roundtrip(&pos(fen), 2);
    }
}

#[test]
fn roundtrip_deeper_on_kiwipete_and_position_4() {
    roundtrip(&pos(KIWIPETE), 3);
    roundtrip(&pos(POS4), 3);
}
