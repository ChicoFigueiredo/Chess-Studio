#![allow(clippy::unwrap_used)]
//! Desfazer depois do fim reabre a Partida também no banco (#115).

mod common;

use chess_game::{Command, GameSetup, PolicyKind, Timestamp};
use chess_store::{GameMetadata, GameQuery, Store};
use common::{TempDb, play};

const FOOLS_MATE: [(&str, &str); 4] = [("f2", "f3"), ("e7", "e5"), ("g2", "g4"), ("d8", "h4")];

fn mated_and_undone(store: &mut Store) -> chess_store::GameId {
    let (mut game, events) = play(GameSetup::standard(PolicyKind::Assisted), &FOOLS_MATE);
    let id = store
        .create_game(game.setup(), &GameMetadata::default())
        .unwrap();
    store.append_events(id, &events).unwrap();
    assert!(store.latest_unfinished_game().unwrap().is_none());
    let undo = game
        .handle(Command::Undo, Timestamp::with_wall(9000, 1_700_000_009_000))
        .unwrap();
    store.append_events(id, &undo).unwrap();
    id
}

#[test]
fn desfazer_o_mate_reabre_a_partida_no_banco() {
    let db = TempDb::new("undo-reopen");
    let id = {
        let mut store = Store::open(db.path()).unwrap();
        mated_and_undone(&mut store)
    };
    let store = Store::open(db.path()).unwrap();
    assert_eq!(store.latest_unfinished_game().unwrap(), Some(id));
    let summary = &store.list_games(&GameQuery::default()).unwrap()[0];
    assert!(!summary.finished);
    assert!(summary.outcome.is_none());
}

#[test]
fn bancos_com_o_flag_errado_sao_corrigidos_ao_migrar() {
    let db = TempDb::new("undo-heal");
    let id = {
        let mut store = Store::open(db.path()).unwrap();
        let id = mated_and_undone(&mut store);
        // Simula o banco gravado pela versão com o bug.
        let conn = rusqlite::Connection::open(db.path()).unwrap();
        conn.execute(
            "UPDATE games SET finished = 1, outcome_json = '{\"x\":1}' WHERE id = ?1",
            [id.0],
        )
        .unwrap();
        // Volta ao esquema v8 (anterior à correção): sem as tabelas das migrações seguintes.
        conn.execute_batch(
            "DROP TABLE fide_ratings; DROP TABLE connections; DROP TABLE opening_moves; DROP TABLE manual_scoresheet_check_entries; DROP TABLE manual_scoresheet_checks;
             DROP TABLE manual_scoresheet_entries; DROP TABLE manual_scoresheets;
             DROP TABLE imported_game_origins;
             DROP INDEX imported_games_fingerprint; DROP INDEX imported_games_remote;
             DROP INDEX imported_games_provider; DROP INDEX imported_games_eco;
             DROP INDEX imported_games_pace;
             ALTER TABLE imported_games DROP COLUMN provider;
             ALTER TABLE imported_games DROP COLUMN import_mode;
             ALTER TABLE imported_games DROP COLUMN retrieved_at_ms;
             ALTER TABLE imported_games DROP COLUMN license;
             ALTER TABLE imported_games DROP COLUMN reference_url;
             ALTER TABLE imported_games DROP COLUMN remote_id;
             ALTER TABLE imported_games DROP COLUMN fingerprint;
             ALTER TABLE imported_games DROP COLUMN site;
             ALTER TABLE imported_games DROP COLUMN white_elo;
             ALTER TABLE imported_games DROP COLUMN black_elo;
             ALTER TABLE imported_games DROP COLUMN time_control;
             ALTER TABLE imported_games DROP COLUMN eco;
             ALTER TABLE imported_games DROP COLUMN pace;
             DROP INDEX exercises_pack; ALTER TABLE exercises DROP COLUMN origin;
             ALTER TABLE exercises DROP COLUMN pack; ALTER TABLE exercises DROP COLUMN hidden;",
        )
        .unwrap();
        conn.pragma_update(None, "user_version", 8).unwrap();
        id
    };
    let store = Store::open(db.path()).unwrap();
    assert_eq!(store.latest_unfinished_game().unwrap(), Some(id));
}

#[test]
fn partida_que_termina_continua_terminada() {
    let mut store = Store::open_in_memory().unwrap();
    let (game, events) = play(GameSetup::standard(PolicyKind::Assisted), &FOOLS_MATE);
    let id = store
        .create_game(game.setup(), &GameMetadata::default())
        .unwrap();
    store.append_events(id, &events).unwrap();
    let summary = &store.list_games(&GameQuery::default()).unwrap()[0];
    assert!(summary.finished);
    assert!(summary.outcome.is_some());
}
