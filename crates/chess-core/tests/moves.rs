use chess_core::{CanonicalPosition, Color, IllegalMoveError, Move, MoveParseError, Piece, Square};

fn pos(fen: &str) -> CanonicalPosition {
    CanonicalPosition::from_fen(fen).expect("FEN válido")
}

fn mv(uci: &str) -> Move {
    uci.parse().expect("UCI válido")
}

fn sq(s: &str) -> Square {
    s.parse().expect("casa válida")
}

fn legal(p: &CanonicalPosition, uci: &str) -> bool {
    p.is_legal(mv(uci))
}

fn ucis(p: &CanonicalPosition) -> Vec<String> {
    let mut v: Vec<String> = p.legal_moves().iter().map(|m| m.to_string()).collect();
    v.sort();
    v
}

#[test]
fn uci_parse_and_format() {
    let m = mv("e2e4");
    assert_eq!(m.from(), sq("e2"));
    assert_eq!(m.to(), sq("e4"));
    assert_eq!(m.promotion(), None);
    assert_eq!(m.to_string(), "e2e4");

    assert_eq!(mv("e7e8q").promotion(), Some(Piece::Queen));
    for s in ["e7e8q", "a7a8r", "a7a8b", "a7a8n", "b2b1n"] {
        assert_eq!(mv(s).to_string(), s);
    }
}

#[test]
fn uci_rejects_malformed_moves() {
    for s in [
        "", "e2", "e2e", "e2e9", "i2e4", "e2e4x", "e7e8k", "e7e8p", "e2e4qq", "E2E4",
    ] {
        assert!(s.parse::<Move>().is_err(), "{s:?} deveria falhar");
    }
    assert!(matches!(
        "e2e4x".parse::<Move>(),
        Err(MoveParseError::InvalidPromotion(_))
    ));
}

#[test]
fn start_position_has_twenty_legal_moves() {
    let p = CanonicalPosition::start();
    assert_eq!(p.legal_moves().len(), 20);
    assert!(legal(&p, "e2e4"));
    assert!(legal(&p, "g1f3"));
    assert!(!legal(&p, "e2e5"));
    assert!(!legal(&p, "e1e2"));
}

#[test]
fn play_returns_new_position_and_leaves_original_untouched() {
    let p = CanonicalPosition::start();
    let q = p.play(mv("e2e4")).unwrap();
    assert_eq!(p, CanonicalPosition::start());
    assert_eq!(q.side_to_move(), Color::Black);
    assert_eq!(q.piece_at(sq("e4")), Some((Color::White, Piece::Pawn)));
    assert_eq!(q.piece_at(sq("e2")), None);
    assert_eq!(
        q.to_fen(),
        "rnbqkbnr/pppppppp/8/8/4P3/8/PPPP1PPP/RNBQKBNR b KQkq e3 0 1"
    );
}

#[test]
fn illegal_move_is_rejected_with_error_and_no_side_effect() {
    let p = CanonicalPosition::start();
    assert_eq!(p.play(mv("e2e5")), Err(IllegalMoveError(mv("e2e5"))));
    assert_eq!(p, CanonicalPosition::start());
}

#[test]
fn move_that_leaves_king_in_check_is_illegal() {
    // Torre branca em e2 cravada contra o rei por torre preta em e8.
    let p = pos("4r1k1/8/8/8/8/8/4R3/4K3 w - - 0 1");
    assert!(!legal(&p, "e2d2"));
    assert!(legal(&p, "e2e5"));
    assert!(legal(&p, "e2e8"));
}

#[test]
fn must_respond_to_check() {
    let p = pos("4k3/8/8/8/8/8/4r3/4K3 w - - 0 1");
    assert!(p.is_check());
    assert_eq!(ucis(&p), vec!["e1d1", "e1e2", "e1f1"]);
}

// ---------- roque ----------

const CASTLE_FEN: &str = "r3k2r/8/8/8/8/8/8/R3K2R w KQkq - 0 1";

#[test]
fn castling_is_generated_in_uci_king_two_squares() {
    let p = pos(CASTLE_FEN);
    assert!(legal(&p, "e1g1"));
    assert!(legal(&p, "e1c1"));
    let moves = ucis(&p);
    assert!(moves.contains(&"e1g1".to_string()));
    assert!(moves.contains(&"e1c1".to_string()));
    // a notação "rei captura a própria torre" não vaza
    assert!(!moves.contains(&"e1h1".to_string()));
    assert!(!legal(&p, "e1h1"));
}

#[test]
fn castling_moves_both_king_and_rook() {
    let short = pos(CASTLE_FEN).play(mv("e1g1")).unwrap();
    assert_eq!(short.to_fen(), "r3k2r/8/8/8/8/8/8/R4RK1 b kq - 1 1");
    let long = pos(CASTLE_FEN).play(mv("e1c1")).unwrap();
    assert_eq!(long.to_fen(), "r3k2r/8/8/8/8/8/8/2KR3R b kq - 1 1");
    let black = pos("r3k2r/8/8/8/8/8/8/R3K2R b KQkq - 0 1")
        .play(mv("e8c8"))
        .unwrap();
    assert_eq!(black.to_fen(), "2kr3r/8/8/8/8/8/8/R3K2R w KQ - 1 2");
}

#[test]
fn castling_refused_when_king_in_check() {
    let p = pos("4r1k1/8/8/8/8/8/8/R3K2R w KQ - 0 1");
    assert!(p.is_check());
    assert!(!legal(&p, "e1g1"));
    assert!(!legal(&p, "e1c1"));
}

#[test]
fn castling_refused_when_king_passes_through_attacked_square() {
    // torre preta em f8 ataca f1 (casa de passagem do roque curto)
    let p = pos("5rk1/8/8/8/8/8/8/R3K2R w KQ - 0 1");
    assert!(!legal(&p, "e1g1"));
    assert!(legal(&p, "e1c1"));
    // torre preta em d8 ataca d1 (passagem do roque longo)
    let p = pos("3r2k1/8/8/8/8/8/8/R3K2R w KQ - 0 1");
    assert!(!legal(&p, "e1c1"));
    assert!(legal(&p, "e1g1"));
}

#[test]
fn castling_refused_when_king_lands_on_attacked_square() {
    let p = pos("1k4r1/8/8/8/8/8/8/R3K2R w KQ - 0 1");
    assert!(!legal(&p, "e1g1"));
    assert!(legal(&p, "e1c1"));
}

#[test]
fn long_castling_allowed_when_only_rook_path_square_is_attacked() {
    // b1 atacada pela torre em b8: o rei não passa por b1, então o roque longo vale.
    let p = pos("1r4k1/8/8/8/8/8/8/R3K2R w KQ - 0 1");
    assert!(legal(&p, "e1c1"));
}

#[test]
fn castling_refused_when_pieces_are_between() {
    let p = pos("r3k2r/8/8/8/8/8/8/RN2K1NR w KQkq - 0 1");
    assert!(!legal(&p, "e1g1"));
    assert!(!legal(&p, "e1c1"));
}

#[test]
fn castling_refused_after_king_or_rook_moved() {
    // sem direitos no FEN
    let p = pos("r3k2r/8/8/8/8/8/8/R3K2R w - - 0 1");
    assert!(!legal(&p, "e1g1"));
    assert!(!legal(&p, "e1c1"));
    // torre h1 moveu e voltou: só o lado do rei perdeu o direito
    let p = pos(CASTLE_FEN)
        .play(mv("h1h2"))
        .unwrap()
        .play(mv("a8a7"))
        .unwrap()
        .play(mv("h2h1"))
        .unwrap()
        .play(mv("a7a8"))
        .unwrap();
    assert!(!legal(&p, "e1g1"));
    assert!(legal(&p, "e1c1"));
    // rei moveu e voltou: nenhum roque
    let p = pos(CASTLE_FEN)
        .play(mv("e1e2"))
        .unwrap()
        .play(mv("a8a7"))
        .unwrap()
        .play(mv("e2e1"))
        .unwrap()
        .play(mv("a7a8"))
        .unwrap();
    assert!(!legal(&p, "e1g1"));
    assert!(!legal(&p, "e1c1"));
}

#[test]
fn capturing_a_rook_removes_the_castling_right() {
    let p = pos("r3k2r/8/8/8/8/8/6b1/R3K2R b KQkq - 0 1");
    let p = p.play(mv("g2h1")).unwrap();
    assert_eq!(p.to_fen(), "r3k2r/8/8/8/8/8/8/R3K2b w Qkq - 0 2");
}

// ---------- en passant ----------

#[test]
fn en_passant_is_legal_immediately_after_double_push() {
    let p = pos("4k3/8/8/8/3p4/8/4P3/4K3 w - - 0 1")
        .play(mv("e2e4"))
        .unwrap();
    // (a casa e3 está no FEN porque o peão preto em d4 pode capturar)
    assert!(legal(&p, "d4e3"));
    let after = p.play(mv("d4e3")).unwrap();
    assert_eq!(after.piece_at(sq("e4")), None);
    assert_eq!(after.piece_at(sq("e3")), Some((Color::Black, Piece::Pawn)));
    assert_eq!(after.to_fen(), "4k3/8/8/8/8/4p3/8/4K3 w - - 0 2");
}

#[test]
fn en_passant_expires_after_one_move() {
    let p = pos("4k3/7p/8/8/3p4/8/4P3/4K3 w - - 0 1")
        .play(mv("e2e4"))
        .unwrap()
        .play(mv("h7h6"))
        .unwrap()
        .play(mv("e1d1"))
        .unwrap();
    assert!(!legal(&p, "d4e3"));
    assert!(!legal(&p.play(mv("h6h5")).unwrap(), "d4e3"));
}

#[test]
fn en_passant_white_captures_too() {
    let p = pos("4k3/3p4/8/4P3/8/8/8/4K3 b - - 0 1")
        .play(mv("d7d5"))
        .unwrap();
    assert!(legal(&p, "e5d6"));
    let after = p.play(mv("e5d6")).unwrap();
    assert_eq!(after.piece_at(sq("d5")), None);
}

#[test]
fn en_passant_refused_if_it_exposes_own_king() {
    // Os dois peões saem da quinta fileira de uma vez: tomar en passant abre a fileira para a torre.
    let p = pos("8/8/8/KPp4r/8/8/8/4k3 w - c6 0 1");
    assert!(!legal(&p, "b5c6"));
    assert!(legal(&p, "b5b6"));
}

// ---------- promoção ----------

#[test]
fn promotion_offers_all_four_pieces() {
    let p = pos("8/4P3/8/8/8/8/8/k6K w - - 0 1");
    for s in ["e7e8q", "e7e8r", "e7e8b", "e7e8n"] {
        assert!(legal(&p, s), "{s}");
    }
    assert!(!legal(&p, "e7e8"), "promoção sem peça é ilegal");
    assert_eq!(
        p.legal_moves()
            .iter()
            .filter(|m| m.promotion().is_some())
            .count(),
        4
    );
}

#[test]
fn promotion_with_capture_for_each_piece() {
    let p = pos("3rr2k/4P3/8/8/8/8/8/K7 w - - 0 1");
    // e8 ocupada por torre: só a captura em d8 promove
    assert!(!legal(&p, "e7e8q"));
    for (s, piece) in [
        ("e7d8q", Piece::Queen),
        ("e7d8r", Piece::Rook),
        ("e7d8b", Piece::Bishop),
        ("e7d8n", Piece::Knight),
    ] {
        assert!(legal(&p, s), "{s}");
        let after = p.play(mv(s)).unwrap();
        assert_eq!(after.piece_at(sq("d8")), Some((Color::White, piece)));
        assert_eq!(after.piece_at(sq("e7")), None);
    }
}

#[test]
fn black_promotion_works() {
    let p = pos("k6K/8/8/8/8/8/3p4/8 b - - 0 1");
    assert!(legal(&p, "d2d1n"));
    let after = p.play(mv("d2d1n")).unwrap();
    assert_eq!(
        after.piece_at(sq("d1")),
        Some((Color::Black, Piece::Knight))
    );
}

#[test]
fn promotion_piece_on_non_promoting_move_is_illegal() {
    let p = CanonicalPosition::start();
    assert!(!legal(&p, "e2e4q"));
}

#[test]
fn halfmove_clock_and_fullmove_number_advance() {
    let p = CanonicalPosition::start()
        .play(mv("g1f3"))
        .unwrap()
        .play(mv("g8f6"))
        .unwrap();
    assert_eq!(p.halfmove_clock(), 2);
    assert_eq!(p.fullmove_number(), 2);
    let p = p.play(mv("e2e4")).unwrap();
    assert_eq!(p.halfmove_clock(), 0);
}

#[test]
fn halfmove_clock_counts_past_100_and_resets_on_capture_or_pawn_move() {
    let mut p = pos("8/8/8/4k3/8/8/8/4K3 w - - 0 1");
    for m in ["e1e2", "e5e6", "e2e1", "e6e5"].iter().cycle().take(101) {
        p = p.play(mv(m)).unwrap();
    }
    assert_eq!(p.halfmove_clock(), 101);
    assert!(p.to_fen().contains(" 101 "));

    // a partir de 120 (vindo de FEN) segue contando e zera em lance de peão
    let q = pos("8/8/8/4k3/8/8/4P3/4K3 w - - 149 200");
    let q = q.play(mv("e1d1")).unwrap();
    assert_eq!(q.halfmove_clock(), 150);
    let q = q.play(mv("e5e6")).unwrap().play(mv("e2e4")).unwrap();
    assert_eq!(q.halfmove_clock(), 0);

    // captura zera
    let c = pos("8/8/8/4k3/8/8/8/R3K3 w - - 130 90")
        .play(mv("a1a5"))
        .unwrap();
    assert_eq!(c.halfmove_clock(), 131);
    let c = pos("8/8/8/r3k3/8/8/8/R3K3 w - - 130 90")
        .play(mv("a1a5"))
        .unwrap();
    assert_eq!(c.halfmove_clock(), 0);
}
