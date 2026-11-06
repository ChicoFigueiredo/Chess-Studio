#![allow(clippy::unwrap_used)]
//! DB-06: um banco da versão anterior do esquema abre na atual sem perder dados.

mod common;

use chess_game::{GameSetup, PolicyKind};
use chess_store::{GameMetadata, LATEST_SCHEMA_VERSION, Store, StoreError, migrations};
use common::{TempDb, play};
use serde_json::json;

/// Cria um banco "como o app da versão anterior o deixaria": esquema N-1 mais
/// dados gravados por essa versão (SQL cru, sem passar pelo código atual).
fn legacy_db(db: &TempDb, game_json: &str, events: &[(i64, &str, &str)]) {
    let mut conn = rusqlite::Connection::open(db.path()).unwrap();
    migrations::migrate_to(&mut conn, LATEST_SCHEMA_VERSION - 1).unwrap();
    assert_eq!(
        migrations::current_version(&conn).unwrap(),
        LATEST_SCHEMA_VERSION - 1
    );
    conn.execute(
        "INSERT INTO ui_preferences (profile_id, key, value_json) VALUES (1, 'board.view', '{\"camera\":\"black\"}')",
        [],
    )
    .unwrap();
    conn.execute(
        "INSERT INTO games (id, profile_id, policy, start_fen, setup_json, created_at_ms, updated_at_ms, session_open)
         VALUES (7, 1, 'assisted', 'x', ?1, 1, 2, 0)",
        [game_json],
    )
    .unwrap();
    for (seq, kind, payload) in events {
        conn.execute(
            "INSERT INTO game_events (game_id, seq, version, kind, payload_json) VALUES (7, ?1, 1, ?2, ?3)",
            rusqlite::params![seq, kind, payload],
        )
        .unwrap();
    }
}

#[test]
fn opening_a_previous_version_database_migrates_and_preserves_data() {
    let db = TempDb::new("migrate");
    let setup = GameSetup::standard(PolicyKind::Assisted);
    let (game, events) = play(setup.clone(), &[("e2", "e4"), ("e7", "e5")]);
    let rows: Vec<(i64, String, String)> = events
        .iter()
        .enumerate()
        .map(|(i, e)| {
            let payload = serde_json::to_value(e).unwrap();
            (
                i as i64,
                payload["kind"]["type"].as_str().unwrap().to_owned(),
                payload.to_string(),
            )
        })
        .collect();
    let rows: Vec<(i64, &str, &str)> = rows.iter().map(|(a, b, c)| (*a, b.as_str(), c.as_str())).collect();
    legacy_db(&db, &serde_json::to_string(&setup).unwrap(), &rows);

    let store = Store::open(db.path()).unwrap();
    assert_eq!(store.schema_version().unwrap(), LATEST_SCHEMA_VERSION);

    // Dados antigos intactos.
    let loaded = store.load_game(chess_store::GameId(7)).unwrap();
    assert_eq!(loaded.snapshot(), game.snapshot());
    let p = store.default_profile().unwrap();
    assert_eq!(
        store.get_pref(p, "board.view").unwrap(),
        Some(json!({"camera": "black"}))
    );
    // Tabelas novas disponíveis.
    let conn = rusqlite::Connection::open(db.path()).unwrap();
    for table in ["exercises", "exercise_runs", "engine_installations", "positions"] {
        conn.query_row(&format!("SELECT COUNT(*) FROM {table}"), [], |r| r.get::<_, i64>(0))
            .unwrap();
    }
    // E o banco migrado continua gravável.
    drop(conn);
    let mut store = store;
    let new = store.create_game(&setup, &GameMetadata::default()).unwrap();
    assert!(new.0 > 7);
}

#[test]
fn database_from_a_newer_app_is_refused_with_a_clear_error() {
    let db = TempDb::new("newer");
    drop(Store::open(db.path()).unwrap());
    let conn = rusqlite::Connection::open(db.path()).unwrap();
    conn.pragma_update(None, "user_version", LATEST_SCHEMA_VERSION + 1).unwrap();
    drop(conn);
    let err = Store::open(db.path()).unwrap_err();
    assert!(matches!(err, StoreError::SchemaTooNew { .. }));
    assert!(err.to_string().contains("atualize"));
}

#[test]
fn a_failing_migration_rolls_back_and_keeps_the_previous_version() {
    let mut conn = rusqlite::Connection::open_in_memory().unwrap();
    migrations::migrate_to(&mut conn, 1).unwrap();
    // Sabota a migração 2: uma tabela homônima já existe, o DDL falha no meio.
    conn.execute_batch("CREATE TABLE engine_installations (x)").unwrap();
    assert!(migrations::migrate(&mut conn).is_err());
    assert_eq!(migrations::current_version(&conn).unwrap(), 1);
    let leftovers: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM sqlite_master WHERE name = 'exercises'",
            [],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(leftovers, 0, "a migração parcial foi desfeita");
}
