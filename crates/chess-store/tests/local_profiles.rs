#![allow(clippy::unwrap_used)]
//! Vários perfis locais (US-29): criar, renomear, trocar, excluir; cada perfil com suas
//! preferências, Partidas e Resoluções, e nada vaza de um para o outro.

mod common;

use chess_game::{GameSetup, PolicyKind};
use chess_store::{GameMetadata, GameQuery, LATEST_SCHEMA_VERSION, Store, StoreError, migrations};
use common::{TempDb, play};
use serde_json::json;

fn setup() -> GameSetup {
    GameSetup::standard(PolicyKind::Assisted)
}

#[test]
fn a_fresh_database_has_one_profile_that_is_active() {
    let store = Store::open_in_memory().unwrap();
    let all = store.list_profiles().unwrap();
    assert_eq!(all.len(), 1);
    assert_eq!(store.active_profile().unwrap(), all[0].id);
    assert_eq!(store.default_profile().unwrap(), all[0].id);
}

#[test]
fn profiles_are_created_with_trimmed_unique_names() {
    let store = Store::open_in_memory().unwrap();
    let ana = store.create_profile("  Ana  ").unwrap();
    assert_eq!(store.profile(ana).unwrap().name, "Ana");
    assert!(matches!(
        store.create_profile("Ana"),
        Err(StoreError::ProfileNameTaken(_))
    ));
    assert!(matches!(
        store.create_profile("   "),
        Err(StoreError::InvalidProfileName)
    ));
    assert!(matches!(
        store.create_profile(&"x".repeat(41)),
        Err(StoreError::InvalidProfileName)
    ));
    assert_eq!(store.list_profiles().unwrap().len(), 2);
}

#[test]
fn renaming_keeps_the_data_and_rejects_duplicates() {
    let store = Store::open_in_memory().unwrap();
    let ana = store.create_profile("Ana").unwrap();
    let beto = store.create_profile("Beto").unwrap();
    store.set_pref(ana, "quality", &json!("Ultra")).unwrap();
    store.rename_profile(ana, "Ana Maria").unwrap();
    assert_eq!(store.profile(ana).unwrap().name, "Ana Maria");
    assert_eq!(
        store.get_pref(ana, "quality").unwrap(),
        Some(json!("Ultra"))
    );
    assert!(matches!(
        store.rename_profile(beto, "Ana Maria"),
        Err(StoreError::ProfileNameTaken(_))
    ));
    // Renomear para o próprio nome é permitido.
    store.rename_profile(beto, "Beto").unwrap();
    assert!(matches!(
        store.rename_profile(chess_store::ProfileId(999), "Z"),
        Err(StoreError::ProfileNotFound(_))
    ));
}

#[test]
fn switching_changes_the_active_profile_and_it_survives_reopening() {
    let db = TempDb::new("active");
    let ana = {
        let store = Store::open(db.path()).unwrap();
        let ana = store.create_profile("Ana").unwrap();
        store.set_active_profile(ana).unwrap();
        assert_eq!(store.active_profile().unwrap(), ana);
        ana
    };
    let store = Store::open(db.path()).unwrap();
    assert_eq!(store.active_profile().unwrap(), ana);
    assert_eq!(store.default_profile().unwrap(), ana);
}

#[test]
fn preferences_games_and_unfinished_games_are_isolated_per_profile() {
    let mut store = Store::open_in_memory().unwrap();
    let first = store.default_profile().unwrap();
    let ana = store.create_profile("Ana").unwrap();
    store.set_pref(first, "k", &json!(1)).unwrap();
    store.set_pref(ana, "k", &json!(2)).unwrap();
    assert_eq!(store.get_pref(first, "k").unwrap(), Some(json!(1)));
    assert_eq!(store.get_pref(ana, "k").unwrap(), Some(json!(2)));

    let g1 = store
        .create_game_for(first, &setup(), &GameMetadata::default())
        .unwrap();
    let g2 = store
        .create_game_for(ana, &setup(), &GameMetadata::default())
        .unwrap();
    let only = |p| {
        store
            .list_games(&GameQuery {
                profile: Some(p),
                ..GameQuery::default()
            })
            .unwrap()
            .into_iter()
            .map(|g| g.id)
            .collect::<Vec<_>>()
    };
    assert_eq!(only(first), vec![g1]);
    assert_eq!(only(ana), vec![g2]);
    assert_eq!(store.latest_unfinished_game_for(first).unwrap(), Some(g1));
    assert_eq!(store.latest_unfinished_game_for(ana).unwrap(), Some(g2));
}

#[test]
fn new_games_belong_to_the_active_profile() {
    let mut store = Store::open_in_memory().unwrap();
    let ana = store.create_profile("Ana").unwrap();
    store.set_active_profile(ana).unwrap();
    let id = store
        .create_game(&setup(), &GameMetadata::default())
        .unwrap();
    let listed = store.list_games(&GameQuery::default()).unwrap();
    assert_eq!(listed[0].id, id);
    assert_eq!(listed[0].profile, ana);
}

#[test]
fn deleting_a_profile_removes_only_its_data() {
    let mut store = Store::open_in_memory().unwrap();
    let first = store.default_profile().unwrap();
    let ana = store.create_profile("Ana").unwrap();
    store.set_pref(ana, "k", &json!(2)).unwrap();
    store.set_pref(first, "k", &json!(1)).unwrap();
    let (game, events) = play(setup(), &[("e2", "e4")]);
    let gid = store
        .create_game_for(ana, game.setup(), &GameMetadata::default())
        .unwrap();
    store.append_events(gid, &events).unwrap();
    let keep = store
        .create_game_for(first, &setup(), &GameMetadata::default())
        .unwrap();
    store.set_active_profile(ana).unwrap();

    store.delete_profile(ana).unwrap();

    assert_eq!(store.list_profiles().unwrap().len(), 1);
    assert!(matches!(
        store.load_game(gid),
        Err(StoreError::GameNotFound(_))
    ));
    assert!(store.load_game(keep).is_ok());
    assert_eq!(store.get_pref(ana, "k").unwrap(), None);
    assert_eq!(store.get_pref(first, "k").unwrap(), Some(json!(1)));
    // O perfil ativo excluído cede a vez ao que sobrou.
    assert_eq!(store.active_profile().unwrap(), first);
}

#[test]
fn the_last_profile_cannot_be_deleted() {
    let store = Store::open_in_memory().unwrap();
    let only = store.default_profile().unwrap();
    assert!(matches!(
        store.delete_profile(only),
        Err(StoreError::LastProfile)
    ));
    assert_eq!(store.list_profiles().unwrap().len(), 1);
}

/// DB-06: um banco da versão anterior (um único perfil, com dados) abre na atual sem perder
/// nada, e o perfil existente continua o ativo.
#[test]
fn previous_version_database_keeps_its_profile_and_data() {
    let db = TempDb::new("profiles-migration");
    {
        let mut conn = rusqlite::Connection::open(db.path()).unwrap();
        migrations::migrate_to(&mut conn, LATEST_SCHEMA_VERSION - 1).unwrap();
        conn.execute(
            "INSERT INTO ui_preferences (profile_id, key, value_json) VALUES (1, 'quality', '\"Ultra\"')",
            [],
        )
        .unwrap();
    }
    let store = Store::open(db.path()).unwrap();
    let profiles = store.list_profiles().unwrap();
    assert_eq!(profiles.len(), 1);
    assert_eq!(store.active_profile().unwrap(), profiles[0].id);
    assert_eq!(
        store.get_pref(profiles[0].id, "quality").unwrap(),
        Some(json!("Ultra"))
    );
}
