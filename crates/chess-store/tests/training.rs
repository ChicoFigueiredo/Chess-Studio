#![allow(clippy::unwrap_used)]
//! Exercícios e Resoluções no SQLite (TRN-02, TRN-05, DB-06).

mod common;

use chess_core::Square;
use chess_game::{Command, GameSetup, PolicyKind, Timestamp};
use chess_store::{GameMetadata, GameQuery, RunQuery, Store, StoreError};
use chess_training::{Aid, AidsRecord, Catalog, ExerciseKind, ExerciseRun, Feedback, RunSummary};
use common::{TempDb, play};

fn sq(s: &str) -> Square {
    s.parse().unwrap()
}

/// Resolve (ou não) um Exercício do catálogo e devolve o registro.
fn run(id: &str, moves: &[&str], aids: AidsRecord, session: u64) -> RunSummary {
    let catalog = Catalog::bundled().unwrap();
    let ex = catalog.get(id).unwrap();
    let mut game = ex.new_game().unwrap();
    let mut run = ExerciseRun::start(ex, aids, 0);
    for (i, uci) in moves.iter().enumerate() {
        game.handle(
            Command::Release {
                from: sq(&uci[0..2]),
                to: sq(&uci[2..4]),
            },
            Timestamp::mono(100 * (i as u64 + 1)),
        )
        .unwrap();
        run.drive(&mut game, 100 * (i as u64 + 1));
    }
    run.finish(&game, 1_000).in_session(session)
}

fn none() -> AidsRecord {
    AidsRecord::new(&[], Feedback::Immediate)
}

fn seeded() -> Store {
    let mut store = Store::open_in_memory().unwrap();
    let catalog = Catalog::bundled().unwrap();
    let all: Vec<_> = catalog.iter().cloned().collect();
    store.upsert_exercises(&all).unwrap();
    store
}

fn record(store: &mut Store, summary: &RunSummary) -> chess_store::RunId {
    let p = store.default_profile().unwrap();
    let id = store
        .start_run(p, &summary.exercise_id, summary.session)
        .unwrap();
    store.finish_run(id, summary).unwrap();
    id
}

#[test]
fn catalog_is_loaded_into_sqlite_and_upsert_is_idempotent_and_versioned() {
    let mut store = Store::open_in_memory().unwrap();
    let catalog = Catalog::bundled().unwrap();
    let all: Vec<_> = catalog.iter().cloned().collect();
    let first = store.upsert_exercises(&all).unwrap();
    assert_eq!(first.inserted, all.len());
    let again = store.upsert_exercises(&all).unwrap();
    assert_eq!((again.inserted, again.updated), (0, 0));
    assert_eq!(store.list_exercises().unwrap(), all);

    // Versão maior substitui; menor é ignorada (o banco não volta atrás).
    let mut newer = all[0].clone();
    newer.version = 2;
    newer.objective = "novo objetivo".into();
    assert_eq!(store.upsert_exercises(&[newer.clone()]).unwrap().updated, 1);
    let mut older = newer.clone();
    older.version = 1;
    older.objective = "velho".into();
    assert_eq!(store.upsert_exercises(&[older]).unwrap().updated, 0);
    let listed = store.list_exercises().unwrap();
    assert_eq!(
        listed.iter().find(|e| e.id == newer.id).unwrap().objective,
        "novo objetivo"
    );
}

/// TRN-02: a Resolução original continua intacta depois de repetir o Exercício.
#[test]
fn a_repeated_run_is_a_new_row_and_the_original_is_not_rewritten() {
    let mut store = seeded();
    let first = run("afogamento-001", &["e6e8"], none(), 1); // mate: erro
    let first_id = record(&mut store, &first);
    let second = run("afogamento-001", &["e6f7"], none(), 1);
    let second_id = record(&mut store, &second);
    assert_ne!(first_id, second_id);

    let runs = store
        .list_runs(&RunQuery {
            exercise: Some("afogamento-001".into()),
            ..RunQuery::default()
        })
        .unwrap();
    assert_eq!(runs.len(), 2);
    let stored_first = runs.iter().find(|r| r.id == first_id).unwrap();
    assert_eq!(stored_first.summary, first);
    assert_eq!(stored_first.summary.errors.len(), 1);
    let stored_second = runs.iter().find(|r| r.id == second_id).unwrap();
    assert_eq!(stored_second.summary, second);
    assert!(stored_second.summary.outcome.is_some());
}

#[test]
fn a_finished_run_cannot_be_finished_again_and_unfinished_runs_are_not_listed() {
    let mut store = seeded();
    let p = store.default_profile().unwrap();
    let summary = run("roque-001", &["e1g1"], none(), 1);
    let id = store.start_run(p, "roque-001", 1).unwrap();
    assert!(store.list_runs(&RunQuery::default()).unwrap().is_empty());
    store.finish_run(id, &summary).unwrap();
    assert!(matches!(
        store.finish_run(id, &summary),
        Err(StoreError::RunAlreadyFinished(_))
    ));
    assert!(matches!(
        store.start_run(p, "nao-existe", 1),
        Err(StoreError::ExerciseNotFound(_))
    ));
    // O registro precisa ser do mesmo Exercício da Resolução iniciada.
    let other = store.start_run(p, "mate-em-1-001", 1).unwrap();
    assert!(store.finish_run(other, &summary).is_err());
}

/// TRN-03 na persistência: Auxílios usados e desligados sobrevivem a fechar e reabrir.
#[test]
fn aids_survive_reopening_the_database() {
    let db = TempDb::new("training_aids");
    let aids = AidsRecord::new(&[Aid::Destinations], Feedback::ReviewOnly);
    let summary = run("roque-001", &["e1g1"], aids.clone(), 3);
    {
        let mut store = Store::open(db.path()).unwrap();
        store
            .upsert_exercises(&[Catalog::bundled()
                .unwrap()
                .get("roque-001")
                .unwrap()
                .clone()])
            .unwrap();
        record(&mut store, &summary);
    }
    let store = Store::open(db.path()).unwrap();
    let runs = store.list_runs(&RunQuery::default()).unwrap();
    assert_eq!(runs[0].summary.aids, aids);
    assert_eq!(runs[0].summary.session, 3);
}

/// TRN-05: erros e acertos por tipo e sessão, sem tocar Partidas competitivas.
#[test]
fn metrics_by_kind_and_session_never_touch_competitive_games() {
    let mut store = seeded();
    // Uma Partida com resultado (mate do bobo), gravada antes de qualquer treino.
    let (game, events) = play(
        GameSetup::standard(PolicyKind::Assisted),
        &[("f2", "f3"), ("e7", "e5"), ("g2", "g4"), ("d8", "h4")],
    );
    let id = store
        .create_game(game.setup(), &GameMetadata::default())
        .unwrap();
    store.append_events(id, &events).unwrap();
    let games_before = store.list_games(&GameQuery::default()).unwrap();
    assert!(games_before[0].outcome.is_some());

    record(&mut store, &run("roque-001", &["e1e5", "e1g1"], none(), 1));
    record(
        &mut store,
        &run(
            "roque-001",
            &["e1g1"],
            AidsRecord::new(&[Aid::Check], Feedback::Immediate),
            1,
        ),
    );
    record(&mut store, &run("afogamento-001", &[], none(), 2)); // desistência
    record(&mut store, &run("afogamento-001", &["e6f7"], none(), 2));

    let m = store.metrics(&RunQuery::default()).unwrap();
    assert_eq!(
        (m.total.runs, m.total.correct, m.total.incorrect),
        (4, 3, 1)
    );
    assert_eq!(m.total.errors, 1);
    assert_eq!(m.total.correct_without_aids, 2);
    assert_eq!(m.by_kind[&ExerciseKind::Castling].correct, 2);
    assert_eq!(m.by_kind[&ExerciseKind::Stalemate].incorrect, 1);
    assert_eq!(m.by_session[&1].runs, 2);
    assert_eq!(m.by_session[&2].runs, 2);

    let only_s2 = store
        .metrics(&RunQuery {
            session: Some(2),
            ..RunQuery::default()
        })
        .unwrap();
    assert_eq!(only_s2.total.runs, 2);

    // Nenhuma Partida criada ou alterada pelo treino.
    assert_eq!(
        store.list_games(&GameQuery::default()).unwrap(),
        games_before
    );
}

/// DB-06: o banco da versão anterior (com Exercícios e Resoluções antigas) abre e
/// continua gravável.
#[test]
fn a_pre_training_api_database_migrates_keeping_old_exercise_rows() {
    let db = TempDb::new("training_legacy");
    {
        let mut conn = rusqlite::Connection::open(db.path()).unwrap();
        chess_store::migrations::migrate_to(&mut conn, 2).unwrap();
        conn.execute(
            "INSERT INTO exercises (id, kind, initial_fen, solution_json, version, created_at_ms)
             VALUES (5, 'mate', 'x', '{}', 1, 0)",
            [],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO exercise_runs (exercise_id, profile_id, correct, hints_used, duration_ms, created_at_ms)
             VALUES (5, 1, 1, 0, 10, 0)",
            [],
        )
        .unwrap();
    }
    let mut store = Store::open(db.path()).unwrap();
    // As linhas antigas continuam no banco, mas não aparecem como Exercícios do catálogo.
    assert!(store.list_exercises().unwrap().is_empty());
    assert!(store.list_runs(&RunQuery::default()).unwrap().is_empty());
    let conn = rusqlite::Connection::open(db.path()).unwrap();
    let n: i64 = conn
        .query_row("SELECT COUNT(*) FROM exercise_runs", [], |r| r.get(0))
        .unwrap();
    assert_eq!(n, 1);
    drop(conn);
    store
        .upsert_exercises(&[Catalog::bundled()
            .unwrap()
            .get("roque-001")
            .unwrap()
            .clone()])
        .unwrap();
    assert_eq!(store.list_exercises().unwrap().len(), 1);
}
