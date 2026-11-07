#![allow(clippy::unwrap_used)]
//! Biblioteca pesquisável (#49): índice de busca sobre Partidas jogadas e importadas,
//! filtros, posições favoritas e Exercícios por tipo. DB-06 preservado.

mod common;

use chess_game::{Command, EndReason, EngineSeat, GameSetup, PolicyKind, Timestamp};
use chess_store::{
    FavoriteOrigin, FavoriteQuery, GameMetadata, LATEST_SCHEMA_VERSION, LibraryQuery,
    LibrarySource, NewFavorite, Store, StoreError, migrations,
};
use chess_training::{Catalog, ExerciseKind};
use common::{TempDb, play};

const PGN: &str = "\
[Event \"Estudo Siciliana\"]
[White \"Ana Álvares\"]
[Black \"Beto\"]
[Date \"2024.03.15\"]
[Result \"0-1\"]
[Opening \"Siciliana\"]

1. e4 c5 2. Nf3 d6 3. d4 cxd4 0-1

[Event \"Estudo Italiana\"]
[White \"Carla\"]
[Black \"Davi\"]
[Date \"2023.??.??\"]
[Result \"1-0\"]

1. e4 e5 2. Nf3 Nc6 3. Bc4 Bc5 (3... Nf6 4. d3) 1-0
";

fn played(store: &mut Store, policy: PolicyKind, moves: &[(&str, &str)]) -> chess_store::GameId {
    let (game, events) = play(GameSetup::standard(policy), moves);
    let id = store
        .create_game(game.setup(), &GameMetadata::default())
        .unwrap();
    store.append_events(id, &events).unwrap();
    id
}

fn seeded() -> Store {
    let mut store = Store::open_in_memory().unwrap();
    store.import_pgn(PGN, Some("estudos.pgn")).unwrap();
    played(
        &mut store,
        PolicyKind::Assisted,
        &[("e2", "e4"), ("e7", "e5"), ("g1", "f3")],
    );
    store
}

#[test]
fn a_busca_cobre_partidas_jogadas_e_importadas() {
    let store = seeded();
    let all = store.search_library(&LibraryQuery::default()).unwrap();
    assert_eq!(all.total, 3);
    assert_eq!(all.entries.len(), 3);
    assert!(
        all.entries
            .iter()
            .any(|e| e.source == LibrarySource::Played)
    );
    assert_eq!(
        all.entries
            .iter()
            .filter(|e| e.source == LibrarySource::Imported)
            .count(),
        2
    );
}

#[test]
fn partidas_sem_nenhum_lance_nao_entram_na_biblioteca() {
    let mut store = Store::open_in_memory().unwrap();
    store
        .create_game(
            &GameSetup::standard(PolicyKind::Assisted),
            &GameMetadata::default(),
        )
        .unwrap();
    let page = store.search_library(&LibraryQuery::default()).unwrap();
    assert_eq!(page.total, 0);
}

#[test]
fn texto_livre_acha_evento_jogador_e_abertura_sem_distinguir_acentos() {
    let store = seeded();
    let q = |text: &str| {
        store
            .search_library(&LibraryQuery {
                text: Some(text.into()),
                ..Default::default()
            })
            .unwrap()
    };
    assert_eq!(q("siciliana").total, 1);
    assert_eq!(q("alvares").total, 1, "sem acento acha 'Álvares'");
    assert_eq!(q("Sici").total, 1, "prefixo");
    // Abertura pelos primeiros Lances: "e4 e5 Nf3" existe na jogada e na Italiana.
    assert_eq!(q("e4 e5 Nf3").total, 2);
    assert_eq!(q("e4 c5").total, 1);
    assert_eq!(q("inexistente").total, 0);
    assert_eq!(q("  \"  ").total, 3, "só pontuação não filtra");
}

#[test]
fn filtros_por_jogador_resultado_politica_e_data() {
    let mut store = seeded();
    played(
        &mut store,
        PolicyKind::FideSimulation,
        &[("d2", "d4"), ("d7", "d5")],
    );
    let page = |query: LibraryQuery| store.search_library(&query).unwrap();

    let by_player = page(LibraryQuery {
        player: Some("davi".into()),
        ..Default::default()
    });
    assert_eq!(by_player.total, 1);
    assert_eq!(by_player.entries[0].white.as_deref(), Some("Carla"));

    let wins = page(LibraryQuery {
        result: Some("0-1".into()),
        ..Default::default()
    });
    assert_eq!(wins.total, 1);
    assert_eq!(wins.entries[0].event.as_deref(), Some("Estudo Siciliana"));

    let fide = page(LibraryQuery {
        policy: Some(PolicyKind::FideSimulation),
        ..Default::default()
    });
    assert_eq!(fide.total, 1);
    assert_eq!(fide.entries[0].source, LibrarySource::Played);
    assert_eq!(fide.entries[0].policy, Some(PolicyKind::FideSimulation));

    let in_2024 = page(LibraryQuery {
        date_from: Some("2024.01.01".into()),
        date_to: Some("2024.12.31".into()),
        source: Some(LibrarySource::Imported),
        ..Default::default()
    });
    assert_eq!(in_2024.total, 1);
    assert_eq!(in_2024.entries[0].date.as_deref(), Some("2024.03.15"));

    // Data parcial (`2023.??.??`) entra em qualquer intervalo que cruze o ano.
    let in_2023 = page(LibraryQuery {
        date_from: Some("2023.06.01".into()),
        date_to: Some("2023.06.30".into()),
        source: Some(LibrarySource::Imported),
        ..Default::default()
    });
    assert_eq!(in_2023.total, 1);
    assert_eq!(in_2023.entries[0].event.as_deref(), Some("Estudo Italiana"));
}

#[test]
fn partida_jogada_traz_politica_adversario_resultado_motivo_e_abertura() {
    let mut store = Store::open_in_memory().unwrap();
    let setup = GameSetup::standard(PolicyKind::Assisted).with_engine(EngineSeat {
        color: chess_core::Color::Black,
        elo: 1500,
    });
    let (mut game, mut events) = play(setup, &[("e2", "e4"), ("e7", "e5")]);
    events.extend(
        game.handle(
            Command::Resign {
                side: chess_core::Color::White,
            },
            Timestamp::mono(9_000),
        )
        .unwrap(),
    );
    let id = store
        .create_game(game.setup(), &GameMetadata::default())
        .unwrap();
    store.append_events(id, &events).unwrap();

    let page = store.search_library(&LibraryQuery::default()).unwrap();
    let e = &page.entries[0];
    assert_eq!(e.source, LibrarySource::Played);
    assert_eq!(e.ref_id, id.0);
    assert_eq!(e.result, "0-1");
    assert_eq!(e.reason, Some(EndReason::Resignation));
    assert_eq!(e.ply_count, 2);
    assert_eq!(e.opening.as_deref(), Some("1. e4 e5"));
    assert_eq!(e.opponent.as_deref(), Some("Engine 1500"));
    assert!(e.finished);
    assert_eq!(e.date.as_deref(), Some("2023.11.14"));
    // O motivo também é pesquisável pelo identificador.
    let by_reason = store
        .search_library(&LibraryQuery {
            text: Some("resignation".into()),
            ..Default::default()
        })
        .unwrap();
    assert_eq!(by_reason.total, 1);
}

#[test]
fn o_indice_acompanha_lances_novos_e_exclusoes() {
    let mut store = Store::open_in_memory().unwrap();
    let (game, events) = play(
        GameSetup::standard(PolicyKind::Assisted),
        &[("e2", "e4"), ("e7", "e5"), ("g1", "f3")],
    );
    let id = store
        .create_game(game.setup(), &GameMetadata::default())
        .unwrap();
    store.append_events(id, &events[..1]).unwrap();
    let first = store.search_library(&LibraryQuery::default()).unwrap();
    assert_eq!(first.entries[0].ply_count, 1);
    store.append_events(id, &events[1..]).unwrap();
    let second = store.search_library(&LibraryQuery::default()).unwrap();
    assert_eq!(second.total, 1);
    assert_eq!(second.entries[0].ply_count, 3);

    let report = store.import_pgn(PGN, None).unwrap();
    assert_eq!(
        store.search_library(&LibraryQuery::default()).unwrap().total,
        3
    );
    store.delete_imported(report.imported[0]).unwrap();
    let after = store.search_library(&LibraryQuery::default()).unwrap();
    assert_eq!(after.total, 2);
    assert_eq!(
        store
            .search_library(&LibraryQuery {
                text: Some("siciliana".into()),
                ..Default::default()
            })
            .unwrap()
            .total,
        0,
        "o texto da partida excluída sai do índice"
    );
}

#[test]
fn paginacao_devolve_o_total_e_fatias() {
    let mut store = Store::open_in_memory().unwrap();
    for _ in 0..5 {
        store.import_pgn("1. e4 e5 *", None).unwrap();
    }
    let page = |offset| {
        store
            .search_library(&LibraryQuery {
                limit: Some(2),
                offset,
                ..Default::default()
            })
            .unwrap()
    };
    let (a, b, c) = (page(0), page(2), page(4));
    assert_eq!((a.total, a.entries.len()), (5, 2));
    assert_eq!(b.entries.len(), 2);
    assert_eq!(c.entries.len(), 1);
    let mut ids: Vec<_> = [a, b, c]
        .iter()
        .flat_map(|p| p.entries.iter().map(|e| e.id))
        .collect();
    ids.sort();
    ids.dedup();
    assert_eq!(ids.len(), 5, "sem repetição entre páginas");
}

#[test]
fn busca_por_posicao_acha_por_fen_ignorando_contadores() {
    let store = seeded();
    // Posição após 1.e4 e5 2.Nf3 (a jogada e a Italiana a contêm).
    let fen = "rnbqkbnr/pppp1ppp/8/4p3/4P3/5N2/PPPP1PPP/RNBQKB1R b KQkq - 1 2";
    let found = store
        .search_library(&LibraryQuery {
            position: Some(fen.into()),
            ..Default::default()
        })
        .unwrap();
    assert_eq!(found.total, 2);
    assert!(found.entries.iter().all(|e| e.matched_ply == Some(3)));
    // Contadores diferentes não importam (chave normalizada).
    let again = store
        .search_library(&LibraryQuery {
            position: Some("rnbqkbnr/pppp1ppp/8/4p3/4P3/5N2/PPPP1PPP/RNBQKB1R b KQkq - 9 40".into()),
            ..Default::default()
        })
        .unwrap();
    assert_eq!(again.total, 2);
    // Posição que só existe numa variação: a partida casa, sem Lance na linha principal.
    let in_variation = store
        .search_library(&LibraryQuery {
            position: Some(
                "r1bqkb1r/pppp1ppp/2n2n2/4p3/2B1P3/5N2/PPPP1PPP/RNBQK2R w KQkq - 4 4".into(),
            ),
            ..Default::default()
        })
        .unwrap();
    assert_eq!(in_variation.total, 0, "posição inexistente não casa");
    let variation_only = store
        .search_library(&LibraryQuery {
            position: Some(
                "r1bqkbnr/pppp1ppp/2n5/4p3/2B1P3/5N2/PPPP1PPP/RNBQK2R b KQkq - 3 3".into(),
            ),
            ..Default::default()
        })
        .unwrap();
    assert_eq!(variation_only.total, 1);
    // Combina com outros filtros.
    let combined = store
        .search_library(&LibraryQuery {
            position: Some(fen.into()),
            source: Some(LibrarySource::Played),
            ..Default::default()
        })
        .unwrap();
    assert_eq!(combined.total, 1);
}

#[test]
fn posicao_so_de_variacao_casa_sem_lance_na_linha_principal() {
    let store = seeded();
    // Depois de 1.e4 e5 2.Nf3 Nc6 3.Bc4 Nf6 (variação da Italiana).
    let fen = "r1bqkb1r/pppp1ppp/2n2n2/4p3/2B1P3/5N2/PPPP1PPP/RNBQK2R w KQkq - 4 4";
    let page = store
        .search_library(&LibraryQuery {
            position: Some(fen.into()),
            ..Default::default()
        })
        .unwrap();
    assert_eq!(page.total, 1);
    assert_eq!(page.entries[0].matched_ply, None);
}

#[test]
fn fen_invalido_na_busca_por_posicao_e_erro_tipado() {
    let store = seeded();
    let err = store
        .search_library(&LibraryQuery {
            position: Some("isto não é FEN".into()),
            ..Default::default()
        })
        .unwrap_err();
    assert!(matches!(err, StoreError::InvalidFen(_)), "{err}");
}

#[test]
fn favoritas_crud_com_origem() {
    let mut store = seeded();
    let fen = "rnbqkbnr/pppp1ppp/8/4p3/4P3/8/PPPP1PPP/RNBQKBNR w KQkq - 0 2";
    let id = store
        .add_favorite(&NewFavorite {
            fen: fen.into(),
            name: "Aberto".into(),
            note: "tema central".into(),
            origin: Some(FavoriteOrigin {
                source: LibrarySource::Played,
                ref_id: 1,
                ply: 2,
            }),
        })
        .unwrap();
    let all = store.list_favorites(&FavoriteQuery::default()).unwrap();
    assert_eq!(all.len(), 1);
    assert_eq!(all[0].id, id);
    assert_eq!(all[0].name, "Aberto");
    assert_eq!(all[0].note, "tema central");
    assert_eq!(all[0].fen, fen);
    assert_eq!(
        all[0].origin,
        Some(FavoriteOrigin {
            source: LibrarySource::Played,
            ref_id: 1,
            ply: 2
        })
    );
    assert_eq!(store.find_favorite(fen).unwrap(), Some(id));
    // Mesma posição (contadores diferentes) é a mesma favorita: atualiza, não duplica.
    let same = store
        .add_favorite(&NewFavorite {
            fen: "rnbqkbnr/pppp1ppp/8/4p3/4P3/8/PPPP1PPP/RNBQKBNR w KQkq - 5 9".into(),
            name: "Aberto (renomeado)".into(),
            note: String::new(),
            origin: None,
        })
        .unwrap();
    assert_eq!(same, id);
    assert_eq!(
        store.list_favorites(&FavoriteQuery::default()).unwrap().len(),
        1
    );
    store.update_favorite(id, "Final", "nota nova").unwrap();
    let got = store.get_favorite(id).unwrap();
    assert_eq!(
        (got.name.as_str(), got.note.as_str()),
        ("Final", "nota nova")
    );
    // Busca por texto no nome ou na nota.
    let hit = store
        .list_favorites(&FavoriteQuery {
            text: Some("NOTA".into()),
            ..Default::default()
        })
        .unwrap();
    assert_eq!(hit.len(), 1);
    store.delete_favorite(id).unwrap();
    assert!(
        store
            .list_favorites(&FavoriteQuery::default())
            .unwrap()
            .is_empty()
    );
    assert!(matches!(
        store.delete_favorite(id),
        Err(StoreError::FavoriteNotFound(_))
    ));
    assert_eq!(store.find_favorite(fen).unwrap(), None);
}

#[test]
fn favorita_com_fen_invalido_e_recusada() {
    let mut store = Store::open_in_memory().unwrap();
    let err = store
        .add_favorite(&NewFavorite {
            fen: "lixo".into(),
            name: "x".into(),
            note: String::new(),
            origin: None,
        })
        .unwrap_err();
    assert!(matches!(err, StoreError::InvalidFen(_)));
    assert!(
        store
            .list_favorites(&FavoriteQuery::default())
            .unwrap()
            .is_empty()
    );
}

#[test]
fn exercicios_sao_encontraveis_por_tipo_e_texto() {
    let mut store = Store::open_in_memory().unwrap();
    let catalog = Catalog::bundled().unwrap();
    let exercises: Vec<_> = catalog.iter().cloned().collect();
    store.upsert_exercises(&exercises).unwrap();
    let mates = store
        .search_exercises(Some(ExerciseKind::Mate), None)
        .unwrap();
    assert!(!mates.is_empty());
    assert!(mates.iter().all(|e| e.kind == ExerciseKind::Mate));
    let expected = exercises
        .iter()
        .filter(|e| e.kind == ExerciseKind::Mate)
        .count();
    assert_eq!(mates.len(), expected);
    let all = store.search_exercises(None, None).unwrap();
    assert_eq!(all.len(), exercises.len());
    let title = &mates[0].title;
    let by_text = store
        .search_exercises(None, Some(&title.to_uppercase()))
        .unwrap();
    assert!(by_text.iter().any(|e| &e.title == title));
}

/// DB-06: um banco da versão anterior abre, migra e a biblioteca indexa os dados novos.
#[test]
fn banco_da_versao_anterior_migra_sem_perder_dados() {
    let db = TempDb::new("library-migrate");
    {
        let mut conn = rusqlite::Connection::open(db.path()).unwrap();
        migrations::migrate_to(&mut conn, LATEST_SCHEMA_VERSION - 1).unwrap();
        let setup = serde_json::to_string(&GameSetup::standard(PolicyKind::Assisted)).unwrap();
        conn.execute(
            "INSERT INTO games (id, profile_id, policy, start_fen, setup_json, created_at_ms, updated_at_ms, session_open)
             VALUES (7, 1, 'assisted', 'x', ?1, 1700000000000, 1700000000000, 0)",
            [&setup],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO ui_preferences (profile_id, key, value_json) VALUES (1, 'k', '1')",
            [],
        )
        .unwrap();
    }
    let mut store = Store::open(db.path()).unwrap();
    assert_eq!(store.schema_version().unwrap(), LATEST_SCHEMA_VERSION);
    assert_eq!(
        store
            .get_pref(store.default_profile().unwrap(), "k")
            .unwrap(),
        Some(serde_json::json!(1))
    );
    assert_eq!(store.list_games(&Default::default()).unwrap().len(), 1);
    // A partida antiga (sem Lances) não aparece; as novas sim.
    assert_eq!(
        store.search_library(&LibraryQuery::default()).unwrap().total,
        0
    );
    store.import_pgn("1. d4 d5 *", None).unwrap();
    assert_eq!(
        store.search_library(&LibraryQuery::default()).unwrap().total,
        1
    );
}
