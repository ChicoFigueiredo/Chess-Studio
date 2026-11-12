#![allow(clippy::unwrap_used)]
//! Partida online no log (ADR-0002, ADR-0007, #230): grava e reabre igual, em andamento
//! (correspondência) ou terminada; abortada conta como terminada, sem resultado.

mod common;
use chess_core::Color;
use chess_game::{
    Command, EndReason, Game, GameResult, GameSetup, OnlineGame, OnlinePlayer, Opponent, Outcome,
    ProviderClock, ProviderEnd, ProviderSnapshot, TimeControl, Timestamp, Variant,
};
use chess_store::{GameMetadata, GameQuery, Store};
use common::TempDb;

fn setup() -> GameSetup {
    let player = |name: &str| OnlinePlayer {
        name: name.into(),
        rating: Some(1500),
    };
    GameSetup::online(
        OnlineGame {
            provider: "lichess".into(),
            remote_id: "Abc12345".into(),
            local_color: Color::Black,
            opponent: Opponent::Human,
            white: player("ana"),
            black: player("bia"),
            rated: true,
            days_per_move: None,
        },
        Variant::Standard,
        None,
    )
    .unwrap()
    .with_time_control(TimeControl {
        base_ms: 600_000,
        increment_ms: 0,
    })
}

fn snapshot(moves: &str, end: Option<ProviderEnd>) -> ProviderSnapshot {
    ProviderSnapshot {
        moves: moves.split_whitespace().map(str::to_owned).collect(),
        clock: Some(ProviderClock {
            white_ms: 590_000,
            black_ms: 595_000,
            running: true,
        }),
        draw_offer: None,
        end,
    }
}

#[test]
fn partida_online_em_andamento_reabre_igual_do_banco() {
    let db = TempDb::new("online");
    let mut game = Game::new(setup());
    let id = {
        let mut store = Store::open(db.path()).unwrap();
        let id = store
            .create_game(game.setup(), &GameMetadata::default())
            .unwrap();
        let events = game
            .handle(
                Command::Provider(snapshot("e2e4 e7e5 g1f3", None)),
                Timestamp::with_wall(1_000, 1_794_398_400_000),
            )
            .unwrap();
        store.append_events(id, &events).unwrap();
        store.close_game(id).unwrap();
        id
    };
    let store = Store::open(db.path()).unwrap();
    let opened = store.open_game(id).unwrap();
    assert_eq!(opened.game.snapshot(), game.snapshot());
    assert_eq!(opened.game.setup(), game.setup());
    assert!(opened.game.online_human_in_progress());
    let summary = &store.list_games(&GameQuery::default()).unwrap()[0];
    assert!(!summary.finished);
    assert_eq!(store.latest_unfinished_game().unwrap(), Some(id));
}

#[test]
fn partida_online_abortada_conta_como_terminada_sem_resultado() {
    let db = TempDb::new("online-aborted");
    let mut store = Store::open(db.path()).unwrap();
    let mut game = Game::new(setup());
    let id = store
        .create_game(game.setup(), &GameMetadata::default())
        .unwrap();
    let events = game
        .handle(
            Command::Provider(snapshot("e2e4", Some(ProviderEnd::Aborted))),
            Timestamp::mono(10),
        )
        .unwrap();
    store.append_events(id, &events).unwrap();
    let summary = &store.list_games(&GameQuery::default()).unwrap()[0];
    assert!(summary.finished, "abortada não é Partida a retomar");
    assert_eq!(summary.outcome, None);
    assert_eq!(store.latest_unfinished_game().unwrap(), None);
    let loaded = store.load_game(id).unwrap();
    assert!(loaded.snapshot().is_over());
    assert_eq!(loaded.snapshot(), game.snapshot());
}

#[test]
fn partida_online_terminada_pelo_provedor_tem_resultado_no_banco() {
    let db = TempDb::new("online-ended");
    let mut store = Store::open(db.path()).unwrap();
    let mut game = Game::new(setup());
    let id = store
        .create_game(game.setup(), &GameMetadata::default())
        .unwrap();
    let outcome = Outcome {
        result: GameResult::WhiteWins,
        reason: EndReason::Disconnection,
    };
    let events = game
        .handle(
            Command::Provider(snapshot("e2e4 e7e5", Some(ProviderEnd::Decided(outcome)))),
            Timestamp::mono(10),
        )
        .unwrap();
    store.append_events(id, &events).unwrap();
    let summary = &store.list_games(&GameQuery::default()).unwrap()[0];
    assert!(summary.finished);
    assert_eq!(summary.outcome, Some(outcome));
}

/// FC-22 (#272): a Partida do FICS que caiu, encerrada pelo jogador no app: terminada, sem
/// resultado, e a Biblioteca a conta como terminada.
#[test]
fn partida_online_encerrada_sem_resultado_conta_como_terminada_sem_resultado() {
    let db = TempDb::new("online-closed");
    let mut store = Store::open(db.path()).unwrap();
    let mut game = Game::new(setup());
    let id = store
        .create_game(game.setup(), &GameMetadata::default())
        .unwrap();
    let events = game
        .handle(
            Command::Provider(snapshot("e2e4 e7e5", None)),
            Timestamp::mono(10),
        )
        .unwrap();
    store.append_events(id, &events).unwrap();
    assert!(!store.list_games(&GameQuery::default()).unwrap()[0].finished);
    let closed = game
        .handle(Command::CloseWithoutResult, Timestamp::mono(20))
        .unwrap();
    store.append_events(id, &closed).unwrap();
    let summary = &store.list_games(&GameQuery::default()).unwrap()[0];
    assert!(summary.finished, "encerrada não é Partida a retomar");
    assert_eq!(summary.outcome, None, "nenhum resultado inventado");
    assert_eq!(store.latest_unfinished_game().unwrap(), None);
    let loaded = store.load_game(id).unwrap();
    assert_eq!(loaded.snapshot(), game.snapshot());
    assert!(!loaded.online_human_in_progress());
    let library = store
        .search_library(&chess_store::LibraryQuery::default())
        .unwrap();
    let entry = &library.entries[0];
    assert!(entry.finished);
    assert_eq!(entry.result, "*");
}
