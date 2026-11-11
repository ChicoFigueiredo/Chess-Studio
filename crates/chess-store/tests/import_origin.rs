#![allow(clippy::unwrap_used)]
//! CX-4 (#216): Origem estruturada, impressão digital, duplicatas, importação em fluxo,
//! encodings e filtros. Migração 13 sobre um banco da versão 12 com dados.

mod common;

use chess_pgn::parse_pgn;
use chess_store::{
    ImportMode, ImportOptions, ImportProgress, ImportSource, ImportedQuery, LATEST_SCHEMA_VERSION,
    LibraryQuery, Pace, ProfileId, Provenance, Store, game_fingerprint, migrations,
};
use common::TempDb;
use std::io::{BufReader, Cursor, Read, Write};
use std::sync::atomic::{AtomicBool, Ordering};

const ME: ProfileId = ProfileId(1);

fn game(event: &str, white: &str, black: &str, date: &str, moves: &str) -> String {
    format!(
        "[Event \"{event}\"]\n[White \"{white}\"]\n[Black \"{black}\"]\n[Date \"{date}\"]\n[Result \"1-0\"]\n\n{moves} 1-0\n\n"
    )
}

fn opts(source: ImportSource, file: &str) -> ImportOptions {
    ImportOptions::new(
        Some(file),
        Provenance::from_source(source, ImportMode::File, 1_000),
    )
}

fn no_progress() -> impl FnMut(&ImportProgress) {
    |_| {}
}

// ---- Migração 13 ------------------------------------------------------------------

#[test]
fn migration_13_lifts_a_v12_database_and_keeps_everything() {
    let db = TempDb::new("mig13");
    let tree = {
        let g = &parse_pgn(
            "[Event \"Copa\"]\n[White \"Ana\"]\n[Black \"Beto\"]\n[Date \"2020.01.02\"]\n\
             [Site \"Rio\"]\n[WhiteElo \"2100\"]\n[ECO \"C20\"]\n[TimeControl \"300+3\"]\n\n\
             1. e4 e5 2. Nf3 {bom} 1-0",
        )
        .unwrap()[0];
        serde_json::to_string(g).unwrap()
    };
    {
        let mut conn = rusqlite::Connection::open(db.path()).unwrap();
        migrations::migrate_to(&mut conn, 12).unwrap();
        conn.execute(
            "INSERT INTO imported_games (profile_id, source, event, white, black, date, result,
                 start_fen, ply_count, variation_count, tree_version, tree_json, imported_at_ms)
             VALUES (1, 'antigo.pgn', 'Copa', 'Ana', 'Beto', '2020.01.02', '1-0', 'x', 3, 0, 1, ?1, 777)",
            [&tree],
        )
        .unwrap();
    }
    let store = Store::open(db.path()).unwrap();
    assert_eq!(store.schema_version().unwrap(), LATEST_SCHEMA_VERSION);
    let list = store.list_imported(&ImportedQuery::default()).unwrap();
    assert_eq!(list.len(), 1);
    let row = &list[0];
    // O texto livre antigo continua intacto e vira a Origem de arquivo.
    assert_eq!(row.source.as_deref(), Some("antigo.pgn"));
    assert_eq!(row.imported_at_ms, 777);
    assert_eq!(row.import_mode, ImportMode::File);
    assert_eq!(row.provider, None);
    // As colunas novas são preenchidas a partir das tags da árvore.
    assert_eq!(row.site.as_deref(), Some("Rio"));
    assert_eq!(row.white_elo, Some(2100));
    assert_eq!(row.eco.as_deref(), Some("C20"));
    assert_eq!(row.time_control.as_deref(), Some("300+3"));
    assert_eq!(row.pace, Some(Pace::Blitz));
    let origins = store.origins(ME, row.id).unwrap();
    assert_eq!(origins.len(), 1);
    assert_eq!(origins[0].source.as_deref(), Some("antigo.pgn"));
    assert_eq!(origins[0].retrieved_at_ms, 777);
    // A árvore continua carregando e a reimportação reconhece a Partida antiga.
    assert_eq!(store.load_imported(ME, row.id).unwrap().moves.len(), 3);
    let mut store = store;
    let again = store
        .import_pgn_with(
            ME,
            &game("Copa", "Ana", "Beto", "2020.01.02", "1. e4 e5 2. Nf3"),
            &opts(ImportSource::Other, "novo.pgn"),
        )
        .unwrap();
    assert_eq!((again.imported.len(), again.duplicates), (0, 1));
}

// ---- Duplicatas -------------------------------------------------------------------

#[test]
fn reimporting_the_same_pgn_adds_nothing() {
    let mut store = Store::open_in_memory().unwrap();
    let text = [
        game("A", "Ana", "Beto", "2021.01.01", "1. e4 e5"),
        game("B", "Carla", "Davi", "2021.01.02", "1. d4 d5"),
        game("C", "Eva", "Fábio", "2021.01.03", "1. c4 c5"),
    ]
    .concat();
    let o = opts(ImportSource::Twic, "twic.pgn");
    let first = store.import_pgn_with(ME, &text, &o).unwrap();
    assert_eq!((first.imported.len(), first.duplicates), (3, 0));
    let second = store.import_pgn_with(ME, &text, &o).unwrap();
    assert_eq!((second.imported.len(), second.duplicates), (0, 3));
    assert_eq!(
        store
            .list_imported(&ImportedQuery::default())
            .unwrap()
            .len(),
        3
    );
    // Reimportar o mesmo arquivo não acrescenta Origem repetida.
    let id = store.list_imported(&ImportedQuery::default()).unwrap()[0].id;
    assert_eq!(store.origins(ME, id).unwrap().len(), 1);
}

#[test]
fn the_same_game_from_two_providers_is_one_game_with_two_origins() {
    let mut store = Store::open_in_memory().unwrap();
    let text = game("Blitz", "Ana", "Beto", "2022.03.04", "1. e4 e5 2. Nf3");
    store
        .import_pgn_with(ME, &text, &opts(ImportSource::Lichess, "li.pgn"))
        .unwrap();
    let second = store
        .import_pgn_with(ME, &text, &opts(ImportSource::ChessCom, "cc.pgn"))
        .unwrap();
    assert_eq!((second.imported.len(), second.duplicates), (0, 1));
    let list = store.list_imported(&ImportedQuery::default()).unwrap();
    assert_eq!(list.len(), 1);
    let origins = store.origins(ME, list[0].id).unwrap();
    let providers: Vec<_> = origins.iter().map(|o| o.provider.as_deref()).collect();
    assert_eq!(providers, [Some("lichess"), Some("chess-com")]);
    // A busca por Provedor acha a Partida por qualquer das duas Origens.
    for provider in ["lichess", "chess-com"] {
        let found = store
            .list_imported(&ImportedQuery {
                provider: Some(provider.into()),
                ..ImportedQuery::default()
            })
            .unwrap();
        assert_eq!(found.len(), 1, "{provider}");
    }
}

#[test]
fn same_names_but_different_games_are_not_merged() {
    let mut store = Store::open_in_memory().unwrap();
    let text = [
        game("R1", "Silva", "Souza", "2020.05.01", "1. e4 e5"),
        game("R2", "Silva", "Souza", "2020.05.02", "1. e4 e5"),
        game("R3", "Silva", "Souza", "2020.05.01", "1. d4 d5"),
        // Cores trocadas: outra Partida.
        game("R4", "Souza", "Silva", "2020.05.01", "1. e4 e5"),
    ]
    .concat();
    let report = store
        .import_pgn_with(ME, &text, &opts(ImportSource::Other, "x.pgn"))
        .unwrap();
    assert_eq!((report.imported.len(), report.duplicates), (4, 0));
}

#[test]
fn fingerprint_ignores_case_accents_and_name_order_but_not_colors() {
    let fp = |w: &str, b: &str| {
        let g = &parse_pgn(&game("E", w, b, "2020.01.01", "1. e4 e5")).unwrap()[0];
        game_fingerprint(g)
    };
    assert_eq!(
        fp("Magnus Carlsen", "Hikaru Nakamura"),
        fp("CARLSEN, Magnus", "nakamura,  hikaru")
    );
    assert_eq!(fp("José Silva", "Ana"), fp("jose silva", "ANA"));
    assert_ne!(fp("Ana", "Beto"), fp("Beto", "Ana"));
}

#[test]
fn remote_id_dedupes_when_the_provider_gives_one() {
    let mut store = Store::open_in_memory().unwrap();
    let a = "[Event \"Rated Blitz game\"]\n[Site \"https://lichess.org/AbCdEf12\"]\n\
             [White \"x\"]\n[Black \"y\"]\n[Date \"2023.01.01\"]\n\n1. e4 e5 *\n\n";
    // Mesma Partida do Lichess, baixada de novo com a data corrigida e outro texto.
    let b = "[Event \"Rated Blitz game\"]\n[Site \"https://lichess.org/AbCdEf12\"]\n\
             [White \"x\"]\n[Black \"y\"]\n[Date \"2023.01.02\"]\n\n1. e4 e5 *\n\n";
    let o = opts(ImportSource::Lichess, "li.pgn");
    assert_eq!(store.import_pgn_with(ME, a, &o).unwrap().imported.len(), 1);
    let second = store.import_pgn_with(ME, b, &o).unwrap();
    assert_eq!((second.imported.len(), second.duplicates), (0, 1));
    let list = store.list_imported(&ImportedQuery::default()).unwrap();
    assert_eq!(list[0].pace, Some(Pace::Blitz));
}

// ---- Fluxo, lotes, cancelamento ---------------------------------------------------

/// Gera N partidas distintas sob demanda, sem existir como arquivo.
struct Synthetic {
    left: usize,
    next: usize,
    buf: Vec<u8>,
    pos: usize,
}

impl Synthetic {
    fn new(n: usize) -> Self {
        Self {
            left: n,
            next: 0,
            buf: Vec::new(),
            pos: 0,
        }
    }
}

impl Read for Synthetic {
    fn read(&mut self, out: &mut [u8]) -> std::io::Result<usize> {
        if self.pos >= self.buf.len() {
            if self.left == 0 {
                return Ok(0);
            }
            self.left -= 1;
            self.next += 1;
            let n = self.next;
            self.buf = format!(
                "[Event \"G{n}\"]\n[White \"W{n}\"]\n[Black \"B{n}\"]\n[Date \"2020.01.01\"]\n\
                 [ECO \"B20\"]\n\n1. e4 c5 2. Nf3 d6 1-0\n\n"
            )
            .into_bytes();
            self.pos = 0;
        }
        let n = out.len().min(self.buf.len() - self.pos);
        out[..n].copy_from_slice(&self.buf[self.pos..self.pos + n]);
        self.pos += n;
        Ok(n)
    }
}

#[test]
fn many_games_import_in_batches_with_progress() {
    let mut store = Store::open_in_memory().unwrap();
    let mut o = opts(ImportSource::PgnMentor, "grande.pgn");
    o.batch_size = 500;
    let mut ticks = Vec::new();
    let summary = store
        .import_pgn_stream(
            ME,
            BufReader::new(Synthetic::new(5_000)),
            &o,
            &AtomicBool::new(false),
            &mut |p: &ImportProgress| ticks.push(p.clone()),
        )
        .unwrap();
    assert_eq!(
        (summary.new, summary.duplicates, summary.errors),
        (5_000, 0, 0)
    );
    assert!(!summary.cancelled);
    // Um relato por lote: leitura em lotes, nunca o arquivo inteiro de uma vez.
    assert!(ticks.len() >= 10, "{} relatos", ticks.len());
    assert!(ticks.windows(2).all(|w| w[0].games_seen <= w[1].games_seen));
    assert!(ticks.iter().all(|t| t.new % 500 == 0 || t.finished));
    assert!(summary.peak_buffered_bytes < 4096);
}

#[test]
fn a_hundred_thousand_games_import_in_bounded_memory() {
    // 100 mil Partidas geradas sob demanda: o teste prova a leitura em fluxo (o maior
    // trecho em memória é uma Partida) e em lotes (nenhum lote passa de `batch_size`).
    let mut store = Store::open_in_memory().unwrap();
    let mut o = opts(ImportSource::PgnMentor, "cem-mil.pgn");
    o.batch_size = 2_000;
    let mut biggest_batch = 0usize;
    let mut last_new = 0usize;
    let summary = store
        .import_pgn_stream(
            ME,
            BufReader::new(Synthetic::new(100_000)),
            &o,
            &AtomicBool::new(false),
            &mut |p: &ImportProgress| {
                biggest_batch = biggest_batch.max(p.new - last_new);
                last_new = p.new;
            },
        )
        .unwrap();
    assert_eq!(summary.new, 100_000);
    assert!(biggest_batch <= 2_000, "lote de {biggest_batch}");
    assert!(summary.peak_buffered_bytes < 4096);
}

#[test]
fn a_big_import_can_be_cancelled_and_keeps_what_was_committed() {
    let mut store = Store::open_in_memory().unwrap();
    let mut o = opts(ImportSource::Other, "x.pgn");
    o.batch_size = 100;
    let cancel = AtomicBool::new(false);
    let summary = store
        .import_pgn_stream(
            ME,
            BufReader::new(Synthetic::new(50_000)),
            &o,
            &cancel,
            &mut |p: &ImportProgress| {
                if p.new >= 300 {
                    cancel.store(true, Ordering::Relaxed);
                }
            },
        )
        .unwrap();
    assert!(summary.cancelled);
    assert!(summary.new >= 300 && summary.new < 1_000, "{}", summary.new);
    let total = store
        .list_imported(&ImportedQuery::default())
        .unwrap()
        .len();
    assert_eq!(total, summary.new);
}

// ---- Encodings e compressão -------------------------------------------------------

#[test]
fn latin1_file_imports_with_accents_and_a_warning() {
    let db = TempDb::new("latin1");
    let path = db.dir().join("antigo.pgn");
    let mut bytes = b"[Event \"Copa\"]\n[White \"Jos".to_vec();
    bytes.push(0xE9);
    bytes.extend_from_slice(b" Ant");
    bytes.push(0xF4);
    bytes.extend_from_slice(b"nio\"]\n[Black \"Beto\"]\n\n1. e4 e5 *\n");
    std::fs::write(&path, bytes).unwrap();
    let mut store = Store::open_in_memory().unwrap();
    let summary = store
        .import_pgn_file(
            ME,
            &path,
            &opts(ImportSource::ChessBase, "antigo.pgn"),
            &AtomicBool::new(false),
            &mut no_progress(),
        )
        .unwrap();
    assert_eq!(summary.new, 1);
    assert_eq!(summary.converted_games, 1);
    let row = &store.list_imported(&ImportedQuery::default()).unwrap()[0];
    assert_eq!(row.white.as_deref(), Some("José Antônio"));
}

#[test]
fn gzip_pgn_imports() {
    let db = TempDb::new("gz");
    let path = db.dir().join("lote.pgn.gz");
    let text = [
        game("A", "Ana", "Beto", "2021.01.01", "1. e4 e5"),
        game("B", "Carla", "Davi", "2021.01.02", "1. d4 d5"),
    ]
    .concat();
    let file = std::fs::File::create(&path).unwrap();
    let mut gz = flate2::write::GzEncoder::new(file, flate2::Compression::default());
    gz.write_all(text.as_bytes()).unwrap();
    gz.finish().unwrap();
    let mut store = Store::open_in_memory().unwrap();
    let summary = store
        .import_pgn_file(
            ME,
            &path,
            &opts(ImportSource::Lichess, "lote.pgn.gz"),
            &AtomicBool::new(false),
            &mut no_progress(),
        )
        .unwrap();
    assert_eq!((summary.new, summary.duplicates, summary.errors), (2, 0, 0));
    // O mesmo arquivo, agora sem compressão, só repete.
    let plain = db.dir().join("lote.pgn");
    std::fs::write(&plain, &text).unwrap();
    let again = store
        .import_pgn_file(
            ME,
            &plain,
            &opts(ImportSource::Lichess, "lote.pgn"),
            &AtomicBool::new(false),
            &mut no_progress(),
        )
        .unwrap();
    assert_eq!((again.new, again.duplicates), (0, 2));
}

#[test]
fn hostile_pgn_is_counted_as_errors_and_the_good_games_survive() {
    let mut bytes = Vec::new();
    bytes.extend_from_slice(
        b"[Event \"x\"]\n[Event \"y\"]\n[White \"Ana\"]\n[Black \"B\"]\n\n1. e4 e5 *\n\n",
    );
    bytes.extend_from_slice(b"[Event \"gigante\"]\n\n1. e4 {");
    bytes.extend(std::iter::repeat_n(b'a', 2 * 1024 * 1024));
    bytes.extend_from_slice(b"} *\n\n[Event \"lixo\"]\n\n1. e4 (( e5 \0\0 )))\n\n");
    bytes.extend_from_slice(b"[Event \"ok\"]\n[White \"C\"]\n[Black \"D\"]\n\n1. d4 d5 *\n");
    let mut store = Store::open_in_memory().unwrap();
    let summary = store
        .import_pgn_stream(
            ME,
            BufReader::new(Cursor::new(bytes)),
            &opts(ImportSource::Other, "ruim.pgn"),
            &AtomicBool::new(false),
            &mut no_progress(),
        )
        .unwrap();
    assert_eq!(summary.new, 2);
    assert!(summary.errors >= 2);
    assert!(!summary.error_samples.is_empty());
}

// ---- Filtros e exportação ---------------------------------------------------------

#[test]
fn filters_by_provider_eco_and_pace_in_the_list_and_in_the_library() {
    let mut store = Store::open_in_memory().unwrap();
    let li = "[Event \"Rated Blitz game\"]\n[White \"a\"]\n[Black \"b\"]\n[ECO \"B20\"]\n\
              [TimeControl \"180+0\"]\n\n1. e4 c5 *\n\n";
    let tw = "[Event \"Torneio\"]\n[White \"c\"]\n[Black \"d\"]\n[ECO \"C42\"]\n\
              [TimeControl \"5400+30\"]\n\n1. e4 e5 2. Nf3 Nf6 *\n\n";
    store
        .import_pgn_with(ME, li, &opts(ImportSource::Lichess, "li.pgn"))
        .unwrap();
    store
        .import_pgn_with(ME, tw, &opts(ImportSource::Twic, "tw.pgn"))
        .unwrap();

    let list = |q: ImportedQuery| store.list_imported(&q).unwrap().len();
    let by_provider = |p: &str| ImportedQuery {
        provider: Some(p.into()),
        ..ImportedQuery::default()
    };
    assert_eq!(list(by_provider("twic")), 1);
    assert_eq!(list(by_provider("lichess")), 1);
    assert_eq!(list(by_provider("chessbase")), 0);
    assert_eq!(
        list(ImportedQuery {
            eco: Some("c4".into()),
            ..ImportedQuery::default()
        }),
        1
    );
    assert_eq!(
        list(ImportedQuery {
            pace: Some(Pace::Classical),
            ..ImportedQuery::default()
        }),
        1
    );

    let lib = |q: LibraryQuery| store.search_library(&q).unwrap().total;
    assert_eq!(
        lib(LibraryQuery {
            provider: Some("twic".into()),
            ..LibraryQuery::default()
        }),
        1
    );
    assert_eq!(
        lib(LibraryQuery {
            eco: Some("B2".into()),
            ..LibraryQuery::default()
        }),
        1
    );
    assert_eq!(
        lib(LibraryQuery {
            pace: Some(Pace::Bullet),
            ..LibraryQuery::default()
        }),
        0
    );
    assert_eq!(
        lib(LibraryQuery {
            pace: Some(Pace::Blitz),
            ..LibraryQuery::default()
        }),
        1
    );
}

#[test]
fn exporting_keeps_the_original_tags() {
    let mut store = Store::open_in_memory().unwrap();
    let text = "[Event \"Copa\"]\n[Site \"Rio\"]\n[White \"Ana\"]\n[Black \"Beto\"]\n\
                [WhiteElo \"2100\"]\n[BlackElo \"2050\"]\n[ECO \"C20\"]\n[Annotator \"Fulano\"]\n\
                [TimeControl \"600+5\"]\n\n1. e4 e5 *\n";
    let id = store
        .import_pgn_with(ME, text, &opts(ImportSource::Twic, "t.pgn"))
        .unwrap()
        .imported[0];
    let pgn = store.load_imported(ME, id).unwrap().to_pgn();
    for tag in [
        "[Site \"Rio\"]",
        "[WhiteElo \"2100\"]",
        "[BlackElo \"2050\"]",
        "[ECO \"C20\"]",
        "[Annotator \"Fulano\"]",
        "[TimeControl \"600+5\"]",
    ] {
        assert!(pgn.contains(tag), "falta {tag} em:\n{pgn}");
    }
}

#[test]
fn provenance_carries_provider_mode_instant_license() {
    let mut store = Store::open_in_memory().unwrap();
    let mut p = Provenance::from_source(ImportSource::Twic, ImportMode::Manual, 42);
    p.reference_url = Some("https://theweekinchess.com/twic".into());
    let id = store
        .import_pgn_with(
            ME,
            &game("A", "Ana", "Beto", "2021.01.01", "1. e4 e5"),
            &ImportOptions::new(Some("twic1500.pgn"), p),
        )
        .unwrap()
        .imported[0];
    let o = &store.origins(ME, id).unwrap()[0];
    assert_eq!(o.provider.as_deref(), Some("twic"));
    assert_eq!(o.mode, ImportMode::Manual);
    assert_eq!(o.retrieved_at_ms, 42);
    assert_eq!(o.license.as_deref(), Some("twic-personal"));
    assert_eq!(
        o.reference_url.as_deref(),
        Some("https://theweekinchess.com/twic")
    );
    assert_eq!(o.source.as_deref(), Some("twic1500.pgn"));
}

#[test]
fn a_gzip_bomb_stops_at_the_decompressed_limit() {
    let db = TempDb::new("bomb");
    let path = db.dir().join("bomba.pgn.gz");
    let file = std::fs::File::create(&path).unwrap();
    let mut gz = flate2::write::GzEncoder::new(file, flate2::Compression::best());
    gz.write_all(b"[Event \"x\"]\n\n1. e4 e5 *\n\n").unwrap();
    let zeros = vec![b' '; 1 << 20];
    for _ in 0..8 {
        gz.write_all(&zeros).unwrap();
    }
    gz.finish().unwrap();
    let mut store = Store::open_in_memory().unwrap();
    let mut o = opts(ImportSource::Other, "bomba.pgn.gz");
    o.max_decompressed_bytes = 1 << 20;
    let err = store
        .import_pgn_file(ME, &path, &o, &AtomicBool::new(false), &mut no_progress())
        .unwrap_err();
    assert!(
        matches!(err, chess_store::StoreError::DecompressedTooLarge { limit } if limit == 1 << 20)
    );
}
