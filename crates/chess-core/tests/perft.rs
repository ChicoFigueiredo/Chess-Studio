//! CORE-02 e posições de referência do chessprogramming wiki.
//! Profundidades caras ficam em testes `#[ignore]`; rode com
//! `cargo test -p chess-core --release -- --ignored`.

use chess_core::CanonicalPosition;

fn check(fen: &str, expected: &[u64]) {
    let pos = CanonicalPosition::from_fen(fen).expect("FEN de referência válido");
    for (i, &nodes) in expected.iter().enumerate() {
        let depth = i as u32 + 1;
        assert_eq!(pos.perft(depth), nodes, "perft({depth}) de {fen}");
    }
}

const START: &str = "rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 0 1";
const KIWIPETE: &str = "r3k2r/p1ppqpb1/bn2pnp1/3PN3/1p2P3/2N2Q1p/PPPBBPPP/R3K2R w KQkq - 0 1";
const POSITION_3: &str = "8/2p5/3p4/KP5r/1R3p1k/8/4P1P1/8 w - - 0 1";
const POSITION_4: &str = "r3k2r/Pppp1ppp/1b3nbN/nP6/BBP1P3/q4N2/Pp1P2PP/R2Q1RK1 w kq - 0 1";
const POSITION_4_MIRRORED: &str =
    "r2q1rk1/pP1p2pp/Q4n2/bbp1p3/Np6/1B3NBn/pPPP1PPP/R3K2R b KQ - 0 1";
const POSITION_5: &str = "rnbq1k1r/pp1Pbppp/2p5/8/2B5/8/PPP1NnPP/RNBQK2R w KQ - 1 8";

#[test]
fn perft_start_position_depth_1_to_4() {
    // CORE-02
    check(START, &[20, 400, 8_902, 197_281]);
}

#[test]
fn perft_start_position_via_start_constructor() {
    assert_eq!(CanonicalPosition::start().perft(3), 8_902);
}

#[test]
fn perft_depth_zero_is_one() {
    assert_eq!(CanonicalPosition::start().perft(0), 1);
}

#[test]
fn perft_kiwipete() {
    check(KIWIPETE, &[48, 2_039, 97_862, 4_085_603]);
}

#[test]
fn perft_position_3() {
    check(POSITION_3, &[14, 191, 2_812, 43_238, 674_624]);
}

#[test]
fn perft_position_4() {
    check(POSITION_4, &[6, 264, 9_467, 422_333]);
}

#[test]
fn perft_position_4_mirrored() {
    check(POSITION_4_MIRRORED, &[6, 264, 9_467, 422_333]);
}

#[test]
fn perft_position_5() {
    check(POSITION_5, &[44, 1_486, 62_379, 2_103_487]);
}

#[test]
#[ignore = "caro em debug; rode com --release -- --ignored"]
fn perft_deep() {
    check(START, &[20, 400, 8_902, 197_281, 4_865_609, 119_060_324]);
    check(KIWIPETE, &[48, 2_039, 97_862, 4_085_603, 193_690_690]);
    check(POSITION_3, &[14, 191, 2_812, 43_238, 674_624, 11_030_083]);
    check(POSITION_4, &[6, 264, 9_467, 422_333, 15_833_292]);
    check(POSITION_5, &[44, 1_486, 62_379, 2_103_487, 89_941_194]);
}
