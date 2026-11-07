#![allow(clippy::unwrap_used)]
//! Métricas ampliadas (US-26): período, jogador, Perfil de treino, sem Auxílios,
//! memória e finais.

use chess_game::{EndReason, GameResult, Outcome, PolicyKind};
use chess_training::{
    Aid, AidsRecord, DatedRun, Difficulty, ErrorCause, ExerciseKind, Feedback, Period,
    PositionMemorySummary, ProfileStep, RunError, RunResult, RunSummary, TrainingProfile,
    by_period, by_player, by_training_profile, civil_date, day_of, report,
};

const DAY_MS: i64 = 86_400_000;
/// 2024-01-01, uma segunda-feira.
const MONDAY: i64 = 19_723;

struct Spec {
    day: i64,
    kind: ExerciseKind,
    id: &'static str,
    correct: bool,
    errors: usize,
    aids: bool,
    stalemated: bool,
    player: i64,
    position_pct: Option<u32>,
}

fn spec(day: i64, kind: ExerciseKind, id: &'static str, correct: bool) -> Spec {
    Spec {
        day,
        kind,
        id,
        correct,
        errors: 0,
        aids: false,
        stalemated: false,
        player: 1,
        position_pct: None,
    }
}

fn dated(s: &Spec) -> DatedRun {
    let aids = if s.aids {
        AidsRecord::new(&[Aid::Destinations], Feedback::Immediate)
    } else {
        AidsRecord::new(&[], Feedback::Immediate)
    };
    DatedRun {
        at_ms: s.day * DAY_MS + 12 * 3_600_000,
        player: s.player,
        summary: RunSummary {
            exercise_id: s.id.into(),
            exercise_version: 1,
            kind: s.kind,
            policy: PolicyKind::Assisted,
            result: if s.correct {
                RunResult::Correct
            } else {
                RunResult::Incorrect
            },
            errors: (0..s.errors)
                .map(|_| RunError {
                    cause: ErrorCause::OffObjective { uci: "a1a2".into() },
                    fen: "8/8/8/8/8/8/8/8 w - - 0 1".into(),
                })
                .collect(),
            moves: Vec::new(),
            aids,
            duration_ms: 1_000,
            outcome: s.stalemated.then_some(Outcome {
                result: GameResult::Draw,
                reason: EndReason::Stalemate,
            }),
            final_fen: "8/8/8/8/8/8/8/8 w - - 0 1".into(),
            started_at_ms: 0,
            session: 0,
            memory: None,
            position_memory: s.position_pct.map(|percent| PositionMemorySummary {
                correct: percent,
                wrong_piece: 0,
                missing: 100 - percent,
                extra: 0,
                total: 100,
                percent,
                study_ms: 10_000,
                recall_ms: 20_000,
            }),
        },
    }
}

fn runs(specs: &[Spec]) -> Vec<DatedRun> {
    specs.iter().map(dated).collect()
}

#[test]
fn days_and_dates_round_trip_across_leap_years() {
    assert_eq!(civil_date(0), (1970, 1, 1));
    assert_eq!(civil_date(MONDAY), (2024, 1, 1));
    assert_eq!(civil_date(11_016), (2000, 2, 29));
    assert_eq!(civil_date(-1), (1969, 12, 31));
    assert_eq!(day_of(MONDAY * DAY_MS, 0), MONDAY);
    // 22h UTC de domingo já é segunda em UTC+3, mas continua domingo em UTC.
    let sunday_night = MONDAY * DAY_MS - 2 * 3_600_000;
    assert_eq!(day_of(sunday_night, 0), MONDAY - 1);
    assert_eq!(day_of(sunday_night, 180), MONDAY);
    assert_eq!(day_of(sunday_night, -180), MONDAY - 1);
}

#[test]
fn by_day_groups_chronologically_and_skips_empty_days() {
    let data = runs(&[
        spec(MONDAY + 2, ExerciseKind::Check, "xeque-001", true),
        spec(MONDAY, ExerciseKind::Check, "xeque-001", false),
        spec(MONDAY, ExerciseKind::Mate, "mate-em-1-001", true),
    ]);
    let days = by_period(&data, Period::Day, 0);
    assert_eq!(days.len(), 2);
    assert_eq!(days[0].start_day, MONDAY);
    assert_eq!((days[0].tally.runs, days[0].tally.correct), (2, 1));
    assert_eq!(days[1].start_day, MONDAY + 2);
    assert_eq!(days[1].tally.runs, 1);
}

#[test]
fn by_week_starts_on_monday() {
    let data = runs(&[
        spec(MONDAY - 1, ExerciseKind::Check, "a", true), // domingo da semana anterior
        spec(MONDAY, ExerciseKind::Check, "a", true),
        spec(MONDAY + 6, ExerciseKind::Check, "a", false), // domingo da mesma semana
        spec(MONDAY + 7, ExerciseKind::Check, "a", true),
    ]);
    let weeks = by_period(&data, Period::Week, 0);
    let starts: Vec<_> = weeks.iter().map(|w| w.start_day).collect();
    assert_eq!(starts, [MONDAY - 7, MONDAY, MONDAY + 7]);
    let runs_per_week: Vec<_> = weeks.iter().map(|w| w.tally.runs).collect();
    assert_eq!(runs_per_week, [1, 2, 1]);
}

#[test]
fn the_timezone_moves_a_run_to_the_local_day() {
    let data = runs(&[spec(MONDAY, ExerciseKind::Check, "a", true)]);
    // 12h UTC: em UTC-13 ainda é 23h de domingo? Não: 12h - 13h = 23h do dia anterior.
    let local = by_period(&data, Period::Day, -13 * 60);
    assert_eq!(local[0].start_day, MONDAY - 1);
}

#[test]
fn by_player_keeps_each_profile_apart() {
    let mut other = spec(MONDAY, ExerciseKind::Check, "a", true);
    other.player = 2;
    let data = runs(&[spec(MONDAY, ExerciseKind::Check, "a", false), other]);
    let by = by_player(&data);
    assert_eq!(by[&1].correct, 0);
    assert_eq!(by[&2].correct, 1);
}

#[test]
fn by_training_profile_counts_only_the_profiles_exercises() {
    let profile = |id: &str, ids: &[&str]| TrainingProfile {
        id: id.into(),
        name: id.into(),
        description: String::new(),
        steps: ids
            .iter()
            .map(|e| ProfileStep {
                exercise: (*e).into(),
                difficulty: Difficulty::Easy,
            })
            .collect(),
    };
    let data = runs(&[
        spec(MONDAY, ExerciseKind::Check, "xeque-001", true),
        spec(MONDAY, ExerciseKind::Mate, "mate-em-1-001", true),
        spec(MONDAY, ExerciseKind::Mate, "mate-em-1-001", false),
    ]);
    let by = by_training_profile(
        &data,
        &[
            profile("basico", &["xeque-001", "mate-em-1-001"]),
            profile("mates", &["mate-em-1-001"]),
            profile("vazio", &["roque-001"]),
        ],
    );
    let get = |id: &str| by.iter().find(|(n, _)| n == id).unwrap().1;
    assert_eq!(get("basico").runs, 3);
    assert_eq!((get("mates").runs, get("mates").correct), (2, 1));
    assert_eq!(get("vazio").runs, 0, "Perfil sem Resoluções aparece zerado");
}

#[test]
fn completed_without_aids_is_a_share_of_the_exercises_solved() {
    let mut with_aids = spec(MONDAY, ExerciseKind::Check, "a", true);
    with_aids.aids = true;
    let data = runs(&[
        spec(MONDAY, ExerciseKind::Check, "a", true),
        spec(MONDAY, ExerciseKind::Check, "b", true),
        spec(MONDAY, ExerciseKind::Check, "c", true),
        with_aids,
        spec(MONDAY, ExerciseKind::Check, "d", false), // não concluído: fora da conta
    ]);
    assert_eq!(report(&data).completed_without_aids_pct, Some(75));
    assert_eq!(report(&[]).completed_without_aids_pct, None);
    assert_eq!(
        report(&runs(&[spec(MONDAY, ExerciseKind::Check, "d", false)])).completed_without_aids_pct,
        None
    );
}

#[test]
fn position_memory_average_uses_each_runs_percent() {
    let mut a = spec(MONDAY, ExerciseKind::MemoryPosition, "p1", false);
    a.position_pct = Some(50);
    let mut b = spec(MONDAY, ExerciseKind::MemoryPosition, "p2", true);
    b.position_pct = Some(100);
    let mut c = spec(MONDAY, ExerciseKind::MemoryPosition, "p3", false);
    c.position_pct = Some(81);
    let r = report(&runs(&[a, b, c]));
    assert_eq!(r.position_memory_pct, Some(77)); // (50 + 100 + 81) / 3 = 77
    assert_eq!(r.sequence_memory_pct, None);
}

#[test]
fn sequence_memory_average_comes_from_memory_runs_only() {
    let data = runs(&[
        spec(MONDAY, ExerciseKind::Memory, "m1", true),
        spec(MONDAY, ExerciseKind::Memory, "m2", false),
        spec(MONDAY, ExerciseKind::Check, "x", true),
    ]);
    let r = report(&data);
    assert_eq!(r.sequence_memory_pct, Some(50));
    assert_eq!(r.position_memory_pct, None);
}

#[test]
fn endgames_count_stalemate_and_mate_with_stalemate_mistakes() {
    let mut missed_stalemate = spec(MONDAY, ExerciseKind::Stalemate, "afogamento-001", true);
    missed_stalemate.errors = 2; // acertou depois de errar: erro de afogamento
    let mut accidental = spec(MONDAY, ExerciseKind::Mate, "mate-em-1-001", false);
    accidental.stalemated = true; // queria mate e afogou
    accidental.errors = 1;
    let data = runs(&[
        spec(MONDAY, ExerciseKind::Stalemate, "afogamento-002", true),
        missed_stalemate,
        accidental,
        spec(MONDAY, ExerciseKind::Mate, "mate-em-1-002", true),
        spec(MONDAY, ExerciseKind::Castling, "roque-001", false), // fora de finais
    ]);
    let e = report(&data).endgames;
    assert_eq!((e.runs, e.correct, e.errors), (4, 3, 3));
    assert_eq!(e.stalemate_errors, 2);
    assert_eq!(e.accuracy_pct(), Some(75));
    assert_eq!(report(&[]).endgames.accuracy_pct(), None);
}

#[test]
fn the_overall_tally_is_the_same_as_the_plain_metrics() {
    let data = runs(&[
        spec(MONDAY, ExerciseKind::Check, "a", true),
        spec(MONDAY + 1, ExerciseKind::Mate, "b", false),
    ]);
    let r = report(&data);
    assert_eq!(
        (r.total.runs, r.total.correct, r.total.incorrect),
        (2, 1, 1)
    );
}

#[test]
fn sequence_memory_uses_the_hits_per_ply_when_recorded() {
    use chess_training::{MemorySummary, PlyResult};
    let mut run = dated(&spec(MONDAY, ExerciseKind::Memory, "m1", true));
    run.summary.memory = Some(MemorySummary {
        plies: vec![
            PlyResult::Hit,
            PlyResult::Hit,
            PlyResult::Hit,
            PlyResult::Miss,
        ],
        first_error: Some(3),
        replays: 0,
    });
    assert_eq!(report(&[run]).sequence_memory_pct, Some(75));
}
