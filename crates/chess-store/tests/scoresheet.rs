#![allow(clippy::unwrap_used)]
//! Súmula manual (V2-SUM-01): registro separado, por perfil local e por Partida. Nunca
//! altera a Partida, o log de eventos nem o PGN.

mod common;

use chess_game::{GameSetup, PgnOptions, PolicyKind};
use chess_store::{GameId, GameMetadata, ProfileId, Store, StoreError};
use chess_training::ManualScoresheet;
use common::{TempDb, play};

struct World {
    db: TempDb,
    store: Store,
    ana: ProfileId,
    beto: ProfileId,
    ana_game: GameId,
    beto_game: GameId,
}

fn world() -> World {
    let db = TempDb::new("scoresheet");
    let mut store = Store::open(db.path()).unwrap();
    let ana = store.default_profile().unwrap();
    let beto = store.create_profile("Beto").unwrap();
    let mut games = Vec::new();
    for profile in [ana, beto] {
        let (game, events) = play(
            GameSetup::standard(PolicyKind::Assisted),
            &[("e2", "e4"), ("e7", "e5")],
        );
        let id = store
            .create_game_for(profile, game.setup(), &GameMetadata::default())
            .unwrap();
        store.append_events(id, &events).unwrap();
        games.push(id);
    }
    World {
        db,
        store,
        ana,
        beto,
        ana_game: games[0],
        beto_game: games[1],
    }
}

fn sheet(entries: &[(u16, &str)]) -> ManualScoresheet {
    ManualScoresheet::from_parts(entries.iter().map(|(p, t)| (*p, (*t).to_owned())), None).unwrap()
}

/// Lê o banco por uma conexão à parte: o que está de fato gravado, sem passar pela API.
fn raw<T: rusqlite::types::FromSql>(w: &World, sql: &str) -> T {
    let conn = rusqlite::Connection::open(w.db.path()).unwrap();
    conn.query_row(sql, [], |r| r.get(0)).unwrap()
}

fn events_dump(w: &World) -> String {
    raw(
        w,
        "SELECT group_concat(game_id || ':' || seq || ':' || kind || ':' || payload_json, '|')
         FROM (SELECT * FROM game_events ORDER BY game_id, seq)",
    )
}

fn pgn(store: &Store, id: GameId) -> String {
    store
        .open_game(id)
        .unwrap()
        .game
        .to_pgn(&PgnOptions::default())
}

#[test]
fn a_game_without_scoresheet_has_none() {
    let w = world();
    assert_eq!(w.store.load_scoresheet(w.ana, w.ana_game).unwrap(), None);
}

#[test]
fn the_scoresheet_round_trips_and_saving_again_replaces_it() {
    let mut w = world();
    w.store
        .save_scoresheet(w.ana, w.ana_game, &sheet(&[(1, "e4"), (2, "e5")]))
        .unwrap();
    let loaded = w.store.load_scoresheet(w.ana, w.ana_game).unwrap().unwrap();
    assert_eq!(
        loaded.entries().collect::<Vec<_>>(),
        vec![(1, "e4"), (2, "e5")]
    );
    assert!(!loaded.is_delivered());

    // Apagar uma anotação e entregar: a segunda gravação substitui a primeira.
    let mut next = sheet(&[(1, "e4")]);
    next.deliver(42).unwrap();
    w.store.save_scoresheet(w.ana, w.ana_game, &next).unwrap();
    let loaded = w.store.load_scoresheet(w.ana, w.ana_game).unwrap().unwrap();
    assert_eq!(loaded, next);
    assert_eq!(loaded.delivered_at_ms(), Some(42));
}

#[test]
fn two_profiles_never_see_each_others_scoresheets() {
    let mut w = world();
    w.store
        .save_scoresheet(w.ana, w.ana_game, &sheet(&[(1, "e4")]))
        .unwrap();
    w.store
        .save_scoresheet(w.beto, w.beto_game, &sheet(&[(1, "d4")]))
        .unwrap();
    assert_eq!(
        w.store
            .load_scoresheet(w.ana, w.ana_game)
            .unwrap()
            .unwrap()
            .text(1),
        Some("e4")
    );
    assert_eq!(
        w.store
            .load_scoresheet(w.beto, w.beto_game)
            .unwrap()
            .unwrap()
            .text(1),
        Some("d4")
    );
    // Cruzar perfil e Partida é recusado, para ler e para gravar.
    assert!(matches!(
        w.store.load_scoresheet(w.beto, w.ana_game),
        Err(StoreError::WrongProfile { .. })
    ));
    assert!(matches!(
        w.store
            .save_scoresheet(w.beto, w.ana_game, &sheet(&[(1, "x")])),
        Err(StoreError::WrongProfile { .. })
    ));
    assert_eq!(
        w.store
            .load_scoresheet(w.ana, w.ana_game)
            .unwrap()
            .unwrap()
            .text(1),
        Some("e4"),
        "a gravação recusada não mexeu em nada"
    );
}

#[test]
fn saving_a_scoresheet_never_touches_the_game_events_or_pgn() {
    let mut w = world();
    let before_pgn = pgn(&w.store, w.ana_game);
    let before_events = events_dump(&w);
    let before_summary = w.store.list_games(&Default::default()).unwrap();

    w.store
        .save_scoresheet(
            w.ana,
            w.ana_game,
            &sheet(&[(1, "e4"), (2, "e5"), (3, "Cf3")]),
        )
        .unwrap();
    let mut delivered = sheet(&[(1, "e4")]);
    delivered.deliver(9).unwrap();
    w.store
        .save_scoresheet(w.ana, w.ana_game, &delivered)
        .unwrap();
    w.store.delete_scoresheet(w.ana, w.ana_game).unwrap();
    w.store
        .save_scoresheet(w.ana, w.ana_game, &sheet(&[(1, "d4")]))
        .unwrap();

    assert_eq!(pgn(&w.store, w.ana_game), before_pgn);
    assert_eq!(events_dump(&w), before_events);
    assert_eq!(
        w.store.list_games(&Default::default()).unwrap(),
        before_summary
    );
    assert!(
        !before_pgn.contains("d4") && !before_pgn.contains("Cf3"),
        "a Súmula, certa ou errada, não vaza para o PGN"
    );
}

#[test]
fn deleting_the_game_or_the_scoresheet_removes_only_that_record() {
    let mut w = world();
    w.store
        .save_scoresheet(w.ana, w.ana_game, &sheet(&[(1, "e4")]))
        .unwrap();
    w.store
        .save_scoresheet(w.beto, w.beto_game, &sheet(&[(1, "d4")]))
        .unwrap();
    w.store.delete_scoresheet(w.ana, w.ana_game).unwrap();
    assert_eq!(w.store.load_scoresheet(w.ana, w.ana_game).unwrap(), None);
    assert!(
        w.store
            .load_scoresheet(w.beto, w.beto_game)
            .unwrap()
            .is_some()
    );
    // Excluir o perfil leva as Súmulas dele (cascata), e só as dele.
    w.store.delete_profile(w.beto).unwrap();
    let sheets: i64 = raw(&w, "SELECT COUNT(*) FROM manual_scoresheets");
    let entries: i64 = raw(&w, "SELECT COUNT(*) FROM manual_scoresheet_entries");
    assert_eq!((sheets, entries), (0, 0), "nenhuma Súmula órfã");
}

#[test]
fn an_unknown_game_is_an_error() {
    let mut w = world();
    assert!(matches!(
        w.store
            .save_scoresheet(w.ana, GameId(999), &sheet(&[(1, "e4")])),
        Err(StoreError::GameNotFound(_))
    ));
}
