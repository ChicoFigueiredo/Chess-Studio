#![allow(clippy::unwrap_used)]
//! Conexões autorizadas (CX-11, #231, migração 16): o `id` nunca é reaproveitado
//! (`AUTOINCREMENT`), a autorização é reservada, concluída e renovada, e nada aqui guarda
//! segredo (a ligação com o Cofre é o próprio `id`).

mod common;

use chess_store::{
    ConnectionId, ConnectionKind, ConnectionRole, NewConnection, ProfileId, Store, StoreError,
};
use common::TempDb;

fn public_name(provider: &str, name: &str) -> NewConnection {
    NewConnection::new(provider, ConnectionKind::PublicName, name)
}

fn scopes(list: &[&str]) -> Vec<String> {
    list.iter().map(|s| (*s).to_owned()).collect()
}

fn begin(store: &mut Store, profile: ProfileId) -> ConnectionId {
    store
        .begin_authorization(
            profile,
            "lichess",
            ConnectionRole::Player,
            ConnectionKind::AuthorizedBrowser,
        )
        .unwrap()
}

#[test]
fn a_removed_connection_id_is_never_reused() {
    // AUTOINCREMENT: o Cofre guarda o segredo sob o id; um id reaproveitado herdaria o segredo.
    let mut store = Store::open_in_memory().unwrap();
    let profile = store.default_profile().unwrap();
    let first = store
        .add_connection(profile, &public_name("lichess", "a"))
        .unwrap();
    store.remove_connection(profile, first).unwrap();
    let second = store
        .add_connection(profile, &public_name("lichess", "b"))
        .unwrap();
    assert!(second > first, "{second:?} deveria ser maior que {first:?}");
    // Também depois de apagar a mais recente: o contador não volta.
    store.remove_connection(profile, second).unwrap();
    let third = store
        .add_connection(profile, &public_name("lichess", "c"))
        .unwrap();
    assert!(third > second);
}

#[test]
fn ids_are_not_reused_after_deleting_the_whole_local_profile_either() {
    let mut store = Store::open_in_memory().unwrap();
    let ana = store.create_profile("Ana").unwrap();
    let doomed = store
        .add_connection(ana, &public_name("lichess", "ana_l"))
        .unwrap();
    store.delete_profile(ana).unwrap();
    let beto = store.create_profile("Beto").unwrap();
    let fresh = store
        .add_connection(beto, &public_name("lichess", "beto_l"))
        .unwrap();
    assert!(fresh > doomed);
}

#[test]
fn an_authorization_is_reserved_pending_then_recorded() {
    let mut store = Store::open_in_memory().unwrap();
    let profile = store.default_profile().unwrap();
    let id = begin(&mut store, profile);
    let pending = store.pending_authorizations(profile).unwrap();
    assert_eq!(pending.len(), 1);
    assert_eq!(pending[0].id, id);
    assert!(pending[0].is_pending() && !pending[0].is_authorized());
    // Só uma reserva por Provedor e papel.
    assert!(matches!(
        store.begin_authorization(
            profile,
            "lichess",
            ConnectionRole::Player,
            ConnectionKind::AuthorizedKey,
        ),
        Err(StoreError::ConnectionExists { .. })
    ));
    // Mas o papel de Engine é outra reserva.
    store
        .begin_authorization(
            profile,
            "lichess",
            ConnectionRole::Engine,
            ConnectionKind::AuthorizedKey,
        )
        .unwrap();

    store
        .record_authorization(
            profile,
            id,
            ConnectionKind::AuthorizedBrowser,
            "Georges",
            &scopes(&["board:play"]),
            1_234,
        )
        .unwrap();
    let list = store
        .list_connections_by_role(profile, Some("lichess"), ConnectionRole::Player)
        .unwrap();
    assert_eq!(list.len(), 1);
    let conn = &list[0];
    assert!(conn.is_authorized() && !conn.is_pending());
    assert_eq!(conn.identifier, "Georges");
    assert_eq!(conn.scopes, ["board:play"]);
    assert_eq!(conn.authorized_at_ms, Some(1_234));
    assert!(conn.is_default, "a autorizada vira a padrão do papel");
    assert_eq!(store.pending_authorizations(profile).unwrap().len(), 1);
}

#[test]
fn authorizing_a_name_already_linked_absorbs_the_plain_one_keeping_its_nickname() {
    let mut store = Store::open_in_memory().unwrap();
    let profile = store.default_profile().unwrap();
    let plain = store
        .add_connection(
            profile,
            &public_name("lichess", "georges").nickname("Principal"),
        )
        .unwrap();
    let other = store
        .add_connection(profile, &public_name("lichess", "outra"))
        .unwrap();
    let id = begin(&mut store, profile);
    store
        .record_authorization(
            profile,
            id,
            ConnectionKind::AuthorizedBrowser,
            "Georges",
            &scopes(&["board:play"]),
            10,
        )
        .unwrap();
    let list = store.list_connections(profile, Some("lichess")).unwrap();
    assert_eq!(
        list.len(),
        2,
        "o nome vinculado foi absorvido, não duplicado"
    );
    assert!(list.iter().all(|c| c.id != plain));
    let authorized = list.iter().find(|c| c.id == id).unwrap();
    assert_eq!(authorized.nickname, "Principal");
    assert!(authorized.is_default);
    assert!(!list.iter().find(|c| c.id == other).unwrap().is_default);
}

#[test]
fn a_second_authorized_connection_for_the_same_name_is_refused() {
    let mut store = Store::open_in_memory().unwrap();
    let profile = store.default_profile().unwrap();
    let first = begin(&mut store, profile);
    store
        .record_authorization(
            profile,
            first,
            ConnectionKind::AuthorizedBrowser,
            "georges",
            &scopes(&["board:play"]),
            1,
        )
        .unwrap();
    let second = begin(&mut store, profile);
    let result = store.record_authorization(
        profile,
        second,
        ConnectionKind::AuthorizedBrowser,
        "georges",
        &scopes(&["board:play"]),
        2,
    );
    assert!(matches!(result, Err(StoreError::ConnectionExists { .. })));
    // O chamador descarta a reserva que não deu certo.
    store.remove_connection(profile, second).unwrap();
    assert_eq!(store.list_connections(profile, None).unwrap().len(), 1);
}

#[test]
fn renewing_an_authorization_keeps_the_id_and_updates_scopes_and_time() {
    let mut store = Store::open_in_memory().unwrap();
    let profile = store.default_profile().unwrap();
    let id = begin(&mut store, profile);
    store
        .record_authorization(
            profile,
            id,
            ConnectionKind::AuthorizedBrowser,
            "georges",
            &scopes(&["board:play"]),
            1,
        )
        .unwrap();
    store
        .record_authorization(
            profile,
            id,
            ConnectionKind::AuthorizedKey,
            "georges",
            &scopes(&["board:play", "preference:read"]),
            99,
        )
        .unwrap();
    let list = store.list_connections(profile, None).unwrap();
    assert_eq!(list.len(), 1);
    assert_eq!(list[0].id, id);
    assert_eq!(list[0].kind, ConnectionKind::AuthorizedKey);
    assert_eq!(list[0].scopes, ["board:play", "preference:read"]);
    assert_eq!(list[0].authorized_at_ms, Some(99));
}

#[test]
fn authorization_data_is_validated_and_belongs_to_the_local_profile() {
    let mut store = Store::open_in_memory().unwrap();
    let ana = store.create_profile("Ana").unwrap();
    let beto = store.create_profile("Beto").unwrap();
    let id = begin(&mut store, ana);
    assert!(matches!(
        store.record_authorization(beto, id, ConnectionKind::AuthorizedBrowser, "x", &[], 1),
        Err(StoreError::WrongProfile { .. })
    ));
    assert!(matches!(
        store.record_authorization(ana, id, ConnectionKind::PublicName, "x", &[], 1),
        Err(StoreError::InvalidConnection(_))
    ));
    assert!(matches!(
        store.record_authorization(
            ana,
            id,
            ConnectionKind::AuthorizedBrowser,
            "x",
            &scopes(&["a b"]),
            1
        ),
        Err(StoreError::InvalidConnection(_))
    ));
    assert!(matches!(
        store.begin_authorization(
            ana,
            "lichess",
            ConnectionRole::Player,
            ConnectionKind::PublicName
        ),
        Err(StoreError::InvalidConnection(_))
    ));
    assert!(matches!(
        store.record_authorization(
            ana,
            ConnectionId(9_999),
            ConnectionKind::AuthorizedKey,
            "x",
            &[],
            1
        ),
        Err(StoreError::ConnectionNotFound(_))
    ));
}

#[test]
fn migration_16_keeps_rows_and_ids_and_the_counter_never_goes_back() {
    let db = TempDb::new("conn-migrate-16");
    {
        let mut conn = rusqlite::Connection::open(db.path()).unwrap();
        chess_store::migrations::migrate_to(&mut conn, 15).unwrap();
        conn.execute_batch(
            "INSERT INTO connections (id, profile_id, provider, role, kind, identifier, nickname,
                                      is_default, created_at_ms)
                 VALUES (5, 1, 'lichess', 'player', 'public_name', 'ana_l', 'Principal', 1, 7),
                        (9, 1, 'fide', 'player', 'public_id', '1503014', '', 1, 8);",
        )
        .unwrap();
    }
    let mut store = Store::open(db.path()).unwrap();
    let profile = ProfileId(1);
    let list = store.list_connections(profile, None).unwrap();
    assert_eq!(list.len(), 2);
    let ana = list.iter().find(|c| c.id == ConnectionId(5)).unwrap();
    assert_eq!(
        (ana.nickname.as_str(), ana.is_default, ana.authorized_at_ms),
        ("Principal", true, None)
    );
    assert!(ana.scopes.is_empty());
    // O contador continua do maior id, e depois de apagar o último ele não volta.
    let next = store
        .add_connection(profile, &public_name("lichess", "nova"))
        .unwrap();
    assert!(next > ConnectionId(9));
    store.remove_connection(profile, next).unwrap();
    let after = store
        .add_connection(profile, &public_name("lichess", "outra"))
        .unwrap();
    assert!(after > next);
}
