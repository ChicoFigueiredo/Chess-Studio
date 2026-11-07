#![allow(clippy::unwrap_used)]
//! Biblioteca, Favoritas e importadas isoladas por perfil local (#114): listar, buscar,
//! abrir e apagar nunca cruzam perfis.

mod common;

use chess_game::{GameSetup, PolicyKind};
use chess_store::{
    FavoriteId, FavoriteQuery, GameMetadata, ImportedId, ImportedQuery, LibraryQuery, NewFavorite,
    ProfileId, Store, StoreError,
};
use common::play;

const PGN: &str = "[Event \"Estudo\"]\n[White \"Ana\"]\n[Black \"Beto\"]\n[Result \"1-0\"]\n\n1. e4 e5 2. Nf3 1-0\n";
const FEN: &str = "rnbqkbnr/pppp1ppp/8/4p3/4P3/8/PPPP1PPP/RNBQKBNR w KQkq - 0 2";

struct World {
    store: Store,
    ana: ProfileId,
    beto: ProfileId,
    ana_imported: ImportedId,
    ana_favorite: FavoriteId,
    ana_game: chess_store::GameId,
}

fn world() -> World {
    let mut store = Store::open_in_memory().unwrap();
    let ana = store.default_profile().unwrap();
    let beto = store.create_profile("Beto").unwrap();
    let ana_imported = store.import_pgn_for(ana, PGN, None).unwrap().imported[0];
    let ana_favorite = store
        .add_favorite_for(
            ana,
            &NewFavorite {
                fen: FEN.into(),
                name: "Aberto".into(),
                note: String::new(),
                origin: None,
            },
        )
        .unwrap();
    let (game, events) = play(
        GameSetup::standard(PolicyKind::Assisted),
        &[("e2", "e4"), ("e7", "e5")],
    );
    let ana_game = store
        .create_game_for(ana, game.setup(), &GameMetadata::default())
        .unwrap();
    store.append_events(ana_game, &events).unwrap();
    World {
        store,
        ana,
        beto,
        ana_imported,
        ana_favorite,
        ana_game,
    }
}

#[test]
fn listagem_e_busca_filtram_pelo_perfil() {
    let w = world();
    let search = |p| {
        w.store
            .search_library(&LibraryQuery {
                profile: Some(p),
                ..Default::default()
            })
            .unwrap()
            .total
    };
    assert_eq!(search(w.ana), 2);
    assert_eq!(search(w.beto), 0);
    let by_position = w
        .store
        .search_library(&LibraryQuery {
            profile: Some(w.beto),
            position: Some(FEN.into()),
            ..Default::default()
        })
        .unwrap();
    assert_eq!(by_position.total, 0);
    let imported = |p| {
        w.store
            .list_imported(&ImportedQuery {
                profile: Some(p),
                ..Default::default()
            })
            .unwrap()
            .len()
    };
    assert_eq!((imported(w.ana), imported(w.beto)), (1, 0));
    let favorites = |p| {
        w.store
            .list_favorites(&FavoriteQuery {
                profile: Some(p),
                ..Default::default()
            })
            .unwrap()
            .len()
    };
    assert_eq!((favorites(w.ana), favorites(w.beto)), (1, 0));
}

#[test]
fn abrir_e_apagar_importada_de_outro_perfil_e_erro_tipado() {
    let w = world();
    assert!(matches!(
        w.store.load_imported(w.beto, w.ana_imported),
        Err(StoreError::WrongProfile { .. })
    ));
    assert!(matches!(
        w.store.delete_imported(w.beto, w.ana_imported),
        Err(StoreError::WrongProfile { .. })
    ));
    assert!(w.store.load_imported(w.ana, w.ana_imported).is_ok());
    assert!(matches!(
        w.store.load_imported(w.ana, ImportedId(999)),
        Err(StoreError::ImportedGameNotFound(_))
    ));
    w.store.delete_imported(w.ana, w.ana_imported).unwrap();
}

#[test]
fn favoritas_de_outro_perfil_nao_sao_lidas_nem_alteradas_nem_apagadas() {
    let w = world();
    assert!(matches!(
        w.store.get_favorite(w.beto, w.ana_favorite),
        Err(StoreError::WrongProfile { .. })
    ));
    assert!(matches!(
        w.store.update_favorite(w.beto, w.ana_favorite, "x", "y"),
        Err(StoreError::WrongProfile { .. })
    ));
    assert!(matches!(
        w.store.delete_favorite(w.beto, w.ana_favorite),
        Err(StoreError::WrongProfile { .. })
    ));
    assert_eq!(w.store.find_favorite(w.beto, FEN).unwrap(), None);
    assert_eq!(
        w.store.find_favorite(w.ana, FEN).unwrap(),
        Some(w.ana_favorite)
    );
    let intact = w.store.get_favorite(w.ana, w.ana_favorite).unwrap();
    assert_eq!(intact.name, "Aberto");
    w.store.delete_favorite(w.ana, w.ana_favorite).unwrap();
}

#[test]
fn partida_jogada_de_outro_perfil_nao_abre() {
    let w = world();
    assert!(matches!(
        w.store.open_game_for(w.beto, w.ana_game),
        Err(StoreError::WrongProfile { .. })
    ));
    assert!(w.store.open_game_for(w.ana, w.ana_game).is_ok());
}
