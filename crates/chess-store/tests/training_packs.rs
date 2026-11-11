#![allow(clippy::unwrap_used)]
//! Pacotes de Exercícios no SQLite: coluna de Origem, remoção de pacote e Resoluções.

use chess_store::{RunQuery, Store, migrations};
use chess_training::{
    AidsRecord, Catalog, Exercise, ExerciseRun, Feedback, PuzzleFilter, import_lichess_csv,
};
use std::io::Cursor;
use std::sync::atomic::AtomicBool;

const BEFORE: &str = "6k1/1p3ppp/8/8/8/8/5PPP/R5K1 b - - 0 1";

fn pack_exercises(prefix: &str, n: usize, pack: &str) -> Vec<Exercise> {
    let mut text = String::new();
    for i in 0..n {
        text += &format!(
            "{prefix}{i},{BEFORE},b7b6 a1a8,1500,75,90,10,mate fork,https://lichess.org/{prefix}{i},\n"
        );
    }
    let cancel = AtomicBool::new(false);
    let mut list = import_lichess_csv(
        Cursor::new(text),
        &PuzzleFilter::default(),
        &cancel,
        &mut |_| {},
    )
    .unwrap()
    .0;
    for e in &mut list {
        e.pack = Some(pack.to_owned());
    }
    list
}

fn seeded() -> Store {
    let mut store = Store::open_in_memory().unwrap();
    let bundled: Vec<_> = Catalog::bundled().unwrap().iter().cloned().collect();
    store.upsert_exercises(&bundled).unwrap();
    store
}

/// Resolve o Exercício `id` de um pacote (qualquer Lance basta para ter a Resolução).
fn solve(store: &mut Store, exercise: &Exercise) {
    let profile = store.default_profile().unwrap();
    let run_id = store.start_run(profile, &exercise.id, 1).unwrap();
    let game = exercise.new_game().unwrap();
    let run = ExerciseRun::start(exercise, AidsRecord::new(&[], Feedback::Immediate), 0);
    let summary = run.finish(&game, 500).in_session(1);
    store.finish_run(run_id, &summary).unwrap();
}

#[test]
fn factory_exercises_are_marked_bundled_and_packs_carry_their_origin() {
    let mut store = seeded();
    let pack = pack_exercises("a", 2, "lichess-1");
    store.upsert_exercises(&pack).unwrap();

    let factory = Catalog::bundled()
        .unwrap()
        .iter()
        .next()
        .unwrap()
        .id
        .clone();
    assert_eq!(
        store.exercise_origin(&factory).unwrap().as_deref(),
        Some("bundled")
    );
    assert_eq!(
        store.exercise_origin("lichess-a0").unwrap().as_deref(),
        Some("lichess")
    );

    let packs = store.packs().unwrap();
    assert_eq!(packs.len(), 1);
    assert_eq!(
        (packs[0].pack.as_str(), packs[0].exercises),
        ("lichess-1", 2)
    );

    // A definição completa volta com tema, rating, Origem e pacote.
    let back = store
        .list_exercises()
        .unwrap()
        .into_iter()
        .find(|e| e.id == "lichess-a0")
        .unwrap();
    assert_eq!(back, pack[0]);
}

#[test]
fn removing_a_pack_removes_only_its_exercises_and_keeps_the_runs() {
    let mut store = seeded();
    let keep = pack_exercises("k", 2, "outro");
    let gone = pack_exercises("g", 3, "lichess-1");
    store.upsert_exercises(&keep).unwrap();
    store.upsert_exercises(&gone).unwrap();
    let factory_count = Catalog::bundled().unwrap().len();

    solve(&mut store, &gone[0]);
    let runs_before = store.list_runs(&RunQuery::default()).unwrap().len();
    assert_eq!(runs_before, 1);

    let report = store.remove_pack("lichess-1").unwrap();
    assert_eq!((report.deleted, report.hidden), (2, 1));

    // Só os Exercícios do pacote saem da lista; os de fábrica e os do outro pacote ficam.
    let ids: Vec<String> = store
        .list_exercises()
        .unwrap()
        .into_iter()
        .map(|e| e.id)
        .collect();
    assert_eq!(ids.len(), factory_count + 2);
    assert!(ids.iter().all(|id| !id.starts_with("lichess-g")));
    assert!(ids.iter().any(|id| id == "lichess-k0"));
    assert!(store.packs().unwrap().iter().all(|p| p.pack == "outro"));

    // A Resolução já feita segue no histórico e nas métricas.
    assert_eq!(
        store.list_runs(&RunQuery::default()).unwrap().len(),
        runs_before
    );
    assert_eq!(store.metrics(&RunQuery::default()).unwrap().total.runs, 1);
    // Mas o Exercício removido não pode mais ser resolvido.
    let profile = store.default_profile().unwrap();
    assert!(store.start_run(profile, "lichess-g0", 2).is_err());
    assert!(store.start_run(profile, "lichess-g1", 2).is_err());
}

#[test]
fn reinstalling_a_removed_pack_brings_back_the_hidden_exercise_with_its_runs() {
    let mut store = seeded();
    let pack = pack_exercises("g", 2, "lichess-1");
    store.upsert_exercises(&pack).unwrap();
    solve(&mut store, &pack[0]);
    store.remove_pack("lichess-1").unwrap();

    store.upsert_exercises(&pack).unwrap();
    assert_eq!(store.packs().unwrap()[0].exercises, 2);
    let profile = store.default_profile().unwrap();
    store.start_run(profile, "lichess-g0", 3).unwrap();
    // As duas Resoluções (a antiga e a nova, esta ainda aberta) apontam o mesmo Exercício.
    assert_eq!(store.list_runs(&RunQuery::default()).unwrap().len(), 1);
}

#[test]
fn migration_14_applies_on_a_v13_database_and_preserves_everything() {
    let mut conn = rusqlite::Connection::open_in_memory().unwrap();
    migrations::migrate_to(&mut conn, 13).unwrap();
    assert_eq!(migrations::current_version(&conn).unwrap(), 13);
    conn.execute(
        "INSERT INTO exercises (kind, initial_fen, solution_json, version, created_at_ms, slug,
                                definition_json)
         VALUES ('mate', '8/8/8/8/8/8/8/8 w - - 0 1', '{}', 1, 0, 'velho-1', '{}')",
        [],
    )
    .unwrap();
    conn.execute(
        "INSERT INTO exercise_runs (exercise_id, profile_id, correct, hints_used, duration_ms,
                                    created_at_ms, finished, session)
         VALUES (1, 1, 1, 0, 10, 5, 1, 7)",
        [],
    )
    .unwrap();
    conn.execute(
        "INSERT INTO imported_games (profile_id, source, result, start_fen, ply_count,
                                     variation_count, tree_json, imported_at_ms)
         VALUES (1, 'a.pgn', '1-0', 'x', 3, 0, '{}', 99)",
        [],
    )
    .unwrap();
    migrations::migrate(&mut conn).unwrap();
    assert_eq!(migrations::current_version(&conn).unwrap(), 14);
    let (origin, pack, hidden): (String, Option<String>, i64) = conn
        .query_row(
            "SELECT origin, pack, hidden FROM exercises WHERE slug = 'velho-1'",
            [],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
        )
        .unwrap();
    assert_eq!((origin.as_str(), pack, hidden), ("bundled", None, 0));
    let runs: i64 = conn
        .query_row("SELECT COUNT(*) FROM exercise_runs", [], |r| r.get(0))
        .unwrap();
    assert_eq!(runs, 1, "a Resolução antiga segue lá");
    // O que a 13 trouxe continua: a Origem estruturada da Partida importada.
    let (source, retrieved): (String, i64) = conn
        .query_row(
            "SELECT source, retrieved_at_ms FROM imported_games",
            [],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )
        .unwrap();
    assert_eq!((source.as_str(), retrieved), ("a.pgn", 99));
}
