use chess_core::{CanonicalPosition, Color, FenError, Piece, Square};

const START: &str = "rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 0 1";

fn sq(s: &str) -> Square {
    s.parse().unwrap()
}

#[test]
fn start_position_matches_start_fen() {
    assert_eq!(CanonicalPosition::start().to_fen(), START);
    assert_eq!(
        CanonicalPosition::from_fen(START).unwrap(),
        CanonicalPosition::start()
    );
}

#[test]
fn valid_fens_round_trip_identically() {
    let fens = [
        START,
        "r3k2r/p1ppqpb1/bn2pnp1/3PN3/1p2P3/2N2Q1p/PPPBBPPP/R3K2R w KQkq - 0 1",
        "8/2p5/3p4/KP5r/1R3p1k/8/4P1P1/8 w - - 0 1",
        "rnbq1k1r/pp1Pbppp/2p5/8/2B5/8/PPP1NnPP/RNBQK2R w KQ - 1 8",
        "rnbqkbnr/ppp1pppp/8/3pP3/8/8/PPPP1PPP/RNBQKBNR w KQkq d6 0 3",
        "4k3/8/8/8/8/8/8/4K3 b - - 37 90",
    ];
    for fen in fens {
        let pos = CanonicalPosition::from_fen(fen).unwrap();
        assert_eq!(pos.to_fen(), fen);
    }
}

#[test]
fn exposes_pieces_and_state() {
    let pos = CanonicalPosition::start();
    assert_eq!(pos.side_to_move(), Color::White);
    assert_eq!(pos.piece_at(sq("e1")), Some((Color::White, Piece::King)));
    assert_eq!(pos.piece_at(sq("d8")), Some((Color::Black, Piece::Queen)));
    assert_eq!(pos.piece_at(sq("e4")), None);
    assert_eq!(pos.halfmove_clock(), 0);
    assert_eq!(pos.fullmove_number(), 1);
    assert!(!pos.is_check());
}

#[test]
fn surrounding_and_repeated_whitespace_is_tolerated() {
    let padded = format!("  {}  ", START.replace(' ', "  "));
    let pos = CanonicalPosition::from_fen(&padded).unwrap();
    assert_eq!(pos.to_fen(), START);
}

#[test]
fn invalid_fens_return_typed_errors_without_panicking() {
    let cases: &[(&str, FenError)] = &[
        ("", FenError::MissingField),
        (
            "rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq -",
            FenError::MissingField,
        ),
        (
            "rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 0 1 x",
            FenError::TooManyFields,
        ),
        (
            "rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP w KQkq - 0 1",
            FenError::InvalidBoard,
        ),
        (
            "rnbqkbnr/pppppppp/9/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 0 1",
            FenError::InvalidBoard,
        ),
        (
            "rnbqkbnr/ppppxppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 0 1",
            FenError::InvalidBoard,
        ),
        // sem rei branco
        (
            "rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQ1BNR w kq - 0 1",
            FenError::InvalidBoard,
        ),
        // peão na primeira fileira
        (
            "rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNP w KQkq - 0 1",
            FenError::InvalidBoard,
        ),
        // lado que não joga está em xeque
        ("4k3/8/8/8/8/8/4R3/4K3 w - - 0 1", FenError::InvalidBoard),
        (&START.replace(" w ", " x "), FenError::InvalidSideToMove),
        (
            &START.replace("KQkq", "KQkz"),
            FenError::InvalidCastlingRights,
        ),
        // direito de roque sem a torre
        (
            "4k3/8/8/8/8/8/8/4K3 w K - 0 1",
            FenError::InvalidCastlingRights,
        ),
        (&START.replace(" - 0 1", " e9 0 1"), FenError::InvalidEnPassant),
        // en passant sem peão que tenha avançado
        (&START.replace(" - 0 1", " e3 0 1"), FenError::InvalidEnPassant),
        (
            &START.replace(" 0 1", " abc 1"),
            FenError::InvalidHalfmoveClock,
        ),
        (
            &START.replace(" 0 1", " 0 0"),
            FenError::InvalidFullmoveNumber,
        ),
        (
            &START.replace(" 0 1", " 0 -3"),
            FenError::InvalidFullmoveNumber,
        ),
    ];
    for (fen, expected) in cases {
        assert_eq!(
            CanonicalPosition::from_fen(fen).unwrap_err(),
            *expected,
            "FEN: {fen:?}"
        );
    }
}

#[test]
fn fen_errors_have_descriptive_messages() {
    let err = CanonicalPosition::from_fen("lixo").unwrap_err();
    assert!(!err.to_string().is_empty());
}
