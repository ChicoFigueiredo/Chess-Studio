#![allow(clippy::unwrap_used)]
//! Conferência da Súmula manual (V2-SUM-02): tabela de súmulas com erros conhecidos.
//! Cada caso monta a linha real em SAN, escreve a Súmula e confere a classe por meio-lance.

use Verdict::{AcceptedWithCaveat as Caveat, Correct, Extra, Missing, Swapped, WrongNotation};
use chess_core::{CanonicalPosition, Move, Square};
use chess_game::{Command, Game, GameSetup, PolicyKind, Timestamp};
use chess_training::{
    CheckError, ManualScoresheet, ScoresheetCheck, Verdict, check_against_game, check_scoresheet,
};

fn line(start: &CanonicalPosition, sans: &[&str]) -> Vec<Move> {
    let mut pos = start.clone();
    let mut out = Vec::new();
    for san in sans {
        let mv = pos.parse_san(san).unwrap_or_else(|e| panic!("{san}: {e}"));
        pos = pos.play(mv).unwrap();
        out.push(mv);
    }
    out
}

fn sheet(entries: &[(u16, &str)], deliver: bool) -> ManualScoresheet {
    let mut s =
        ManualScoresheet::from_parts(entries.iter().map(|(p, t)| (*p, (*t).to_owned())), None)
            .unwrap();
    if deliver {
        s.deliver(1_000).unwrap();
    }
    s
}

fn check(start: &CanonicalPosition, real: &[&str], written: &[(u16, &str)]) -> ScoresheetCheck {
    let moves = line(start, real);
    check_scoresheet(&sheet(written, true), start, &moves, 42).unwrap()
}

fn verdicts(c: &ScoresheetCheck) -> Vec<Verdict> {
    c.entries().iter().map(|e| e.verdict).collect()
}

fn fen(f: &str) -> CanonicalPosition {
    CanonicalPosition::from_fen(f).unwrap()
}

#[test]
fn exact_canonical_san_is_correct_and_clean() {
    let c = check(
        &CanonicalPosition::start(),
        &["e4", "e5", "Nf3"],
        &[(1, "e4"), (2, "e5"), (3, "Nf3")],
    );
    assert_eq!(verdicts(&c), vec![Correct; 3]);
    assert!(c.is_clean());
    assert_eq!(c.checked_at_ms(), 42);
    assert_eq!(c.entries()[2].expected.as_deref(), Some("Nf3"));
}

#[test]
fn accepted_notation_variations_table() {
    let start = CanonicalPosition::start();
    // (linha real, ply, escrito, classe esperada)
    let table: &[(&[&str], u16, &str, Verdict)] = &[
        // letras pt-BR
        (&["e4", "e5", "Nf3"], 3, "Cf3", Caveat),
        (&["e4", "e5", "Qh5"], 3, "Dh5", Caveat),
        (&["e4", "e5", "Bc4"], 3, "Bc4", Correct),
        (
            &["e4", "e5", "Nf3", "Nc6", "Bc4", "Bc5", "O-O"],
            7,
            "0-0",
            Caveat,
        ),
        (
            &["e4", "e5", "Nf3", "Nc6", "Bc4", "Bc5", "O-O"],
            7,
            "o-o",
            Caveat,
        ),
        (
            &["e4", "e5", "Nf3", "Nc6", "Bc4", "Bc5", "O-O"],
            7,
            "O-O",
            Correct,
        ),
        // sufixos
        (
            &["e4", "e5", "Bc4", "Nc6", "Qh5", "Nf6", "Qxf7#"],
            7,
            "Qxf7#",
            Correct,
        ),
        (
            &["e4", "e5", "Bc4", "Nc6", "Qh5", "Nf6", "Qxf7#"],
            7,
            "Qxf7",
            Caveat,
        ),
        (
            &["e4", "e5", "Bc4", "Nc6", "Qh5", "Nf6", "Qxf7#"],
            7,
            "Qxf7+",
            Caveat,
        ),
        (
            &["e4", "e5", "Bc4", "Nc6", "Qh5", "Nf6", "Qxf7#"],
            7,
            "Dxf7#",
            Caveat,
        ),
        (&["e4", "e5"], 1, "e4!?", Caveat),
        // x opcional
        (&["e4", "d5", "exd5"], 3, "ed5", Caveat),
        (&["e4", "d5", "exd5"], 3, "exd5", Correct),
        // desambiguação desnecessária é aceita
        (&["Nf3"], 1, "Ngf3", Caveat),
    ];
    for (real, ply, written, want) in table {
        let c = check(&start, real, &[(*ply, written)]);
        let got = c.entries().iter().find(|e| e.ply == *ply).unwrap().verdict;
        assert_eq!(got, *want, "{real:?} ply {ply}: escrito {written:?}");
    }
}

#[test]
fn promotion_variations() {
    let start = fen("8/P6k/8/8/8/8/8/K7 w - - 0 1");
    for (written, want) in [
        ("a8=Q", Correct),
        ("a8=D", Caveat),
        ("a8D", Caveat),
        ("a8Q", Caveat),
        ("a8=T", Swapped),
        ("a8=R", Swapped),
        ("a8", WrongNotation),
    ] {
        let c = check(&start, &["a8=Q"], &[(1, written)]);
        assert_eq!(c.entries()[0].verdict, want, "escrito {written:?}");
    }
}

#[test]
fn wrong_notation_versus_swapped_table() {
    let start = CanonicalPosition::start();
    let real = ["e4", "e5", "Nf3"];
    for (ply, written, want) in [
        (1, "d4", Swapped),       // outro Lance legal
        (2, "Nf6", Swapped),      // legal, mas não o jogado
        (1, "e5", WrongNotation), // ilegal para as brancas
        (1, "xyz", WrongNotation),
        (1, "e9", WrongNotation),
        (3, "nf3", WrongNotation), // letra minúscula não é aceita
        (3, "Kf3", WrongNotation),
    ] {
        let c = check(&start, &real, &[(ply, written)]);
        let got = c.entries().iter().find(|e| e.ply == ply).unwrap().verdict;
        assert_eq!(got, want, "ply {ply} escrito {written:?}");
    }
}

#[test]
fn missing_disambiguation_is_wrong_notation_unneeded_is_caveat() {
    let start = fen("4k3/8/8/8/8/1N3N2/8/4K3 w - - 0 1");
    for (written, want) in [
        ("Nbd2", Correct),
        ("Cbd2", Caveat),
        ("Nb3d2", Caveat),
        ("N3d2", WrongNotation),
        ("Nd2", WrongNotation),
        ("Cd2", WrongNotation),
    ] {
        let c = check(&start, &["Nbd2"], &[(1, written)]);
        assert_eq!(c.entries()[0].verdict, want, "escrito {written:?}");
    }
}

#[test]
fn r_is_read_as_rook_or_pt_br_king() {
    let start = fen("4k3/8/8/8/8/8/8/R3K3 w - - 0 1");
    // Rb1 é Torre (en) e Rei (pt-BR) não chega lá: Correct.
    assert_eq!(
        check(&start, &["Rb1"], &[(1, "Rb1")]).entries()[0].verdict,
        Correct
    );
    // Rd1: Torre legal (outro Lance) ou Rei legal (o jogado): aceito pela leitura pt-BR.
    assert_eq!(
        check(&start, &["Kd1"], &[(1, "Rd1")]).entries()[0].verdict,
        Caveat
    );
    // Rf1: só o Rei chega (leitura pt-BR).
    assert_eq!(
        check(&start, &["Kf1"], &[(1, "Rf1")]).entries()[0].verdict,
        Caveat
    );
    // Rf1 quando o jogado foi a Torre: Rf1 não é legal para a Torre (rei no caminho),
    // mas é Lance legal do Rei: outro Lance.
    assert_eq!(
        check(&start, &["Rb1"], &[(1, "Rf1")]).entries()[0].verdict,
        Swapped
    );
}

#[test]
fn missing_and_extra_entries() {
    let start = CanonicalPosition::start();
    let c = check(&start, &["e4", "e5"], &[(1, "e4"), (4, "Cf3")]);
    assert_eq!(verdicts(&c), vec![Correct, Missing, Extra]);
    let e = c.entries();
    assert_eq!(e[1].written, None);
    assert_eq!(e[1].expected.as_deref(), Some("e5"));
    assert_eq!(e[2].ply, 4);
    assert_eq!(e[2].written.as_deref(), Some("Cf3"));
    assert_eq!(e[2].expected, None);
    let counts = c.counts();
    assert_eq!((counts.correct, counts.missing, counts.extra), (1, 1, 1));
    assert!(!c.is_clean());
}

#[test]
fn an_empty_delivered_sheet_is_all_missing() {
    let c = check(&CanonicalPosition::start(), &["e4", "e5"], &[]);
    assert_eq!(verdicts(&c), vec![Missing, Missing]);
}

#[test]
fn only_delivered_sheets_are_checked() {
    let start = CanonicalPosition::start();
    let s = sheet(&[(1, "e4")], false);
    assert_eq!(
        check_scoresheet(&s, &start, &line(&start, &["e4"]), 0),
        Err(CheckError::NotDelivered)
    );
}

#[test]
fn verdict_keys_round_trip() {
    for v in Verdict::ALL {
        assert_eq!(Verdict::from_key(v.key()), Some(v));
    }
    assert_eq!(Verdict::from_key("nope"), None);
}

#[test]
fn checking_a_game_does_not_touch_its_events() {
    let mut game = Game::new(GameSetup::standard(PolicyKind::Assisted));
    for (i, (from, to)) in [("e2", "e4"), ("e7", "e5"), ("g1", "f3")]
        .iter()
        .enumerate()
    {
        let sq = |s: &str| s.parse::<Square>().unwrap();
        game.handle(
            Command::Release {
                from: sq(from),
                to: sq(to),
            },
            Timestamp::mono(1000 * (i as u64 + 1)),
        )
        .unwrap();
    }
    let events_before = game.events().to_vec();
    let s = sheet(&[(1, "e4"), (2, "e6"), (3, "Cf3")], true);
    let c = check_against_game(&s, &game, 7).unwrap();
    assert_eq!(verdicts(&c), vec![Correct, Swapped, Caveat]);
    assert_eq!(game.events(), events_before.as_slice());
}
