#![allow(clippy::unwrap_used)]
//! Resolução, revisão e métricas (TRN-01, TRN-02, TRN-03, TRN-05, BDD-05).

mod common;

use chess_game::{EndReason, GameResult};
use chess_training::{
    Aid, AidsRecord, ErrorCause, ExerciseKind, ExerciseRun, Feedback, Metrics, Review, RunResult,
    RunStatus,
};
use common::{exercise, no_aids, release, start};

const STALEMATE: &str = "afogamento-001";

/// TRN-01 + BDD-05: provocar o afogamento termina em empate e a revisão explica
/// por que não é xeque-mate.
#[test]
fn stalemate_exercise_ends_in_a_draw_and_review_explains_it_is_not_checkmate() {
    let (mut run, mut game) = start(STALEMATE, no_aids());
    release(&mut run, &mut game, "e6f7", 4_000);
    assert_eq!(run.status(), RunStatus::Solved);
    let summary = run.finish(&game, 5_000);
    let outcome = summary.outcome.unwrap();
    assert_eq!(outcome.result, GameResult::Draw);
    assert_eq!(outcome.reason, EndReason::Stalemate);
    assert_eq!(summary.result, RunResult::Correct);
    assert_eq!(summary.duration_ms, 5_000);

    let review = Review::new(&exercise(STALEMATE), &summary);
    let why = review.not_checkmate.expect("explicação afogamento x mate");
    assert!(!why.king_in_check, "o rei não está em xeque");
    assert_eq!(why.legal_moves, 0, "sem Lances legais");
    assert!(review.explanation.to_lowercase().contains("xeque-mate"));
    assert_eq!(review.solution_san, ["Qf7"], "o Lance que a pessoa fez");
}

/// Dar mate no lugar de afogar é um erro, o Lance é desfeito e a Posição volta.
#[test]
fn a_move_that_misses_the_objective_is_an_error_and_is_reverted() {
    let ex = exercise(STALEMATE);
    let (mut run, mut game) = start(STALEMATE, no_aids());
    release(&mut run, &mut game, "e6e8", 1_000); // dá mate (Qe8#): não é o objetivo
    assert_eq!(run.status(), RunStatus::InProgress);
    assert_eq!(game.snapshot().position.to_fen(), ex.fen);
    assert_eq!(run.errors().len(), 1);
    assert!(matches!(
        &run.errors()[0].cause,
        ErrorCause::OffObjective { uci } if uci == "e6e8"
    ));
    assert_eq!(run.errors()[0].fen, ex.fen, "registra a posição do erro");
}

/// TRN-02 + BDD-05: repetir parte da posição original e não reescreve a Resolução anterior.
#[test]
fn repeating_after_an_error_starts_from_the_original_position_and_keeps_the_first_run() {
    let ex = exercise(STALEMATE);
    let (mut run1, mut game1) = start(STALEMATE, no_aids());
    release(&mut run1, &mut game1, "e6e8", 1_000);
    let first = run1.finish(&game1, 2_000);
    assert_eq!(first.result, RunResult::Incorrect);
    assert_eq!(first.errors.len(), 1);
    let first_before = first.clone();

    let mut game2 = ex.new_game().unwrap();
    assert_eq!(game2.snapshot().position.to_fen(), ex.fen);
    let mut run2 = ExerciseRun::start(&ex, no_aids(), 10_000);
    release(&mut run2, &mut game2, "e6f7", 14_000);
    let second = run2.finish(&game2, 15_000);

    assert_eq!(second.result, RunResult::Correct);
    assert_eq!(second.outcome.unwrap().reason, EndReason::Stalemate);
    assert_eq!(second.duration_ms, 5_000);
    assert!(second.errors.is_empty());
    assert_eq!(first, first_before, "a Resolução original não muda");
}

/// Lance ilegal conta como erro; o Exercício segue possível.
#[test]
fn illegal_moves_are_errors_and_the_run_can_still_be_solved() {
    let (mut run, mut game) = start("roque-001", no_aids());
    release(&mut run, &mut game, "e1e5", 100); // ilegal
    assert!(matches!(
        run.errors()[0].cause,
        ErrorCause::IllegalMove { .. }
    ));
    release(&mut run, &mut game, "e1g1", 200);
    assert_eq!(run.status(), RunStatus::Solved);
    let summary = run.finish(&game, 300);
    assert_eq!(summary.moves, ["e1g1"]);
    assert_eq!(summary.errors.len(), 1);
}

#[test]
fn mate_in_one_and_promotion_objectives() {
    let (mut run, mut game) = start("mate-em-1-001", no_aids());
    release(&mut run, &mut game, "a1a7", 1); // legal, não é mate
    assert_eq!(run.errors().len(), 1);
    release(&mut run, &mut game, "a1a8", 2);
    assert_eq!(run.status(), RunStatus::Solved);

    let (mut run, mut game) = start("promocao-001", no_aids());
    release(&mut run, &mut game, "a7a8q", 1);
    assert_eq!(run.status(), RunStatus::Solved);
}

/// "Recusar o Lance ilegal X": tentar X e ver a recusa cumpre o objetivo.
#[test]
fn refusing_the_illegal_pawn_move_solves_the_exercise_without_an_error() {
    let (mut run, mut game) = start("peao-ilegal-001", no_aids());
    release(&mut run, &mut game, "d2d3", 1);
    assert_eq!(run.status(), RunStatus::Solved);
    assert!(run.errors().is_empty());
    let summary = run.finish(&game, 9);
    let review = Review::new(&exercise("peao-ilegal-001"), &summary);
    assert!(review.not_checkmate.is_none());
}

/// Desistir encerra a Resolução como erro; a revisão ainda mostra a solução.
#[test]
fn giving_up_ends_as_incorrect_with_the_solution_in_the_review() {
    let (run, game) = start("mate-em-1-001", no_aids());
    let summary = run.finish(&game, 7_000);
    assert_eq!(summary.result, RunResult::Incorrect);
    let review = Review::new(&exercise("mate-em-1-001"), &summary);
    assert_eq!(review.solution_san, ["Ra8#"]);
}

/// TRN-03: a Resolução guarda quais Auxílios estavam ligados e quais desligados,
/// e a política de feedback.
#[test]
fn aids_used_and_disabled_are_recorded() {
    let aids = AidsRecord::new(&[Aid::Destinations, Aid::Check], Feedback::AfterAttempt);
    assert_eq!(aids.used, [Aid::Destinations, Aid::Check]);
    assert_eq!(aids.off, [Aid::Ghosts, Aid::SelectedPiece, Aid::LastMove]);
    assert!(!aids.without_aids());

    let (mut run, mut game) = start("roque-001", no_aids());
    assert!(run.aids().without_aids());
    release(&mut run, &mut game, "e1g1", 1);
    let summary = run.finish(&game, 2);
    assert!(summary.aids.without_aids());
    assert_eq!(
        summary.aids.off,
        Aid::ALL,
        "todos desligados ficam registrados"
    );
    assert_eq!(summary.aids.feedback, Feedback::Immediate);

    // Ligar um Auxílio no meio da Resolução conta como usado.
    let (mut run, game) = start("roque-001", no_aids());
    run.note_aids(&AidsRecord::new(&[Aid::Ghosts], Feedback::ReviewOnly));
    let summary = run.finish(&game, 2);
    assert_eq!(summary.aids.used, [Aid::Ghosts]);
    assert!(!summary.aids.off.contains(&Aid::Ghosts));
    assert_eq!(summary.aids.feedback, Feedback::ReviewOnly);
}

/// TRN-05: acertos, erros e Exercícios sem Auxílios por tipo e por sessão.
#[test]
fn metrics_count_hits_errors_and_no_aid_runs_by_kind_and_session() {
    let solve = |uci: &str, aids: AidsRecord, session: u64, slip: bool| {
        let (mut run, mut game) = start("roque-001", aids);
        if slip {
            release(&mut run, &mut game, "e1e5", 1); // Lance ilegal
        }
        release(&mut run, &mut game, uci, 2);
        run.finish(&game, 3).in_session(session)
    };
    let aids_on = AidsRecord::new(&[Aid::Destinations], Feedback::Immediate);
    let giveup = {
        let (run, game) = start(STALEMATE, no_aids());
        run.finish(&game, 3).in_session(2)
    };
    let runs = vec![
        solve("e1g1", no_aids(), 1, false),
        solve("e1g1", aids_on, 1, true),
        giveup,
    ];
    let m = Metrics::from_runs(&runs);
    assert_eq!(
        (m.total.runs, m.total.correct, m.total.incorrect),
        (3, 2, 1)
    );
    assert_eq!(m.total.errors, 1);
    assert_eq!(m.total.correct_without_aids, 1);
    assert_eq!(m.total.first_try_correct, 1);
    assert_eq!(m.by_kind[&ExerciseKind::Castling].runs, 2);
    assert_eq!(m.by_kind[&ExerciseKind::Stalemate].incorrect, 1);
    assert_eq!(m.by_session[&1].correct, 2);
    assert_eq!(m.by_session[&2].runs, 1);
    assert_eq!(m.by_exercise["roque-001"].runs, 2);
}
