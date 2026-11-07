//! API de Engines instaladas (UCI-05): cadastro, configuração persistida e migração que
//! preserva os registros do esquema antigo.

use chess_store::{EngineId, EngineOrigin, EngineRecord, Store, StoreError};
use std::collections::BTreeMap;

fn record(name: &str, path: &str) -> EngineRecord {
    EngineRecord {
        name: name.to_owned(),
        version: Some("19".to_owned()),
        path: path.to_owned(),
        protocol: "uci".to_owned(),
        checksum: Some("ab".repeat(32)),
        status: "ready".to_owned(),
        origin: EngineOrigin::Catalog,
        catalog_id: Some("stockfish".to_owned()),
        variant: Some(String::new()),
        install_dir: Some("/dados/engines/stockfish-19".to_owned()),
        settings: BTreeMap::new(),
        installed_at_ms: 1_000,
    }
}

#[test]
fn engines_are_registered_listed_updated_and_removed() {
    let store = Store::open_in_memory().unwrap();
    assert!(store.list_engines().unwrap().is_empty());

    let a = store
        .upsert_engine(&record("Stockfish 19", "/dados/engines/stockfish-19/sf"))
        .unwrap();
    let mut manual = record("Minha Engine", "/opt/eng/eng");
    manual.origin = EngineOrigin::Manual;
    manual.install_dir = None;
    manual.catalog_id = None;
    let b = store.upsert_engine(&manual).unwrap();
    assert_ne!(a, b);

    assert_eq!(store.list_engines().unwrap().len(), 2);
    let got = store.get_engine(a).unwrap().expect("existe");
    assert_eq!(got.name, "Stockfish 19");
    assert_eq!(got.origin, EngineOrigin::Catalog);
    let manual_back = store.get_engine(b).unwrap().unwrap();
    assert_eq!(manual_back.origin, EngineOrigin::Manual);
    assert_eq!(manual_back.install_dir, None);

    // O mesmo caminho atualiza o registro em vez de duplicar.
    let mut newer = record("Stockfish 19.1", "/dados/engines/stockfish-19/sf");
    newer.version = Some("19.1".into());
    let again = store.upsert_engine(&newer).unwrap();
    assert_eq!(again, a);
    assert_eq!(store.list_engines().unwrap().len(), 2);
    assert_eq!(
        store.get_engine(a).unwrap().unwrap().version.as_deref(),
        Some("19.1")
    );

    store.remove_engine(b).unwrap();
    assert_eq!(store.list_engines().unwrap().len(), 1);
    assert!(store.get_engine(b).unwrap().is_none());
}

#[test]
fn settings_and_status_persist() {
    let store = Store::open_in_memory().unwrap();
    let id = store.upsert_engine(&record("SF", "/x/sf")).unwrap();
    let mut settings = BTreeMap::new();
    settings.insert("Threads".to_owned(), "4".to_owned());
    settings.insert("Hash".to_owned(), "256".to_owned());
    store.set_engine_settings(id, &settings).unwrap();
    store.set_engine_status(id, "missing").unwrap();
    let got = store.get_engine(id).unwrap().unwrap();
    assert_eq!(got.settings, settings);
    assert_eq!(got.status, "missing");
    assert!(matches!(
        store.set_engine_settings(EngineId(9999), &settings),
        Err(StoreError::EngineNotFound(_))
    ));
}

#[test]
fn legacy_rows_survive_the_migration_with_safe_defaults() {
    let path = std::env::temp_dir().join(format!("engines-legacy-{}.db", std::process::id()));
    let _ = std::fs::remove_file(&path);
    {
        // Banco no esquema da versão 3, com uma Engine gravada pelo esquema antigo.
        let mut conn = rusqlite::Connection::open(&path).unwrap();
        chess_store::migrations::migrate_to(&mut conn, 3).unwrap();
        conn.execute(
            "INSERT INTO engine_installations (name, version, path, protocol, checksum, status)
             VALUES ('Velha', '1', '/velha/eng', 'uci', NULL, 'unknown')",
            [],
        )
        .unwrap();
    }
    let store = Store::open(&path).unwrap();
    let all = store.list_engines().unwrap();
    assert_eq!(all.len(), 1);
    let (_, old) = &all[0];
    assert_eq!(old.name, "Velha");
    assert_eq!(old.path, "/velha/eng");
    assert_eq!(
        old.origin,
        EngineOrigin::Manual,
        "registro antigo nunca é apagável"
    );
    assert_eq!(old.install_dir, None);
    assert!(old.settings.is_empty());
    drop(store);
    let _ = std::fs::remove_file(&path);
}
