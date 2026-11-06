#![allow(clippy::unwrap_used)]
//! Interface acordada do repositório: gravar, fechar, reabrir, retomar.

mod common;

use chess_game::{Event, EventKind, GameSetup, PAYLOAD_VERSION, PolicyKind, TimeControl};
use chess_store::{GameMetadata, GameQuery, Store, StoreError};
use common::{TempDb, play};
use serde_json::json;

fn setup() -> GameSetup {
    GameSetup::standard(PolicyKind::Assisted).with_time_control(TimeControl {
        base_ms: 300_000,
        increment_ms: 2_000,
    })
}

/// DB-01 + CORE-09 + DB-04: reabrir reproduz o mesmo `snapshot()`, com Lance
/// ilegal registrado fora do histórico legal.
#[test]
fn save_close_reopen_restores_identical_snapshot() {
    let db = TempDb::new("roundtrip");
    let (id, original) = {
        let mut store = Store::open(db.path()).unwrap();
        let (game, events) = play(
            setup(),
            &[("e2", "e4"), ("e7", "e5"), ("e2", "e5"), ("g1", "f3")],
        );
        let id = store.create_game(game.setup(), &GameMetadata::default()).unwrap();
        store.append_events(id, &events).unwrap();
        (id, game.snapshot())
    };
    let store = Store::open(db.path()).unwrap();
    let loaded = store.load_game(id).unwrap();
    assert_eq!(loaded.snapshot(), original);
    assert_eq!(loaded.setup(), &setup());
    // O Lance ilegal (e2e5, branco já movido -> peça inexistente) está no log, não no histórico.
    assert!(
        loaded
            .events()
            .iter()
            .any(|e| matches!(e.kind, EventKind::IllegalMoveRejected { .. }))
    );
    assert_eq!(loaded.snapshot().history.len(), 3);
}

#[test]
fn appends_in_several_calls_keep_sequence_order() {
    let mut store = Store::open_in_memory().unwrap();
    let (game, events) = play(setup(), &[("e2", "e4"), ("e7", "e5"), ("g1", "f3")]);
    let id = store.create_game(game.setup(), &GameMetadata::default()).unwrap();
    for chunk in events.chunks(1) {
        store.append_events(id, chunk).unwrap();
    }
    assert_eq!(store.load_game(id).unwrap().events(), events.as_slice());
}

#[test]
fn append_to_unknown_game_is_a_typed_error() {
    let mut store = Store::open_in_memory().unwrap();
    let (_, events) = play(setup(), &[("e2", "e4")]);
    let bogus = chess_store::GameId(999);
    assert!(matches!(
        store.append_events(bogus, &events),
        Err(StoreError::GameNotFound(_))
    ));
}

/// Log que não reaplica vira erro tipado, não uma Partida pela metade.
#[test]
fn load_game_reports_log_that_does_not_replay() {
    let mut store = Store::open_in_memory().unwrap();
    let id = store.create_game(&setup(), &GameMetadata::default()).unwrap();
    // Lance das pretas como primeiro Evento: lado errado.
    let (_, events) = play(setup(), &[("e2", "e4"), ("e7", "e5")]);
    store.append_events(id, &events[1..]).unwrap();
    assert!(matches!(
        store.load_game(id),
        Err(StoreError::Replay { .. })
    ));
}

#[test]
fn unknown_event_version_is_rejected_on_read_with_clear_error() {
    let db = TempDb::new("version");
    let mut store = Store::open(db.path()).unwrap();
    let (game, events) = play(setup(), &[("e2", "e4")]);
    let id = store.create_game(game.setup(), &GameMetadata::default()).unwrap();
    store.append_events(id, &events).unwrap();
    drop(store);

    let conn = rusqlite::Connection::open(db.path()).unwrap();
    conn.execute("UPDATE game_events SET version = ?1", [PAYLOAD_VERSION + 1])
        .unwrap();
    drop(conn);

    let store = Store::open(db.path()).unwrap();
    let err = store.load_game(id).unwrap_err();
    assert!(matches!(
        err,
        StoreError::UnsupportedEventVersion { version, .. } if version == PAYLOAD_VERSION + 1
    ));
    assert!(err.to_string().contains("versão"));
}

#[test]
fn refuses_to_write_an_event_with_unknown_version() {
    let mut store = Store::open_in_memory().unwrap();
    let id = store.create_game(&setup(), &GameMetadata::default()).unwrap();
    let (_, mut events) = play(setup(), &[("e2", "e4")]);
    events[0] = Event {
        version: PAYLOAD_VERSION + 1,
        ..events[0].clone()
    };
    assert!(matches!(
        store.append_events(id, &events),
        Err(StoreError::UnsupportedEventVersion { .. })
    ));
    assert!(store.load_game(id).unwrap().events().is_empty());
}

/// Um lote inválido (versão ruim no meio) não grava nada: uma transação por chamada.
#[test]
fn failed_batch_leaves_nothing_behind() {
    let mut store = Store::open_in_memory().unwrap();
    let id = store.create_game(&setup(), &GameMetadata::default()).unwrap();
    let (_, mut events) = play(setup(), &[("e2", "e4"), ("e7", "e5")]);
    events[1].version = PAYLOAD_VERSION + 7;
    assert!(store.append_events(id, &events).is_err());
    assert!(store.load_game(id).unwrap().events().is_empty());
}

/// DB-02 no nível do store: FEN inválido nem chega a criar `GameSetup`, e o banco não muda.
#[test]
fn invalid_fen_does_not_touch_the_database() {
    let store = Store::open_in_memory().unwrap();
    assert!(GameSetup::from_fen(PolicyKind::Assisted, "isto não é FEN").is_err());
    assert!(store.list_games(&GameQuery::default()).unwrap().is_empty());
}

#[test]
fn custom_fen_start_survives_reopen() {
    let db = TempDb::new("fen");
    let fen = "r3k2r/8/8/8/8/8/8/R3K2R w KQkq - 0 1";
    let setup = GameSetup::from_fen(PolicyKind::Assisted, fen).unwrap();
    let (game, events) = play(setup, &[("e1", "g1"), ("e8", "c8")]);
    let id = {
        let mut store = Store::open(db.path()).unwrap();
        let id = store.create_game(game.setup(), &GameMetadata::default()).unwrap();
        store.append_events(id, &events).unwrap();
        id
    };
    let loaded = Store::open(db.path()).unwrap().load_game(id).unwrap();
    assert_eq!(loaded.snapshot(), game.snapshot());
}

#[test]
fn game_end_is_recorded_and_unfinished_lookup_skips_it() {
    let mut store = Store::open_in_memory().unwrap();
    let (mated, mated_events) = play(
        setup(),
        &[("f2", "f3"), ("e7", "e5"), ("g2", "g4"), ("d8", "h4")],
    );
    let done = store.create_game(mated.setup(), &GameMetadata::default()).unwrap();
    store.append_events(done, &mated_events).unwrap();
    let open = store.create_game(&setup(), &GameMetadata::default()).unwrap();

    assert_eq!(store.latest_unfinished_game().unwrap(), Some(open));
    let games = store.list_games(&GameQuery::default()).unwrap();
    assert_eq!(games.len(), 2);
    let finished = games.iter().find(|g| g.id == done).unwrap();
    assert!(finished.finished);
    assert!(finished.outcome.is_some(), "AC-07: resultado e motivo guardados");
    assert_eq!(finished.event_count, mated_events.len() as u64);
    let only_open = store
        .list_games(&GameQuery {
            only_unfinished: true,
            ..GameQuery::default()
        })
        .unwrap();
    assert_eq!(only_open.len(), 1);
}

#[test]
fn latest_unfinished_is_none_on_empty_database() {
    let store = Store::open_in_memory().unwrap();
    assert_eq!(store.latest_unfinished_game().unwrap(), None);
}

/// Fechamento limpo vs. abrupto; a reabertura devolve a Partida pausada (DEC-42).
#[test]
fn reopen_reports_clean_or_abrupt_close_and_returns_paused() {
    let db = TempDb::new("session");
    let (game, events) = play(setup(), &[("e2", "e4")]);
    let id = {
        let mut store = Store::open(db.path()).unwrap();
        let id = store.create_game(game.setup(), &GameMetadata::default()).unwrap();
        store.append_events(id, &events).unwrap();
        id // sem close_game: como um processo que morreu
    };
    let store = Store::open(db.path()).unwrap();
    let opened = store.open_game(id).unwrap();
    assert!(opened.abrupt_close);
    assert!(opened.paused);
    assert_eq!(opened.game.snapshot(), game.snapshot());

    store.resume_game(id).unwrap();
    store.close_game(id).unwrap();
    drop(store);

    let store = Store::open(db.path()).unwrap();
    let opened = store.open_game(id).unwrap();
    assert!(!opened.abrupt_close);
    assert!(opened.paused, "toda reabertura volta pausada");
}

/// UX-08 no nível do store.
#[test]
fn preferences_persist_across_opens() {
    let db = TempDb::new("prefs");
    {
        let store = Store::open(db.path()).unwrap();
        let p = store.default_profile().unwrap();
        store.set_pref(p, "board.view", &json!({"camera": "white", "fov": 45.5})).unwrap();
        store.set_pref(p, "input.method", &json!("click")).unwrap();
        store.set_pref(p, "input.method", &json!("drag")).unwrap();
    }
    let store = Store::open(db.path()).unwrap();
    let p = store.default_profile().unwrap();
    assert_eq!(
        store.get_pref(p, "board.view").unwrap(),
        Some(json!({"camera": "white", "fov": 45.5}))
    );
    assert_eq!(store.get_pref(p, "input.method").unwrap(), Some(json!("drag")));
    assert_eq!(store.get_pref(p, "nada").unwrap(), None);
}

#[test]
fn preferences_are_per_profile() {
    let store = Store::open_in_memory().unwrap();
    let a = store.default_profile().unwrap();
    let b = store.create_profile("convidado").unwrap();
    store.set_pref(a, "k", &json!(1)).unwrap();
    assert_eq!(store.get_pref(b, "k").unwrap(), None);
    assert!(matches!(
        store.get_pref(chess_store::ProfileId(404), "k"),
        Err(StoreError::ProfileNotFound(_)) | Ok(None)
    ));
}

#[test]
fn pragmas_are_applied() {
    let db = TempDb::new("pragmas");
    let store = Store::open(db.path()).unwrap();
    let pragmas = store.pragmas().unwrap();
    assert_eq!(pragmas.journal_mode, "wal");
    assert!(pragmas.foreign_keys);
    assert_eq!(pragmas.synchronous, 2, "FULL");
    assert!(pragmas.busy_timeout_ms >= 1000);
}
