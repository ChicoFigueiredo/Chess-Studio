#![allow(clippy::unwrap_used)]
//! Memória de posição (C11, US-25): estudo cronometrado, reconstrução num tabuleiro
//! vazio e julgamento casa a casa.

use chess_core::{Color, Piece, Square};
use chess_training::{
    AidsRecord, Exercise, ExerciseKind, Feedback, PiecePlacement, PositionMemoryRun, PositionPhase,
    RunResult, Solution, SquareVerdict, judge,
};

fn sq(s: &str) -> Square {
    s.parse().unwrap()
}

fn no_aids() -> AidsRecord {
    AidsRecord::new(&[], Feedback::Immediate)
}

const FEN: &str = "6k1/5ppp/8/8/8/8/5PPP/R5K1 w - - 0 1";

fn exercise(seconds: u32) -> Exercise {
    Exercise {
        id: "memoria-posicao-teste".into(),
        kind: ExerciseKind::MemoryPosition,
        title: "Teste".into(),
        fen: FEN.into(),
        objective: "Memorize a posição e recoloque as peças.".into(),
        solution: Solution::MemorizePosition { seconds },
        explanation: "Explicação.".into(),
        version: 1,
        themes: Vec::new(),
        rating: None,
        provenance: None,
        pack: None,
    }
}

fn target() -> PiecePlacement {
    PiecePlacement::from_fen(FEN).unwrap()
}

#[test]
fn placement_reads_the_pieces_of_a_fen() {
    let t = target();
    assert_eq!(t.len(), 9);
    assert_eq!(t.get(sq("a1")), Some((Color::White, Piece::Rook)));
    assert_eq!(t.get(sq("g8")), Some((Color::Black, Piece::King)));
    assert_eq!(t.get(sq("e4")), None);
}

#[test]
fn an_exact_reconstruction_is_perfect() {
    let j = judge(&target(), &target());
    assert!(j.is_perfect());
    assert_eq!((j.correct, j.target_total), (9, 9));
    assert_eq!(j.percent(), 100);
    assert_eq!(
        j.verdict_at(sq("a1")),
        Some(SquareVerdict::Correct((Color::White, Piece::Rook)))
    );
}

#[test]
fn every_kind_of_mistake_is_told_apart_square_by_square() {
    let mut placed = target();
    placed.place(sq("a1"), Color::White, Piece::Queen); // peça errada (era torre)
    placed.remove(sq("g8")); // faltando o rei preto
    placed.place(sq("e4"), Color::White, Piece::Knight); // sobrando
    let j = judge(&target(), &placed);
    assert_eq!(
        (j.correct, j.wrong_piece, j.missing, j.extra, j.target_total),
        (7, 1, 1, 1, 9)
    );
    assert_eq!(
        j.verdict_at(sq("a1")),
        Some(SquareVerdict::WrongPiece {
            expected: (Color::White, Piece::Rook),
            placed: (Color::White, Piece::Queen)
        })
    );
    assert_eq!(
        j.verdict_at(sq("g8")),
        Some(SquareVerdict::Missing((Color::Black, Piece::King)))
    );
    assert_eq!(
        j.verdict_at(sq("e4")),
        Some(SquareVerdict::Extra((Color::White, Piece::Knight)))
    );
    assert_eq!(j.verdict_at(sq("d4")), None, "casa vazia certa não entra");
    assert!(!j.is_perfect());
}

#[test]
fn the_right_piece_of_the_wrong_color_is_a_wrong_piece() {
    let mut placed = target();
    placed.place(sq("a1"), Color::Black, Piece::Rook);
    let j = judge(&target(), &placed);
    assert_eq!(j.wrong_piece, 1);
    assert_eq!(j.correct, 8);
}

#[test]
fn percent_counts_correct_pieces_over_target_plus_extras() {
    // Nada colocado: 0 %.
    assert_eq!(judge(&target(), &PiecePlacement::empty()).percent(), 0);
    // 4 de 9 certas, nada sobrando: 44 % (arredondado).
    let mut half = PiecePlacement::empty();
    for s in ["a1", "g1", "g8", "f7"] {
        let (c, p) = target().get(sq(s)).unwrap();
        half.place(sq(s), c, p);
    }
    assert_eq!(judge(&target(), &half).percent(), 44);
    // Peça a mais derruba o percentual: 9 certas em 10 = 90 %.
    let mut extra = target();
    extra.place(sq("d4"), Color::White, Piece::Pawn);
    let j = judge(&target(), &extra);
    assert_eq!(j.percent(), 90);
    assert!(!j.is_perfect());
}

#[test]
fn verdicts_come_in_reading_order() {
    let mut placed = PiecePlacement::empty();
    placed.place(sq("h1"), Color::White, Piece::Pawn);
    placed.place(sq("a8"), Color::Black, Piece::Pawn);
    let j = judge(&PiecePlacement::empty(), &placed);
    let order: Vec<_> = j.squares.iter().map(|(s, _)| s.to_string()).collect();
    assert_eq!(order, ["a8", "h1"]);
}

#[test]
fn the_editor_places_moves_and_removes_pieces() {
    let mut p = PiecePlacement::empty();
    p.place(sq("e2"), Color::White, Piece::Pawn);
    assert!(p.move_piece(sq("e2"), sq("e4")));
    assert_eq!(p.get(sq("e2")), None);
    assert_eq!(p.get(sq("e4")), Some((Color::White, Piece::Pawn)));
    assert!(!p.move_piece(sq("a1"), sq("a2")), "casa de origem vazia");
    assert_eq!(p.remove(sq("e4")), Some((Color::White, Piece::Pawn)));
    assert!(p.is_empty());
}

#[test]
fn the_study_countdown_follows_the_injected_clock() {
    let run = PositionMemoryRun::start(&exercise(10), no_aids(), 5_000);
    assert_eq!(run.phase(), PositionPhase::Study);
    assert_eq!(run.study_remaining_ms(5_000), 10_000);
    assert_eq!(run.study_remaining_ms(9_000), 6_000);
    assert!(!run.study_finished(14_999));
    assert!(run.study_finished(15_000));
    assert_eq!(run.study_remaining_ms(99_000), 0);
}

#[test]
fn the_board_only_accepts_edits_during_recall() {
    let mut run = PositionMemoryRun::start(&exercise(10), no_aids(), 0);
    assert!(
        !run.place(sq("a1"), Color::White, Piece::Rook),
        "ainda estudando"
    );
    run.begin_recall(4_000);
    assert_eq!(run.phase(), PositionPhase::Recall);
    assert!(run.placed().is_empty(), "tabuleiro esvaziado");
    assert!(run.place(sq("a1"), Color::White, Piece::Rook));
    assert!(run.move_piece(sq("a1"), sq("a2")));
    assert!(run.remove(sq("a2")));
    assert!(!run.remove(sq("a2")), "nada a remover");
    run.place(sq("a1"), Color::White, Piece::Rook);
    run.clear();
    assert!(run.placed().is_empty());
}

#[test]
fn checking_judges_and_closes_the_run() {
    let mut run = PositionMemoryRun::start(&exercise(10), no_aids(), 1_000);
    run.begin_recall(11_000);
    for (s, c, p) in target().iter() {
        run.place(s, c, p);
    }
    let percent = run.check(31_000).percent();
    assert_eq!(percent, 100);
    assert_eq!(run.phase(), PositionPhase::Done);
    assert!(
        !run.place(sq("d4"), Color::White, Piece::Pawn),
        "conferido: sem edição"
    );

    let summary = run.finish(31_000);
    assert_eq!(summary.kind, ExerciseKind::MemoryPosition);
    assert_eq!(summary.result, RunResult::Correct);
    let mem = summary.position_memory.unwrap();
    assert_eq!((mem.correct, mem.total, mem.percent), (9, 9, 100));
    assert_eq!(mem.study_ms, 10_000);
    assert_eq!(mem.recall_ms, 20_000);
    assert_eq!(summary.duration_ms, 30_000);
}

#[test]
fn an_imperfect_reconstruction_is_incorrect_but_keeps_the_counts() {
    let mut run = PositionMemoryRun::start(&exercise(10), no_aids(), 0);
    run.begin_recall(10_000);
    run.place(sq("a1"), Color::White, Piece::Rook);
    run.place(sq("e4"), Color::White, Piece::Pawn);
    run.check(15_000);
    let summary = run.finish(15_000);
    assert_eq!(summary.result, RunResult::Incorrect);
    let mem = summary.position_memory.unwrap();
    assert_eq!(
        (mem.correct, mem.missing, mem.extra, mem.wrong_piece),
        (1, 8, 1, 0)
    );
    assert_eq!(mem.percent, 10, "1 certa em 9 + 1 sobrando");
}

#[test]
fn giving_up_without_checking_judges_what_is_on_the_board() {
    let mut run = PositionMemoryRun::start(&exercise(10), no_aids(), 0);
    run.begin_recall(10_000);
    run.place(sq("a1"), Color::White, Piece::Rook);
    let summary = run.finish(12_000);
    assert_eq!(summary.result, RunResult::Incorrect);
    assert_eq!(summary.position_memory.unwrap().correct, 1);
}

#[test]
fn giving_up_during_study_records_zero_percent() {
    let run = PositionMemoryRun::start(&exercise(10), no_aids(), 0);
    let summary = run.finish(3_000);
    let mem = summary.position_memory.unwrap();
    assert_eq!((mem.percent, mem.recall_ms), (0, 0));
    assert_eq!(mem.study_ms, 3_000, "só o tempo que de fato olhou");
}
