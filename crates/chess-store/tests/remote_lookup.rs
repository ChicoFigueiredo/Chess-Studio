#![allow(clippy::unwrap_used)]
//! Achar a Partida importada pelo id do Provedor (#239).

use chess_store::{ImportMode, ImportOptions, ImportSource, ProfileId, Provenance, Store};

const ME: ProfileId = ProfileId(1);

fn lichess_game(id: &str) -> String {
    format!(
        "[Event \"Rated Blitz game\"]\n[Site \"https://lichess.org/{id}\"]\n[White \"ana\"]\n[Black \"beto\"]\n[Date \"2026.11.07\"]\n[Result \"1-0\"]\n\n1. e4 e5 2. Nf3 1-0\n\n"
    )
}

fn api() -> ImportOptions {
    ImportOptions::new(
        None,
        Provenance::from_source(ImportSource::Lichess, ImportMode::Api, 5_000),
    )
}

#[test]
fn a_game_imported_from_the_api_is_found_by_its_remote_id() {
    let mut store = Store::open_in_memory().unwrap();
    let report = store
        .import_pgn_with(ME, &lichess_game("AbCd1234"), &api())
        .unwrap();
    let id = report.imported[0];
    assert_eq!(
        store
            .imported_by_remote_id(ME, "lichess", "AbCd1234")
            .unwrap(),
        Some(id)
    );
}

#[test]
fn another_id_another_provider_or_another_profile_finds_nothing() {
    let mut store = Store::open_in_memory().unwrap();
    store
        .import_pgn_with(ME, &lichess_game("AbCd1234"), &api())
        .unwrap();
    assert_eq!(
        store
            .imported_by_remote_id(ME, "lichess", "ZZZZ9999")
            .unwrap(),
        None
    );
    assert_eq!(
        store
            .imported_by_remote_id(ME, "chess-com", "AbCd1234")
            .unwrap(),
        None
    );
    assert_eq!(
        store
            .imported_by_remote_id(ProfileId(2), "lichess", "AbCd1234")
            .unwrap(),
        None
    );
}

#[test]
fn the_same_game_from_a_file_and_later_the_api_is_found_through_the_added_origin() {
    let mut store = Store::open_in_memory().unwrap();
    let file = ImportOptions::new(
        Some("minhas.pgn"),
        Provenance::unknown(ImportMode::File, 1_000),
    );
    let first = store
        .import_pgn_with(ME, &lichess_game("AbCd1234"), &file)
        .unwrap();
    let second = store
        .import_pgn_with(ME, &lichess_game("AbCd1234"), &api())
        .unwrap();
    assert_eq!(second.duplicates, 1, "não vira cópia");
    assert_eq!(
        store
            .imported_by_remote_id(ME, "lichess", "AbCd1234")
            .unwrap(),
        Some(first.imported[0])
    );
}
