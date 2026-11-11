#![allow(clippy::unwrap_used)]
//! Conexões por nome ou ID público (CX-3, #214): vários por Provedor e por Perfil local, com
//! apelido e uma padrão. Nenhum segredo passa por aqui (o Cofre é a etapa 3).

mod common;

use chess_store::{ConnectionKind, ConnectionRole, NewConnection, Store, StoreError};
use common::TempDb;

fn public_name(provider: &str, name: &str) -> NewConnection {
    NewConnection::new(provider, ConnectionKind::PublicName, name)
}

#[test]
fn a_public_name_connection_persists_across_reopening() {
    let db = TempDb::new("conn-persist");
    let id = {
        let mut store = Store::open(db.path()).unwrap();
        let profile = store.default_profile().unwrap();
        store
            .add_connection(profile, &public_name("lichess", "magnus"))
            .unwrap()
    };
    let store = Store::open(db.path()).unwrap();
    let profile = store.default_profile().unwrap();
    let list = store.list_connections(profile, Some("lichess")).unwrap();
    assert_eq!(list.len(), 1);
    assert_eq!(list[0].id, id);
    assert_eq!(list[0].identifier, "magnus");
    assert_eq!(list[0].kind, ConnectionKind::PublicName);
    assert_eq!(list[0].role, ConnectionRole::Player);
    assert!(
        list[0].is_default,
        "a primeira Conexão do Provedor é a padrão"
    );
}

#[test]
fn there_can_be_several_per_provider_with_one_default() {
    let mut store = Store::open_in_memory().unwrap();
    let profile = store.default_profile().unwrap();
    let first = store
        .add_connection(profile, &public_name("chesscom", "hikaru"))
        .unwrap();
    let second = store
        .add_connection(
            profile,
            &public_name("chesscom", "fabiano").nickname("Meu irmão"),
        )
        .unwrap();
    let list = store.list_connections(profile, Some("chesscom")).unwrap();
    assert_eq!(list.len(), 2);
    assert_eq!(list.iter().filter(|c| c.is_default).count(), 1);
    assert!(list.iter().find(|c| c.id == first).unwrap().is_default);

    store.set_default_connection(profile, second).unwrap();
    let list = store.list_connections(profile, Some("chesscom")).unwrap();
    assert!(list.iter().find(|c| c.id == second).unwrap().is_default);
    assert!(!list.iter().find(|c| c.id == first).unwrap().is_default);
    assert_eq!(
        list.iter().find(|c| c.id == second).unwrap().nickname,
        "Meu irmão"
    );
}

#[test]
fn removing_the_default_promotes_another_one() {
    let mut store = Store::open_in_memory().unwrap();
    let profile = store.default_profile().unwrap();
    let a = store
        .add_connection(profile, &public_name("lichess", "a"))
        .unwrap();
    let b = store
        .add_connection(profile, &public_name("lichess", "b"))
        .unwrap();
    store.remove_connection(profile, a).unwrap();
    let list = store.list_connections(profile, Some("lichess")).unwrap();
    assert_eq!(list.len(), 1);
    assert_eq!(list[0].id, b);
    assert!(list[0].is_default);
}

#[test]
fn the_same_identifier_twice_is_refused_but_other_providers_are_independent() {
    let mut store = Store::open_in_memory().unwrap();
    let profile = store.default_profile().unwrap();
    store
        .add_connection(profile, &public_name("lichess", "magnus"))
        .unwrap();
    assert!(matches!(
        store.add_connection(profile, &public_name("lichess", "MAGNUS")),
        Err(StoreError::ConnectionExists { .. })
    ));
    store
        .add_connection(profile, &public_name("chesscom", "magnus"))
        .unwrap();
    assert_eq!(store.list_connections(profile, None).unwrap().len(), 2);
}

#[test]
fn connections_belong_to_the_local_profile() {
    let mut store = Store::open_in_memory().unwrap();
    let ana = store.create_profile("Ana").unwrap();
    let beto = store.create_profile("Beto").unwrap();
    let id = store
        .add_connection(ana, &public_name("lichess", "ana_l"))
        .unwrap();
    assert!(store.list_connections(beto, None).unwrap().is_empty());
    assert!(matches!(
        store.remove_connection(beto, id),
        Err(StoreError::WrongProfile { .. })
    ));
    assert!(matches!(
        store.set_default_connection(beto, id),
        Err(StoreError::WrongProfile { .. })
    ));
    // Excluir o Perfil local leva as Conexões dele.
    store.delete_profile(ana).unwrap();
    assert!(store.list_connections(ana, None).unwrap().is_empty());
}

#[test]
fn identifiers_are_trimmed_and_validated() {
    let mut store = Store::open_in_memory().unwrap();
    let profile = store.default_profile().unwrap();
    let id = store
        .add_connection(profile, &public_name("lichess", "  magnus  "))
        .unwrap();
    assert_eq!(
        store.list_connections(profile, None).unwrap()[0].identifier,
        "magnus"
    );
    for bad in ["", "   ", "com espaço", "a\nb", &"x".repeat(65)] {
        assert!(
            matches!(
                store.add_connection(profile, &public_name("lichess", bad)),
                Err(StoreError::InvalidConnection(_))
            ),
            "{bad:?}"
        );
    }
    assert!(matches!(
        store.add_connection(profile, &public_name("", "x")),
        Err(StoreError::InvalidConnection(_))
    ));
    store
        .rename_connection(profile, id, "  Principal ")
        .unwrap();
    assert_eq!(
        store.list_connections(profile, None).unwrap()[0].nickname,
        "Principal"
    );
}

#[test]
fn engine_connections_do_not_mix_with_player_ones() {
    let mut store = Store::open_in_memory().unwrap();
    let profile = store.default_profile().unwrap();
    store
        .add_connection(profile, &public_name("lichess", "magnus"))
        .unwrap();
    store
        .add_connection(
            profile,
            &public_name("lichess", "magnus").role(ConnectionRole::Engine),
        )
        .unwrap();
    let players = store
        .list_connections_by_role(profile, Some("lichess"), ConnectionRole::Player)
        .unwrap();
    let engines = store
        .list_connections_by_role(profile, Some("lichess"), ConnectionRole::Engine)
        .unwrap();
    assert_eq!((players.len(), engines.len()), (1, 1));
    // Cada papel tem a sua padrão.
    assert!(players[0].is_default && engines[0].is_default);
}

#[test]
fn migration_16_applies_on_a_version_14_database_and_keeps_everything() {
    let db = TempDb::new("conn-migrate");
    {
        let mut conn = rusqlite::Connection::open(db.path()).unwrap();
        chess_store::migrations::migrate_to(&mut conn, 14).unwrap();
        conn.execute_batch(
            "INSERT INTO profiles (id, name, created_at_ms) VALUES (2, 'Ana', 5);
             INSERT INTO ui_preferences (profile_id, key, value_json) VALUES (2, 'quality', '\"Ultra\"');
             INSERT INTO exercises (id, kind, initial_fen, solution_json, version, created_at_ms, origin, pack)
                 VALUES (7, 'tactic', '8/8/8/8/8/8/8/K6k w - - 0 1', '[]', 1, 9, 'lichess', 'pack-a');",
        )
        .unwrap();
        assert_eq!(chess_store::migrations::current_version(&conn).unwrap(), 14);
    }
    let mut store = Store::open(db.path()).unwrap();
    assert!(store.schema_version().unwrap() >= 16);
    let ana = chess_store::ProfileId(2);
    assert_eq!(store.profile(ana).unwrap().name, "Ana");
    assert_eq!(
        store.get_pref(ana, "quality").unwrap(),
        Some(serde_json::json!("Ultra"))
    );
    store
        .add_connection(ana, &public_name("lichess", "ana_l"))
        .unwrap();
    drop(store);
    let conn = rusqlite::Connection::open(db.path()).unwrap();
    let (origin, pack): (String, String) = conn
        .query_row("SELECT origin, pack FROM exercises WHERE id = 7", [], |r| {
            Ok((r.get(0)?, r.get(1)?))
        })
        .unwrap();
    assert_eq!((origin.as_str(), pack.as_str()), ("lichess", "pack-a"));
}
