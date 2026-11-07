#![allow(clippy::unwrap_used)]
//! Perfis de treino no SQLite (US-27): avanço registrado, Perfil ativo e Resoluções
//! datadas para a evolução (US-26).

use chess_game::PolicyKind;
use chess_store::{RunQuery, Store};
use chess_training::{
    AidsRecord, Catalog, ExerciseKind, Feedback, ProfileSet, RunResult, RunSummary,
};

fn setup() -> (Store, ProfileSet) {
    let mut store = Store::open_in_memory().unwrap();
    let catalog = Catalog::bundled().unwrap();
    let all: Vec<_> = catalog.iter().cloned().collect();
    store.upsert_exercises(&all).unwrap();
    (store, ProfileSet::bundled(&catalog).unwrap())
}

fn summary(id: &str, kind: ExerciseKind, correct: bool) -> RunSummary {
    RunSummary {
        exercise_id: id.into(),
        exercise_version: 1,
        kind,
        policy: PolicyKind::Assisted,
        result: if correct {
            RunResult::Correct
        } else {
            RunResult::Incorrect
        },
        errors: Vec::new(),
        moves: Vec::new(),
        aids: AidsRecord::new(&[], Feedback::Immediate),
        duration_ms: 1,
        outcome: None,
        final_fen: "8/8/8/8/8/8/8/8 w - - 0 1".into(),
        started_at_ms: 0,
        session: 0,
        memory: None,
        position_memory: None,
    }
}

fn finish(store: &mut Store, s: &RunSummary) {
    let p = store.default_profile().unwrap();
    let run = store.start_run(p, &s.exercise_id, 1).unwrap();
    store.finish_run(run, s).unwrap();
}

#[test]
fn progress_starts_empty_and_records_each_step_once() {
    let (mut store, _) = setup();
    let player = store.default_profile().unwrap();
    assert!(
        store
            .profile_progress(player, "memoria")
            .unwrap()
            .is_empty()
    );
    assert!(
        store
            .record_profile_step(player, "memoria", "memoria-posicao-001")
            .unwrap()
    );
    assert!(
        !store
            .record_profile_step(player, "memoria", "memoria-posicao-001")
            .unwrap(),
        "o segundo registro do mesmo passo não duplica"
    );
    let done = store.profile_progress(player, "memoria").unwrap();
    assert_eq!(done.len(), 1);
    assert!(done.contains("memoria-posicao-001"));
    assert!(
        store
            .profile_progress(player, "regras-basicas")
            .unwrap()
            .is_empty(),
        "cada Perfil de treino tem o seu avanço"
    );
}

#[test]
fn a_correct_run_advances_every_profile_that_contains_the_exercise() {
    let (mut store, profiles) = setup();
    let player = store.default_profile().unwrap();
    // mate-em-1-001 está em "regras-basicas" e em "finais-afogamento".
    let s = summary("mate-em-1-001", ExerciseKind::Mate, true);
    let mut advanced = store.advance_profiles(player, &profiles, &s).unwrap();
    advanced.sort();
    assert_eq!(advanced, ["finais-afogamento", "regras-basicas"]);
    assert!(
        store
            .profile_progress(player, "regras-basicas")
            .unwrap()
            .contains("mate-em-1-001")
    );
}

#[test]
fn a_wrong_run_or_a_foreign_exercise_advances_nothing() {
    let (mut store, profiles) = setup();
    let player = store.default_profile().unwrap();
    let wrong = summary("xeque-001", ExerciseKind::Check, false);
    assert!(
        store
            .advance_profiles(player, &profiles, &wrong)
            .unwrap()
            .is_empty()
    );
    // "roque-004" não está em nenhum Perfil embutido.
    let foreign = summary("roque-004", ExerciseKind::Castling, true);
    assert!(
        store
            .advance_profiles(player, &profiles, &foreign)
            .unwrap()
            .is_empty()
    );
}

#[test]
fn the_active_profile_is_remembered_per_player_and_can_be_cleared() {
    let (mut store, _) = setup();
    let player = store.default_profile().unwrap();
    assert_eq!(store.active_training_profile(player).unwrap(), None);
    store
        .set_active_training_profile(player, Some("memoria"))
        .unwrap();
    assert_eq!(
        store.active_training_profile(player).unwrap().as_deref(),
        Some("memoria")
    );
    store
        .set_active_training_profile(player, Some("regras-basicas"))
        .unwrap();
    assert_eq!(
        store.active_training_profile(player).unwrap().as_deref(),
        Some("regras-basicas")
    );
    store.set_active_training_profile(player, None).unwrap();
    assert_eq!(store.active_training_profile(player).unwrap(), None);
}

#[test]
fn dated_runs_carry_the_timestamp_and_the_player_for_the_evolution() {
    let (mut store, _) = setup();
    finish(&mut store, &summary("xeque-001", ExerciseKind::Check, true));
    finish(
        &mut store,
        &summary("xeque-002", ExerciseKind::Check, false),
    );
    let dated = store.dated_runs(&RunQuery::default()).unwrap();
    assert_eq!(dated.len(), 2);
    assert!(dated.iter().all(|d| d.at_ms > 1_700_000_000_000));
    let player = store.default_profile().unwrap();
    assert!(dated.iter().all(|d| d.player == player.0));
    let report = chess_training::report(&dated);
    assert_eq!((report.total.runs, report.total.correct), (2, 1));
}

#[test]
fn the_new_tables_arrive_by_migration_without_touching_existing_data() {
    let (mut store, _) = setup();
    finish(&mut store, &summary("xeque-001", ExerciseKind::Check, true));
    // Reabrir/migrar não perde Resoluções (DB-06): o avanço é tabela à parte.
    let player = store.default_profile().unwrap();
    store
        .record_profile_step(player, "memoria", "memoria-posicao-001")
        .unwrap();
    assert_eq!(store.list_runs(&RunQuery::default()).unwrap().len(), 1);
}
