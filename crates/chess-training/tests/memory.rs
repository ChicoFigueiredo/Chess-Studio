#![allow(clippy::unwrap_used)]
//! Exercício de memória de sequência (TRN-04, DEC-45): apresentação, reprodução e
//! julgamento Lance a Lance.

use chess_core::Square;
use chess_game::{Command, Game, Timestamp};
use chess_training::{
    AidsRecord, Catalog, ExerciseKind, Feedback, MemoryPhase, MemoryRun, Metrics, PlyResult,
    Review, RunResult, RunStatus, Solution,
};

const ITALIAN: &str = "memoria-italiana-001";
const LINE: [&str; 6] = ["e2e4", "e7e5", "g1f3", "b8c6", "f1c4", "f8c5"];

fn no_aids() -> AidsRecord {
    AidsRecord::new(&[], Feedback::Immediate)
}

fn start(id: &str) -> (MemoryRun, Game) {
    let catalog = Catalog::bundled().unwrap();
    let ex = catalog.get(id).unwrap();
    (MemoryRun::start(ex, no_aids(), 0), ex.new_game().unwrap())
}

fn recall(id: &str) -> (MemoryRun, Game) {
    let (mut run, game) = start(id);
    run.begin_recall();
    (run, game)
}

fn release(run: &mut MemoryRun, game: &mut Game, uci: &str, at: u64) {
    let sq = |s: &str| -> Square { s.parse().unwrap() };
    game.handle(
        Command::Release {
            from: sq(&uci[0..2]),
            to: sq(&uci[2..4]),
        },
        Timestamp::mono(at),
    )
    .unwrap();
    run.drive(game, at);
}

#[test]
fn bundled_catalog_has_memory_exercises_of_3_to_8_moves() {
    let catalog = Catalog::bundled().unwrap();
    let memory: Vec<_> = catalog
        .iter()
        .filter(|e| e.kind == ExerciseKind::Memory)
        .collect();
    assert!(memory.len() >= 3);
    for ex in memory {
        let Solution::Memorize { line, speed_ms } = &ex.solution else {
            panic!("{} não é memorize", ex.id);
        };
        assert!((3..=8).contains(&line.len()), "{}", ex.id);
        assert!(*speed_ms > 0);
    }
}

#[test]
fn a_line_outside_3_to_8_moves_or_illegal_is_rejected() {
    let json = |line: &str| {
        format!(
            r#"{{"format":1,"exercises":[{{"id":"m","kind":"memory","title":"t",
            "fen":"rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 0 1","objective":"o",
            "solution":{{"type":"memorize","line":{line},"speed_ms":1000}},
            "explanation":"e","version":1}}]}}"#
        )
    };
    assert!(Catalog::from_json(&json(r#"["e2e4","e7e5"]"#)).is_err());
    assert!(
        Catalog::from_json(&json(
            r#"["e2e4","e7e5","g1f3","b8c6","f1c4","f8c5","c2c3","g8f6","d2d4"]"#
        ))
        .is_err()
    );
    assert!(Catalog::from_json(&json(r#"["e2e4","e7e5","e1e5"]"#)).is_err());
    assert!(Catalog::from_json(&json(r#"["e2e4","e7e5","g1f3"]"#)).is_ok());
}

#[test]
fn presentation_steps_through_the_line_then_ends_and_recall_restarts_at_the_beginning() {
    let (mut run, game) = start(ITALIAN);
    assert_eq!(run.phase(), MemoryPhase::Presentation);
    let speed = u64::from(run.speed_ms());
    assert_eq!(run.presentation_ply(0), 0, "começa na posição inicial");
    assert_eq!(run.presentation_ply(speed), 1);
    assert_eq!(run.presentation_ply(speed * 3 + 1), 3);
    assert_eq!(run.presentation_ply(speed * 99), 6, "para no último Lance");
    assert!(!run.presentation_finished(speed * 6));
    assert!(run.presentation_finished(speed * 7));
    run.begin_recall();
    assert_eq!(run.phase(), MemoryPhase::Recall);
    assert!(game.snapshot().history.is_empty(), "volta ao início");
}

#[test]
fn moves_are_ignored_while_presenting() {
    let (mut run, mut game) = start(ITALIAN);
    release(&mut run, &mut game, "e2e4", 10);
    assert_eq!(run.plies(), &[PlyResult::Pending; 6]);
}

#[test]
fn a_perfect_recall_of_both_sides_hits_every_ply() {
    let (mut run, mut game) = recall(ITALIAN);
    for (i, uci) in LINE.iter().enumerate() {
        assert_eq!(run.status(), RunStatus::InProgress);
        release(&mut run, &mut game, uci, 1_000 + i as u64 * 100);
    }
    assert_eq!(run.status(), RunStatus::Solved);
    assert_eq!(run.plies(), &[PlyResult::Hit; 6]);
    assert_eq!(run.first_error(), None);
    let summary = run.finish(&game, 9_000);
    assert_eq!(summary.result, RunResult::Correct);
    assert!(summary.first_try());
    assert_eq!(summary.duration_ms, 9_000);
    let memory = summary.memory.clone().unwrap();
    assert_eq!(memory.plies, vec![PlyResult::Hit; 6]);
    assert_eq!(memory.first_error, None);
    assert_eq!(memory.hits(), 6);
}

#[test]
fn a_wrong_move_is_a_miss_at_that_ply_is_undone_and_the_first_error_is_kept() {
    let (mut run, mut game) = recall(ITALIAN);
    release(&mut run, &mut game, "e2e4", 100);
    release(&mut run, &mut game, "e7e5", 200);
    release(&mut run, &mut game, "b1c3", 300); // esperado g1f3
    assert_eq!(
        game.snapshot().history.len(),
        2,
        "o Lance errado foi desfeito"
    );
    assert_eq!(run.first_error(), Some(2));
    assert_eq!(run.plies()[2], PlyResult::Miss);
    assert_eq!(run.errors().len(), 1);
    // Tentar de novo e seguir: o Lance 3 continua errado (primeira tentativa).
    for (i, uci) in LINE[2..].iter().enumerate() {
        release(&mut run, &mut game, uci, 400 + i as u64 * 100);
    }
    assert_eq!(run.status(), RunStatus::Solved);
    assert_eq!(
        run.plies(),
        &[
            PlyResult::Hit,
            PlyResult::Hit,
            PlyResult::Miss,
            PlyResult::Hit,
            PlyResult::Hit,
            PlyResult::Hit
        ]
    );
    let summary = run.finish(&game, 2_000);
    assert_eq!(summary.result, RunResult::Correct, "completou a sequência");
    assert!(!summary.first_try());
    assert_eq!(summary.memory.as_ref().unwrap().first_error, Some(2));
    assert_eq!(summary.memory.as_ref().unwrap().hits(), 5);
}

#[test]
fn an_illegal_move_counts_as_an_error_at_the_current_ply() {
    let (mut run, mut game) = recall(ITALIAN);
    let sq = |s: &str| -> Square { s.parse().unwrap() };
    game.handle(
        Command::Release {
            from: sq("e2"),
            to: sq("e5"),
        },
        Timestamp::mono(50),
    )
    .unwrap();
    run.drive(&mut game, 50);
    assert_eq!(run.plies()[0], PlyResult::Miss);
    assert_eq!(run.first_error(), Some(0));
}

#[test]
fn show_again_goes_back_to_presentation_and_is_counted() {
    let (mut run, mut game) = recall(ITALIAN);
    release(&mut run, &mut game, "e2e4", 100);
    run.show_again(5_000);
    assert_eq!(run.phase(), MemoryPhase::Presentation);
    assert_eq!(run.presentation_ply(5_000), 0);
    assert_eq!(run.presentation_ply(5_000 + u64::from(run.speed_ms())), 1);
    // A reprodução recomeça do início, numa Partida nova; o que já foi julgado fica.
    let mut fresh = Catalog::bundled()
        .unwrap()
        .get(ITALIAN)
        .unwrap()
        .new_game()
        .unwrap();
    run.begin_recall();
    release(&mut run, &mut fresh, "e2e4", 6_000);
    assert_eq!(run.plies()[0], PlyResult::Hit);
    let summary = run.finish(&fresh, 7_000);
    assert_eq!(summary.memory.unwrap().replays, 1);
}

#[test]
fn giving_up_is_incorrect_and_leaves_unplayed_plies_pending() {
    let (mut run, mut game) = recall(ITALIAN);
    release(&mut run, &mut game, "e2e4", 100);
    release(&mut run, &mut game, "d7d6", 200); // erro no Lance 2
    let summary = run.finish(&game, 1_000);
    assert_eq!(summary.result, RunResult::Incorrect);
    let memory = summary.memory.as_ref().unwrap();
    assert_eq!(memory.plies[0], PlyResult::Hit);
    assert_eq!(memory.plies[1], PlyResult::Miss);
    assert_eq!(memory.plies[2], PlyResult::Pending);
    assert_eq!(memory.first_error, Some(1));
}

#[test]
fn review_lists_each_ply_san_and_result_and_metrics_count_plies() {
    let catalog = Catalog::bundled().unwrap();
    let ex = catalog.get(ITALIAN).unwrap();
    let (mut run, mut game) = recall(ITALIAN);
    release(&mut run, &mut game, "e2e4", 100);
    release(&mut run, &mut game, "d7d6", 200);
    for (i, uci) in LINE[1..].iter().enumerate() {
        release(&mut run, &mut game, uci, 300 + i as u64 * 100);
    }
    let summary = run.finish(&game, 2_000);
    let review = Review::new(ex, &summary);
    let memory = review.memory.as_ref().unwrap();
    let san: Vec<_> = memory.plies.iter().map(|p| p.san.as_str()).collect();
    assert_eq!(san, ["e4", "e5", "Nf3", "Nc6", "Bc4", "Bc5"]);
    assert_eq!(memory.plies[1].result, PlyResult::Miss);
    assert_eq!(memory.first_error, Some(1));
    assert_eq!(review.solution_san, san);

    let perfect = {
        let (mut r, mut g) = recall(ITALIAN);
        for (i, uci) in LINE.iter().enumerate() {
            release(&mut r, &mut g, uci, 100 * (i as u64 + 1));
        }
        r.finish(&g, 1_000)
    };
    let metrics = Metrics::from_runs(&[summary, perfect]);
    assert_eq!(metrics.total.memory_plies, 12);
    assert_eq!(metrics.total.memory_hits, 5 + 6);
    assert_eq!(metrics.total.correct, 2, "Exercícios completos");
    assert_eq!(metrics.by_kind[&ExerciseKind::Memory].memory_hits, 11);
}
