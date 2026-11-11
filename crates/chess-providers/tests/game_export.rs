//! A Partida de referência do Explorador, pelo id (#239): sempre contra o servidor falso.

mod common;

use chess_net::ProviderId;
use chess_net::testing::FakeResponse;
use chess_providers::{ProviderError, import_options};
use chess_store::{ProfileId, Store};
use common::*;

const PGN: &str = "[Event \"Rated Blitz game\"]\n[Site \"https://lichess.org/AbCd1234\"]\n[White \"ana\"]\n[Black \"beto\"]\n[Date \"2026.11.07\"]\n[Result \"1-0\"]\n\n1. e4 e5 2. Nf3 Nc6 1-0\n";

#[test]
fn exports_the_game_by_id_with_its_origin() {
    let env = env_enabled();
    env.server.route(
        "/game/export/AbCd1234",
        FakeResponse::ok(PGN).header("content-type", "application/x-chess-pgn"),
    );

    let answer = env.lichess.game_pgn("AbCd1234").wait().expect("a Partida");

    assert_eq!(answer.value.id, "AbCd1234");
    assert!(answer.value.pgn.contains("1. e4 e5"));
    assert_eq!(answer.provenance.provider, ProviderId::Lichess);
    assert!(answer.provenance.source.contains("/game/export/AbCd1234"));
    let sent = &env.server.requests()[0];
    assert_eq!(sent.query_value("clocks").as_deref(), Some("false"));
    assert_eq!(sent.header("accept"), Some("application/x-chess-pgn"));
}

#[test]
fn an_exported_game_is_imported_once_even_if_opened_twice() {
    let env = env_enabled();
    env.server
        .route("/game/export/AbCd1234", FakeResponse::ok(PGN));
    let answer = env.lichess.game_pgn("AbCd1234").wait().expect("a Partida");
    let options = import_options(&answer.provenance);

    let mut store = Store::open_in_memory().expect("banco");
    let me = ProfileId(1);
    let first = store
        .import_pgn_with(me, &answer.value.pgn, &options)
        .expect("importa");
    let again = store
        .import_pgn_with(me, &answer.value.pgn, &options)
        .expect("importa de novo");

    assert_eq!(first.imported.len(), 1);
    assert_eq!(again.duplicates, 1);
    assert_eq!(
        store
            .imported_by_remote_id(me, "lichess", "AbCd1234")
            .expect("busca"),
        Some(first.imported[0])
    );
    let origins = store.origins(me, first.imported[0]).expect("Origem");
    assert_eq!(origins[0].provider.as_deref(), Some("lichess"));
    assert_eq!(origins[0].remote_id.as_deref(), Some("AbCd1234"));
}

#[test]
fn an_unknown_game_is_no_data_and_a_bad_id_never_reaches_the_network() {
    let env = env_enabled();
    env.server
        .route("/game/export/ZZZZ9999", FakeResponse::status(404));

    let missing = env.lichess.game_pgn("ZZZZ9999").wait();
    assert_eq!(missing.unwrap_err(), ProviderError::NoData);

    let before = env.server.requests().len();
    for bad in ["", "curto", "../../etc1", "AbCd 234", "AbCd1234/x"] {
        assert!(
            matches!(
                env.lichess.game_pgn(bad).wait(),
                Err(ProviderError::InvalidInput { .. })
            ),
            "{bad:?}"
        );
    }
    assert_eq!(env.server.requests().len(), before);
}

#[test]
fn an_answer_that_is_not_a_pgn_is_malformed() {
    let env = env_enabled();
    env.server
        .route("/game/export/AbCd1234", FakeResponse::ok("<html>oops</html>"));
    assert!(matches!(
        env.lichess.game_pgn("AbCd1234").wait(),
        Err(ProviderError::Malformed { .. })
    ));
}

#[test]
fn a_masters_game_goes_to_the_explorer_with_the_connection_token() {
    let env = env_enabled();
    env.server
        .route("/masters/pgn/AbCd1234", FakeResponse::ok(PGN));

    let answer = env
        .lichess
        .masters_game_pgn("AbCd1234")
        .wait()
        .expect("a Partida");

    assert_eq!(answer.value.id, "AbCd1234");
    let sent = &env.server.requests()[0];
    assert_eq!(
        sent.header("authorization"),
        Some(format!("Bearer {EXPLORER_TOKEN}").as_str())
    );
}
