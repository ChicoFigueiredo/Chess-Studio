#![allow(clippy::unwrap_used)]
//! Importação PGN no store (#48): árvore de variações no SQLite, DB-06 preservado.

mod common;

use chess_store::{ImportedQuery, LATEST_SCHEMA_VERSION, Store, StoreError, migrations};
use common::TempDb;

const PGN: &str = "\
[Event \"Estudo A\"]
[White \"Ana\"]
[Black \"Beto\"]
[Result \"1-0\"]

1. e4 e5 (1... c5 {Siciliana} 2. Nf3 (2. c3 $5)) 2. Nf3 {desenvolve} Nc6 3. Bb5 1-0

[Event \"Estudo B\"]
[White \"Carla\"]
[Black \"Davi\"]

1. d4 d5 2. c4 *

[Event \"Quebrada\"]

1. e4 e5 2. Ke7 *
";

#[test]
fn import_pgn_grava_a_arvore_e_relata_os_erros_sem_perder_as_boas() {
    let mut store = Store::open_in_memory().unwrap();
    let report = store.import_pgn(PGN, Some("estudos.pgn")).unwrap();
    assert_eq!(report.imported.len(), 2);
    assert_eq!(report.errors.len(), 1);
    assert_eq!(report.errors[0].game, 3);
    assert!(report.errors[0].to_string().contains("Ke7"));

    let list = store.list_imported(&ImportedQuery::default()).unwrap();
    assert_eq!(list.len(), 2);
    // Mais recentes primeiro.
    assert_eq!(list[0].event.as_deref(), Some("Estudo B"));
    let a = &list[1];
    assert_eq!(
        (a.white.as_deref(), a.black.as_deref()),
        (Some("Ana"), Some("Beto"))
    );
    assert_eq!(a.result, "1-0");
    assert_eq!(a.ply_count, 8); // 5 da linha principal + 3 das variações
    assert_eq!(a.variation_count, 2);
    assert_eq!(a.source.as_deref(), Some("estudos.pgn"));
}

#[test]
fn load_imported_devolve_a_mesma_arvore_com_comentarios_e_nags() {
    let mut store = Store::open_in_memory().unwrap();
    let report = store.import_pgn(PGN, None).unwrap();
    let game = store.load_imported(report.imported[0]).unwrap();
    assert_eq!(game.mainline_sans(), ["e4", "e5", "Nf3", "Nc6", "Bb5"]);
    assert_eq!(game.moves[1].variations[0][0].comments, ["Siciliana"]);
    assert_eq!(game.moves[1].variations[0][1].variations[0][0].nags, [5]);
    assert_eq!(game.moves[2].comments, ["desenvolve"]);
    assert_eq!(game, chess_pgn::parse_pgn(PGN_FIRST).unwrap().remove(0));
}

const PGN_FIRST: &str = "\
[Event \"Estudo A\"]
[White \"Ana\"]
[Black \"Beto\"]
[Result \"1-0\"]

1. e4 e5 (1... c5 {Siciliana} 2. Nf3 (2. c3 $5)) 2. Nf3 {desenvolve} Nc6 3. Bb5 1-0
";

#[test]
fn filtro_por_texto_e_exclusao() {
    let mut store = Store::open_in_memory().unwrap();
    let report = store.import_pgn(PGN, None).unwrap();
    let ana = store
        .list_imported(&ImportedQuery {
            text: Some("ana".into()),
            ..Default::default()
        })
        .unwrap();
    assert_eq!(ana.len(), 1);
    store.delete_imported(report.imported[0]).unwrap();
    assert_eq!(
        store
            .list_imported(&ImportedQuery::default())
            .unwrap()
            .len(),
        1
    );
    assert!(matches!(
        store.load_imported(report.imported[0]),
        Err(StoreError::ImportedGameNotFound(_))
    ));
}

#[test]
fn migracao_preserva_partidas_e_preferencias_e_cria_a_tabela_de_importadas() {
    let db = TempDb::new("pgn-mig");
    {
        let mut conn = rusqlite::Connection::open(db.path()).unwrap();
        migrations::migrate_to(&mut conn, LATEST_SCHEMA_VERSION - 1).unwrap();
        conn.execute(
            "INSERT INTO ui_preferences (profile_id, key, value_json) VALUES (1, 'k', '1')",
            [],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO games (id, profile_id, policy, start_fen, setup_json, created_at_ms, updated_at_ms, session_open)
             VALUES (7, 1, 'assisted', 'x', '{}', 1, 2, 0)",
            [],
        )
        .unwrap();
    }
    let mut store = Store::open(db.path()).unwrap();
    assert_eq!(store.schema_version().unwrap(), LATEST_SCHEMA_VERSION);
    assert_eq!(store.list_games(&Default::default()).unwrap().len(), 1);
    let profile = store.default_profile().unwrap();
    assert_eq!(
        store.get_pref(profile, "k").unwrap(),
        Some(serde_json::json!(1))
    );
    assert_eq!(store.import_pgn(PGN, None).unwrap().imported.len(), 2);
}

#[test]
fn importar_sobrevive_a_reabertura_do_banco() {
    let db = TempDb::new("pgn-reopen");
    let id = {
        let mut store = Store::open(db.path()).unwrap();
        store.import_pgn(PGN, None).unwrap().imported[0]
    };
    let store = Store::open(db.path()).unwrap();
    assert_eq!(
        store.load_imported(id).unwrap().tag("Event"),
        Some("Estudo A")
    );
}
