#![allow(clippy::unwrap_used)]
//! CX-19 (#242): último rating FIDE por Perfil local (migração 17) e Partidas importadas
//! apagadas em lote por Origem.

mod common;

use chess_game::{GameSetup, PolicyKind};
use chess_store::{
    FideSnapshot, GameMetadata, ImportMode, ImportOptions, ImportScope, ImportedQuery, ProfileId,
    Provenance, Store, migrations,
};
use common::{TempDb, play};

const ME: ProfileId = ProfileId(1);

fn snapshot(id: u64) -> FideSnapshot {
    FideSnapshot {
        fide_id: id,
        name: "Exemplo, Ana".into(),
        federation: Some("BRA".into()),
        title: Some("FM".into()),
        birth_year: Some(1999),
        standard: Some(2301),
        rapid: Some(2250),
        blitz: None,
        inactive: false,
        queried_at_ms: 1_794_398_400_000,
        source: "lichess".into(),
    }
}

// ---- Migração 17 e rating guardado -------------------------------------------------

#[test]
fn migration_17_applies_on_a_v16_database_and_keeps_every_row() {
    let db = TempDb::new("fide-mig17");
    {
        let mut conn = rusqlite::Connection::open(db.path()).unwrap();
        migrations::migrate_to(&mut conn, 16).unwrap();
        conn.execute_batch(
            "INSERT INTO profiles (id, name, created_at_ms) VALUES (2, 'Ana', 5);
             INSERT INTO connections (id, profile_id, provider, role, kind, identifier, nickname,
                                      is_default, created_at_ms)
                 VALUES (3, 2, 'fide', 'player', 'public_id', '9999991', '', 1, 8);",
        )
        .unwrap();
        assert_eq!(migrations::current_version(&conn).unwrap(), 16);
    }
    let store = Store::open(db.path()).unwrap();
    assert_eq!(store.schema_version().unwrap(), 17);
    let ana = ProfileId(2);
    assert_eq!(store.profile(ana).unwrap().name, "Ana");
    assert_eq!(store.list_connections(ana, None).unwrap().len(), 1);
    assert_eq!(store.fide_rating(ana, 9_999_991).unwrap(), None);
}

#[test]
fn the_saved_rating_is_read_back_offline_with_standard_rapid_and_blitz_apart() {
    let db = TempDb::new("fide-offline");
    {
        let store = Store::open(db.path()).unwrap();
        store.save_fide_rating(ME, &snapshot(9_999_991)).unwrap();
    }
    // Outro processo, sem rede nenhuma: o valor continua lá.
    let store = Store::open(db.path()).unwrap();
    let saved = store.fide_rating(ME, 9_999_991).unwrap().unwrap();
    assert_eq!(saved, snapshot(9_999_991));
    assert_eq!(
        (saved.standard, saved.rapid, saved.blitz),
        (Some(2301), Some(2250), None),
        "o blitz ausente continua ausente: nunca herda outro ritmo"
    );
}

#[test]
fn a_new_query_replaces_the_value_and_each_local_profile_keeps_its_own() {
    let store = Store::open_in_memory().unwrap();
    let other = store.create_profile("Beto").unwrap();
    store.save_fide_rating(ME, &snapshot(1)).unwrap();
    let mut newer = snapshot(1);
    newer.standard = Some(2320);
    newer.blitz = Some(2199);
    newer.queried_at_ms += 86_400_000;
    store.save_fide_rating(ME, &newer).unwrap();
    assert_eq!(store.fide_rating(ME, 1).unwrap().unwrap(), newer);
    assert_eq!(store.fide_rating(other, 1).unwrap(), None);
    store.save_fide_rating(other, &snapshot(1)).unwrap();
    assert_eq!(
        store.fide_rating(other, 1).unwrap().unwrap().standard,
        Some(2301)
    );
    assert_eq!(
        store.fide_rating(ME, 1).unwrap().unwrap().standard,
        Some(2320)
    );
}

#[test]
fn deleting_a_local_profile_takes_its_saved_rating_along() {
    let mut store = Store::open_in_memory().unwrap();
    let other = store.create_profile("Beto").unwrap();
    store.save_fide_rating(other, &snapshot(5)).unwrap();
    store.delete_profile(other).unwrap();
    let conn_count: i64 = {
        // O perfil sumiu: a leitura por ele não acha nada e a tabela não guarda órfãs.
        store.fide_rating(other, 5).unwrap().map_or(0, |_| 1)
    };
    assert_eq!(conn_count, 0);
}

// ---- Apagar importadas por Origem --------------------------------------------------

fn pgn(tag: &str, n: u32) -> String {
    (1..=n).fold(String::new(), |mut out, i| {
        out.push_str(&format!(
            "[Event \"{tag} {i}\"]\n[Site \"https://exemplo.test/{tag}{i}\"]\n[White \"{tag}\"]\n\
             [Black \"adversario{i}\"]\n[Date \"2026.11.0{i}\"]\n[Result \"1-0\"]\n\n\
             1. e4 e5 2. Nf3 Nc6 {i} 1-0\n\n"
        ));
        out
    })
}

fn import(store: &mut Store, tag: &str, n: u32, file: Option<&str>, provider: &str, at: i64) {
    let source = chess_store::ImportSource::from_provider_id(provider);
    let provenance = match source {
        Some(s) => Provenance::from_source(s, ImportMode::Api, at),
        None => Provenance::unknown(ImportMode::File, at),
    };
    store
        .import_pgn_with(ME, &pgn(tag, n), &ImportOptions::new(file, provenance))
        .unwrap();
}

fn imported_count(store: &Store) -> usize {
    store
        .list_imported(&ImportedQuery {
            profile: Some(ME),
            ..Default::default()
        })
        .unwrap()
        .len()
}

fn play_one(store: &mut Store) {
    let (game, events) = play(
        GameSetup::standard(PolicyKind::Assisted),
        &[("e2", "e4"), ("e7", "e5")],
    );
    let id = store
        .create_game(game.setup(), &GameMetadata::default())
        .unwrap();
    store.append_events(id, &events).unwrap();
}

#[test]
fn groups_list_each_import_by_name_and_date_newest_first() {
    let mut store = Store::open_in_memory().unwrap();
    import(&mut store, "ana", 3, Some("lichess ana"), "lichess", 1_000);
    import(&mut store, "bia", 2, Some("lichess ana"), "lichess", 2_000);
    import(&mut store, "cid", 4, Some("estudos.pgn"), "other", 3_000);
    let groups = store.import_groups(ME).unwrap();
    let shape: Vec<_> = groups
        .iter()
        .map(|g| {
            (
                g.provider.as_deref(),
                g.source.as_deref(),
                g.retrieved_at_ms,
                g.games,
            )
        })
        .collect();
    assert_eq!(
        shape,
        [
            (Some("other"), Some("estudos.pgn"), 3_000, 4),
            (Some("lichess"), Some("lichess ana"), 2_000, 2),
            (Some("lichess"), Some("lichess ana"), 1_000, 3),
        ]
    );
}

#[test]
fn deleting_one_import_leaves_the_other_imports_and_every_played_game() {
    let mut store = Store::open_in_memory().unwrap();
    import(&mut store, "ana", 3, Some("lichess ana"), "lichess", 1_000);
    import(&mut store, "bia", 2, Some("lichess ana"), "lichess", 2_000);
    play_one(&mut store);
    let scope = store.import_groups(ME).unwrap()[1].scope();
    let preview = store.count_imported_in(ME, &scope).unwrap();
    assert_eq!((preview.to_delete, preview.kept_other_origin), (3, 0));

    let done = store.delete_imported_in(ME, &scope).unwrap();

    assert_eq!(done.deleted, 3);
    assert_eq!(imported_count(&store), 2);
    let own = store
        .list_games(&chess_store::GameQuery::default())
        .unwrap();
    assert_eq!(own.len(), 1, "a Partida jogada no app nunca é tocada");
}

#[test]
fn deleting_a_provider_takes_all_its_imports_and_nothing_of_the_others() {
    let mut store = Store::open_in_memory().unwrap();
    import(&mut store, "ana", 3, Some("a"), "lichess", 1_000);
    import(&mut store, "bia", 2, Some("b"), "lichess", 2_000);
    import(&mut store, "cid", 4, Some("c"), "chess-com", 3_000);
    import(
        &mut store,
        "dai",
        1,
        Some("d.pgn"),
        "xx-desconhecido",
        4_000,
    );
    play_one(&mut store);

    let scope = ImportScope::Provider(Some("lichess".into()));
    assert_eq!(store.count_imported_in(ME, &scope).unwrap().to_delete, 5);
    store.delete_imported_in(ME, &scope).unwrap();

    assert_eq!(imported_count(&store), 5);
    let left: Vec<_> = store.import_groups(ME).unwrap();
    assert!(
        left.iter()
            .all(|g| g.provider.as_deref() != Some("lichess"))
    );
    assert_eq!(
        store
            .list_games(&chess_store::GameQuery::default())
            .unwrap()
            .len(),
        1
    );
    // Arquivos sem Provedor têm o seu próprio alvo.
    let none = ImportScope::Provider(None);
    assert_eq!(store.count_imported_in(ME, &none).unwrap().to_delete, 1);
    store.delete_imported_in(ME, &none).unwrap();
    assert_eq!(imported_count(&store), 4);
}

#[test]
fn another_local_profile_is_never_touched() {
    let mut store = Store::open_in_memory().unwrap();
    let other = store.create_profile("Beto").unwrap();
    import(&mut store, "ana", 3, Some("a"), "lichess", 1_000);
    store
        .import_pgn_with(
            other,
            &pgn("beto", 2),
            &ImportOptions::new(
                Some("a"),
                Provenance::from_source(chess_store::ImportSource::Lichess, ImportMode::Api, 1_000),
            ),
        )
        .unwrap();
    store
        .delete_imported_in(ME, &ImportScope::Provider(Some("lichess".into())))
        .unwrap();
    assert_eq!(imported_count(&store), 0);
    assert_eq!(store.import_groups(other).unwrap()[0].games, 2);
}

#[test]
fn a_game_that_also_came_from_another_origin_stays_and_is_counted_apart() {
    let mut store = Store::open_in_memory().unwrap();
    // A mesma Partida chega primeiro de um arquivo e depois do Lichess: uma Partida, duas Origens.
    import(&mut store, "ana", 2, Some("estudos.pgn"), "other", 1_000);
    import(&mut store, "ana", 2, Some("lichess ana"), "lichess", 2_000);
    import(&mut store, "bia", 1, Some("estudos.pgn"), "other", 1_000);
    assert_eq!(
        imported_count(&store),
        3,
        "as repetidas viram Origem, não cópia"
    );

    let scope = ImportScope::Provider(Some("other".into()));
    let preview = store.count_imported_in(ME, &scope).unwrap();
    assert_eq!((preview.to_delete, preview.kept_other_origin), (1, 2));
    let done = store.delete_imported_in(ME, &scope).unwrap();
    assert_eq!((done.deleted, done.kept_other_origin), (1, 2));
    assert_eq!(imported_count(&store), 2);
}
