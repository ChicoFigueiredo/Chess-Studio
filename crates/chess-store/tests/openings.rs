#![allow(clippy::unwrap_used)]
//! Explorador de aberturas (C19, #147): estatística por lance candidato, transposição,
//! indexação incremental, isolamento por perfil local e desempenho da consulta.

mod common;

use chess_core::CanonicalPosition;
use chess_game::{GameSetup, PolicyKind};
use chess_store::{GameMetadata, OpeningMoveStats, ProfileId, Store};
use common::{TempDb, play};
use std::time::Instant;

const START: &str = "rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 0 1";

fn pgn(result: &str, moves: &str) -> String {
    format!(
        "[Event \"t\"]\n[White \"A\"]\n[Black \"B\"]\n[Result \"{result}\"]\n\n{moves} {result}\n\n"
    )
}

fn fen_after(sans: &str) -> String {
    let mut p = CanonicalPosition::start();
    for san in sans.split_whitespace().filter(|t| !t.ends_with('.')) {
        let mv = p.parse_san(san).unwrap();
        p = p.play(mv).unwrap();
    }
    p.to_fen()
}

fn stat<'a>(stats: &'a [OpeningMoveStats], san: &str) -> &'a OpeningMoveStats {
    stats.iter().find(|s| s.san == san).unwrap()
}

fn count_rows(db: &TempDb) -> i64 {
    let conn = rusqlite::Connection::open(db.path()).unwrap();
    conn.query_row("SELECT COUNT(*) FROM opening_moves", [], |r| r.get(0))
        .unwrap()
}

fn store_with_imports(games: &[(&str, &str)]) -> (Store, ProfileId) {
    let mut store = Store::open_in_memory().unwrap();
    let profile = store.default_profile().unwrap();
    let text: String = games.iter().map(|(r, m)| pgn(r, m)).collect();
    let report = store.import_pgn_for(profile, &text, None).unwrap();
    assert!(report.errors.is_empty(), "{:?}", report.errors);
    (store, profile)
}

#[test]
fn counts_games_results_and_orders_by_frequency() {
    let (store, p) = store_with_imports(&[
        ("1-0", "1. e4 e5 2. Nf3"),
        ("0-1", "1. e4 c5"),
        ("1/2-1/2", "1. e4 e5"),
        ("1-0", "1. d4 d5"),
        ("*", "1. e4 e6"),
    ]);
    let root = store.opening_stats(p, START).unwrap();
    let sans: Vec<&str> = root.iter().map(|s| s.san.as_str()).collect();
    assert_eq!(sans, ["e4", "d4"], "mais frequente primeiro");
    let e4 = stat(&root, "e4");
    assert_eq!(
        (
            e4.games,
            e4.white_wins,
            e4.draws,
            e4.black_wins,
            e4.unfinished
        ),
        (4, 1, 1, 1, 1)
    );
    assert_eq!(e4.decided(), 3);
    let (w, d, b) = e4.shares();
    assert!(
        (w - 1.0 / 3.0).abs() < 1e-6
            && (d - 1.0 / 3.0).abs() < 1e-6
            && (b - 1.0 / 3.0).abs() < 1e-6
    );

    let after_e4 = store.opening_stats(p, &fen_after("1. e4")).unwrap();
    assert_eq!(stat(&after_e4, "e5").games, 2);
    assert_eq!(stat(&after_e4, "c5").black_wins, 1);
    assert_eq!(after_e4[0].san, "e5");
}

#[test]
fn position_without_games_has_no_moves() {
    let (store, p) = store_with_imports(&[("1-0", "1. e4 e5")]);
    assert!(
        store
            .opening_stats(p, &fen_after("1. a4"))
            .unwrap()
            .is_empty()
    );
}

#[test]
fn clocks_in_the_fen_do_not_matter() {
    let (store, p) = store_with_imports(&[("1-0", "1. e4 e5")]);
    let with_clocks = "rnbqkbnr/pppppppp/8/8/4P3/8/PPPP1PPP/RNBQKBNR b KQkq - 17 33";
    let stats = store.opening_stats(p, with_clocks).unwrap();
    assert_eq!(stats.len(), 1);
    assert_eq!(stats[0].san, "e5");
}

#[test]
fn transposed_games_share_the_position() {
    let (store, p) = store_with_imports(&[
        ("1-0", "1. d4 Nf6 2. c4 e6 3. Nc3"),
        ("0-1", "1. c4 Nf6 2. d4 e6 3. Nf3"),
    ]);
    let stats = store
        .opening_stats(p, &fen_after("1. d4 Nf6 2. c4 e6"))
        .unwrap();
    assert_eq!(stats.len(), 2);
    assert_eq!(stat(&stats, "Nc3").white_wins, 1);
    assert_eq!(stat(&stats, "Nf3").black_wins, 1);
}

#[test]
fn variations_are_not_counted_as_games() {
    let (store, p) = store_with_imports(&[("1-0", "1. e4 (1. d4 d5) e5 2. Nf3")]);
    let root = store.opening_stats(p, START).unwrap();
    assert_eq!(root.len(), 1);
    assert_eq!(root[0].san, "e4");
}

#[test]
fn played_games_count_too_and_index_when_they_end() {
    let db = TempDb::new("openings-played");
    let mut store = Store::open(db.path()).unwrap();
    let p = store.default_profile().unwrap();
    // Mate do louco: 1. f3 e5 2. g4 Qh4#
    let (game, events) = play(
        GameSetup::standard(PolicyKind::Assisted),
        &[("f2", "f3"), ("e7", "e5"), ("g2", "g4"), ("d8", "h4")],
    );
    let id = store
        .create_game_for(p, game.setup(), &GameMetadata::default())
        .unwrap();
    store.append_events(id, &events).unwrap();
    // Já indexada no fim da Partida (sem nenhuma busca da Biblioteca antes).
    assert_eq!(count_rows(&db), 4);
    let root = store.opening_stats(p, START).unwrap();
    assert_eq!(root[0].san, "f3");
    assert_eq!(root[0].black_wins, 1);
    assert!(root[0].last_used_ms > 0);
}

#[test]
fn import_feeds_the_index_incrementally() {
    let db = TempDb::new("openings-import");
    let mut store = Store::open(db.path()).unwrap();
    let p = store.default_profile().unwrap();
    store
        .import_pgn_for(p, &pgn("1-0", "1. e4 e5"), None)
        .unwrap();
    assert_eq!(count_rows(&db), 2, "indexado já na importação");
    assert_eq!(store.opening_stats(p, START).unwrap()[0].games, 1);
    // Sem nenhuma consulta no meio, a importação seguinte já está no índice.
    store
        .import_pgn_for(p, &pgn("0-1", "1. e4 c5"), None)
        .unwrap();
    assert_eq!(count_rows(&db), 4);
    assert_eq!(store.opening_stats(p, START).unwrap()[0].games, 2);
}

#[test]
fn deleting_a_game_removes_its_moves() {
    let db = TempDb::new("openings-delete");
    let mut store = Store::open(db.path()).unwrap();
    let p = store.default_profile().unwrap();
    let text = format!("{}{}", pgn("1-0", "1. e4 e5"), pgn("1-0", "1. d4 d5"));
    let report = store.import_pgn_for(p, &text, None).unwrap();
    assert_eq!(store.opening_stats(p, START).unwrap().len(), 2);
    store.delete_imported(p, report.imported[0]).unwrap();
    let left = store.opening_stats(p, START).unwrap();
    assert_eq!(left.len(), 1);
    assert_eq!(count_rows(&db), 2);
}

#[test]
fn rebuild_reproduces_the_same_stats() {
    let (store, p) = store_with_imports(&[
        ("1-0", "1. e4 e5 2. Nf3"),
        ("0-1", "1. e4 c5"),
        ("1/2-1/2", "1. d4 d5"),
    ]);
    let before = store.opening_stats(p, START).unwrap();
    store.rebuild_opening_index().unwrap();
    assert_eq!(store.opening_stats(p, START).unwrap(), before);
}

#[test]
fn profiles_never_mix_in_stats() {
    let mut store = Store::open_in_memory().unwrap();
    let ana = store.default_profile().unwrap();
    let beto = store.create_profile("Beto").unwrap();
    store
        .import_pgn_for(ana, &pgn("1-0", "1. e4 e5"), None)
        .unwrap();
    store
        .import_pgn_for(
            beto,
            &format!("{}{}", pgn("0-1", "1. d4 d5"), pgn("0-1", "1. d4 Nf6")),
            None,
        )
        .unwrap();
    let a = store.opening_stats(ana, START).unwrap();
    let b = store.opening_stats(beto, START).unwrap();
    assert_eq!(a.iter().map(|s| s.san.as_str()).collect::<Vec<_>>(), ["e4"]);
    assert_eq!(b.iter().map(|s| s.san.as_str()).collect::<Vec<_>>(), ["d4"]);
    assert_eq!(b[0].games, 2);
    // Perfil inexistente: nada.
    assert!(
        store
            .opening_stats(ProfileId(999), START)
            .unwrap()
            .is_empty()
    );
    // Apagar o perfil leva o índice junto.
    store.rebuild_opening_index().unwrap();
    assert_eq!(store.opening_stats(ana, START).unwrap().len(), 1);
}

#[test]
fn invalid_fen_is_a_typed_error() {
    let store = Store::open_in_memory().unwrap();
    let p = store.default_profile().unwrap();
    assert!(store.opening_stats(p, "isto não é FEN").is_err());
}

// --- desempenho -------------------------------------------------------------------------------

/// Partidas pseudoaleatórias determinísticas (LCG), com as primeiras jogadas concentradas
/// para que as Posições iniciais se repitam, como numa base real.
fn synthetic_pgn(games: usize, plies: usize) -> String {
    let mut seed: u64 = 0x9e37_79b9_7f4a_7c15;
    let mut next = move || {
        seed = seed
            .wrapping_mul(6_364_136_223_846_793_005)
            .wrapping_add(1_442_695_040_888_963_407);
        (seed >> 33) as usize
    };
    let mut out = String::new();
    for g in 0..games {
        let mut position = CanonicalPosition::start();
        let mut text = String::new();
        for ply in 0..plies {
            let legal = position.legal_moves();
            if legal.is_empty() {
                break;
            }
            let pick = if ply < 6 {
                next() % 4.min(legal.len())
            } else {
                next() % legal.len()
            };
            let mv = legal[pick];
            let san = position.san(mv).unwrap();
            if ply % 2 == 0 {
                text.push_str(&format!("{}. ", ply / 2 + 1));
            }
            text.push_str(&san);
            text.push(' ');
            position = position.play(mv).unwrap();
        }
        let result = ["1-0", "0-1", "1/2-1/2"][g % 3];
        out.push_str(&pgn(result, text.trim()));
    }
    out
}

fn measure(
    games: usize,
) -> (
    std::time::Duration,
    std::time::Duration,
    std::time::Duration,
) {
    let mut store = Store::open_in_memory().unwrap();
    let p = store.default_profile().unwrap();
    let text = synthetic_pgn(games, 40);
    let t = Instant::now();
    let report = store.import_pgn_for(p, &text, None).unwrap();
    let import = t.elapsed();
    assert_eq!(report.imported.len(), games);
    // Consulta com o índice quente (nada novo a indexar), na raiz e numa Posição profunda.
    let t = Instant::now();
    let root = store.opening_stats(p, START).unwrap();
    let root_time = t.elapsed();
    assert!(!root.is_empty());
    assert_eq!(root.iter().map(|s| s.games as usize).sum::<usize>(), games);
    let t = Instant::now();
    store
        .opening_stats(p, &fen_after("1. e4 e5 2. Nf3"))
        .unwrap();
    let deep = t.elapsed();
    (import, root_time, deep.max(root_time))
}

#[test]
fn query_is_fast_with_a_few_hundred_games() {
    let (import, root, worst) = measure(500);
    eprintln!(
        "500 partidas: importação+indexação {import:?}, consulta raiz {root:?}, pior {worst:?}"
    );
    // Medido: ~9 ms. O teto de 50 ms (spec) vale no teste de 10 mil Partidas, `--ignored`;
    // aqui a margem larga só evita falso positivo com a máquina carregada (#236).
    assert!(worst.as_millis() < 250, "consulta lenta: {worst:?}");
}

#[test]
#[ignore = "lento: importa 10 mil Partidas; rode com --ignored --release/--nocapture"]
fn query_under_50ms_with_10_thousand_games() {
    let (import, root, worst) = measure(10_000);
    eprintln!(
        "10000 partidas: importação+indexação {import:?}, consulta raiz {root:?}, pior {worst:?}"
    );
    assert!(worst.as_millis() < 50, "consulta lenta: {worst:?}");
}
