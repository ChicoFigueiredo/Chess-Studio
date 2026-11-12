//! Partidas públicas (Lichess e Chess.com) em fluxo, de ponta a ponta até o `chess-store`
//! em memória, com Origem e sem duplicar numa segunda execução.

mod common;

use chess_net::ProviderId;
use chess_net::testing::FakeResponse;
use chess_providers::{
    Color, Day, GamesEvent, GamesFeed, GamesFilter, Pace, ProviderError, YearMonth,
};
use chess_store::{ImportMode, ImportedQuery, ProfileId, Store};
use common::*;
use std::io::BufReader;
use std::sync::Arc;
use std::sync::atomic::AtomicBool;
use std::time::Duration;

const ME: ProfileId = ProfileId(1);

/// Lê o fluxo até o fim (ou a falha); devolve os trechos e o último evento.
fn drain(feed: &mut GamesFeed) -> (Vec<String>, GamesEvent) {
    let mut chunks = Vec::new();
    let mut last = None;
    assert!(
        eventually(Duration::from_secs(20), || {
            while let Some(event) = feed.try_next() {
                match event {
                    GamesEvent::Chunk(pgn) => chunks.push(pgn),
                    other => {
                        last = Some(other);
                        return true;
                    }
                }
            }
            false
        }),
        "o fluxo não terminou"
    );
    (chunks, last.expect("evento final"))
}

fn count_events(text: &str) -> usize {
    text.lines().filter(|l| l.starts_with("[Event ")).count()
}

fn import(store: &mut Store, feed: GamesFeed) -> chess_store::ImportSummary {
    let options = feed.import_options();
    let cancel = Arc::new(AtomicBool::new(false));
    let reader = BufReader::new(feed.into_reader(cancel.clone()));
    store
        .import_pgn_stream(ME, reader, &options, &cancel, &mut |_| {})
        .expect("importação")
}

// ---- Lichess -----------------------------------------------------------------------

#[test]
fn lichess_games_stream_pgn_with_progress_and_filters_in_the_request() {
    let env = env_enabled();
    env.server.route(
        "/api/games/user/thibault",
        fixture("lichess-games-ndjson.http"),
    );
    let filter = GamesFilter {
        since: Some(Day::new(2026, 10, 1).expect("data")),
        until: Some(Day::new(2026, 11, 7).expect("data")),
        paces: vec![Pace::Blitz],
        color: Some(Color::White),
        max: Some(2),
    };

    let mut feed = env.lichess.games("thibault", &filter);
    let (chunks, last) = drain(&mut feed);

    assert_eq!(last, GamesEvent::Finished);
    assert_eq!(chunks.len(), 2);
    assert!(chunks[0].starts_with("[Event \"rated blitz game\"]"));
    assert!(chunks[0].contains("1. e4 e5 2. f4"));
    assert_eq!(feed.progress().games, 2);
    assert_eq!(feed.provenance().provider, ProviderId::Lichess);
    assert_eq!(feed.provenance().queried_at, T0);

    let sent = &env.server.requests()[0];
    assert_eq!(sent.header("accept"), Some("application/x-ndjson"));
    assert_eq!(sent.query_value("pgnInJson").as_deref(), Some("true"));
    assert_eq!(sent.query_value("perfType").as_deref(), Some("blitz"));
    assert_eq!(sent.query_value("color").as_deref(), Some("white"));
    assert_eq!(sent.query_value("max").as_deref(), Some("2"));
    assert_eq!(
        sent.query_value("since").as_deref(),
        Some(
            Day::new(2026, 10, 1)
                .expect("data")
                .start_ms()
                .to_string()
                .as_str()
        )
    );
    assert_eq!(
        sent.query_value("until").as_deref(),
        Some(
            Day::new(2026, 11, 7)
                .expect("data")
                .end_ms()
                .to_string()
                .as_str()
        )
    );
}

#[test]
fn lichess_games_skip_refused_variants() {
    let env = env_enabled();
    let body = concat!(
        "{\"id\":\"aaaaaaaa\",\"variant\":\"standard\",\"pgn\":\"[Event \\\"x\\\"]\\n\\n1. e4 *\\n\\n\"}\n",
        "{\"id\":\"bbbbbbbb\",\"variant\":\"chess960\",\"pgn\":\"[Event \\\"y\\\"]\\n\\n1. e4 *\\n\\n\"}\n",
        "\n",
    );
    env.server.route(
        "/api/games/user/alguem",
        FakeResponse::ok(body).header("content-type", "application/x-ndjson"),
    );

    let mut feed = env.lichess.games("alguem", &GamesFilter::default());
    let (chunks, last) = drain(&mut feed);

    assert_eq!(last, GamesEvent::Finished);
    assert_eq!(chunks.len(), 1);
    assert_eq!(feed.progress().skipped, 1);
}

#[test]
fn lichess_games_failures_are_typed_and_import_is_not_blocked_by_fair_play() {
    let env = env_enabled();
    env.server
        .route("/api/games/user/fantasma", FakeResponse::status(404));
    let (_, last) = drain(&mut env.lichess.games("fantasma", &GamesFilter::default()));
    assert_eq!(
        last,
        GamesEvent::Failed(ProviderError::UnknownName {
            provider: ProviderId::Lichess,
            name: "fantasma".to_owned()
        })
    );

    env.server
        .route("/api/games/user/lento", FakeResponse::status(429));
    let (_, last) = drain(&mut env.lichess.games("lento", &GamesFilter::default()));
    assert!(matches!(
        last,
        GamesEvent::Failed(ProviderError::RateLimited { .. })
    ));

    let env2 = env_enabled();
    env2.server.route(
        "/api/games/user/torto",
        FakeResponse::stream(&[b"nao e json\n"]),
    );
    let (_, last) = drain(&mut env2.lichess.games("torto", &GamesFilter::default()));
    assert!(matches!(
        last,
        GamesEvent::Failed(ProviderError::Malformed { .. })
    ));

    let (_, last) = drain(&mut env2.lichess.games("../x", &GamesFilter::default()));
    assert!(matches!(
        last,
        GamesEvent::Failed(ProviderError::InvalidInput { .. })
    ));

    // Desligado: nada sai.
    let off = common::env();
    let (_, last) = drain(&mut off.lichess.games("thibault", &GamesFilter::default()));
    assert_eq!(
        last,
        GamesEvent::Failed(ProviderError::Disabled {
            provider: ProviderId::Lichess
        })
    );
    assert!(off.server.requests().is_empty());

    // Trava de fair play ligada: a importação de Partidas públicas segue permitida.
    let locked = env_enabled();
    locked.server.route(
        "/api/games/user/thibault",
        fixture("lichess-games-ndjson.http"),
    );
    locked.net.fair_play_guard().engage("Partida online");
    let (chunks, last) = drain(&mut locked.lichess.games("thibault", &GamesFilter::default()));
    assert_eq!(last, GamesEvent::Finished);
    assert_eq!(chunks.len(), 2);
}

#[test]
fn lichess_import_goes_end_to_end_with_origin_and_does_not_duplicate() {
    let env = env_enabled();
    env.server.route(
        "/api/games/user/thibault",
        fixture("lichess-games-ndjson.http"),
    );
    let mut store = Store::open_in_memory().expect("banco");

    let first = import(
        &mut store,
        env.lichess.games("thibault", &GamesFilter::default()),
    );
    assert_eq!((first.new, first.duplicates, first.errors), (2, 0, 0));

    let list = store
        .list_imported(&ImportedQuery::default())
        .expect("lista");
    assert_eq!(list.len(), 2);
    let origins = store.origins(ME, list[0].id).expect("origens");
    let origin = &origins[0];
    assert_eq!(origin.provider.as_deref(), Some("lichess"));
    assert_eq!(origin.mode, ImportMode::Api);
    assert_eq!(origin.retrieved_at_ms, T0.as_millis());
    assert_eq!(origin.license.as_deref(), Some("lichess-cc0"));
    assert_eq!(
        origin.reference_url.as_deref(),
        Some("https://lichess.org/api/games/user/thibault")
    );
    let mut ids: Vec<String> = list
        .iter()
        .flat_map(|g| store.origins(ME, g.id).expect("origens"))
        .filter_map(|o| o.remote_id)
        .collect();
    ids.sort();
    assert_eq!(ids, vec!["0IdR4QNI".to_owned(), "h7BRAh6C".to_owned()]);

    // Segunda execução: tudo repetido, nada novo.
    let second = import(
        &mut store,
        env.lichess.games("thibault", &GamesFilter::default()),
    );
    assert_eq!((second.new, second.duplicates), (0, 2));
    assert_eq!(
        store
            .list_imported(&ImportedQuery::default())
            .expect("lista")
            .len(),
        2
    );
}

#[test]
fn cancelling_the_reader_stops_the_stream_and_frees_the_connection() {
    let env = env_enabled();
    env.server.route(
        "/api/games/user/thibault",
        FakeResponse::stream_forever(b"\n"),
    );
    let feed = env.lichess.games("thibault", &GamesFilter::default());
    let cancel = Arc::new(AtomicBool::new(false));
    let mut reader = feed.into_reader(cancel.clone());
    assert!(env.server.wait_for_requests(1, Duration::from_secs(30)));

    cancel.store(true, std::sync::atomic::Ordering::Release);
    let mut buf = [0u8; 16];
    assert_eq!(std::io::Read::read(&mut reader, &mut buf).expect("fim"), 0);
    drop(reader);

    assert!(
        eventually(Duration::from_secs(30), || env.net.status().active_requests
            == 0),
        "o fluxo ficou preso"
    );
}

// ---- Chess.com ---------------------------------------------------------------------

fn month_fixture_total() -> usize {
    let text = std::fs::read_to_string(
        std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("tests/fixtures/chesscom-month-pgn.http"),
    )
    .expect("fixture");
    count_events(&text)
}

fn only_july_2007() -> GamesFilter {
    GamesFilter {
        since: Some(Day::new(2007, 7, 1).expect("data")),
        until: Some(Day::new(2007, 7, 31).expect("data")),
        max: None,
        ..GamesFilter::default()
    }
}

fn chesscom_env() -> Env {
    let env = env_enabled();
    env.server.route(
        "/pub/player/erik/games/archives",
        fixture("chesscom-archives.http"),
    );
    env.server.route(
        "/pub/player/erik/games/2007/07/pgn",
        fixture("chesscom-month-pgn.http"),
    );
    env
}

#[test]
fn chesscom_profile_and_archives_parse_the_real_answers() {
    let env = chesscom_env();
    env.server
        .route("/pub/player/erik", fixture("chesscom-profile.http"));

    let profile = env.chesscom.profile("Erik").wait().expect("perfil");
    assert_eq!(profile.value.username, "erik");
    assert_eq!(profile.value.country.as_deref(), Some("US"));
    assert_eq!(profile.value.joined_ms, Some(1_178_556_600_000));
    assert_eq!(profile.provenance.provider, ProviderId::ChessCom);
    assert!(profile.provenance.license.contains("Chess.com"));

    let archives = env.chesscom.archives("erik").wait().expect("arquivos");
    assert_eq!(
        archives.value.first(),
        Some(&YearMonth::new(2007, 7).expect("mês"))
    );
    assert!(archives.value.len() > 100);
    assert!(archives.value.windows(2).all(|w| w[0] < w[1]));
}

#[test]
fn a_hostile_archive_list_is_deduplicated_and_capped_to_the_newest_months() {
    // SEC-07: a lista não tinha teto e aceitava meses repetidos (um pedido por item).
    let env = chesscom_env();
    let mut urls: Vec<String> = Vec::new();
    for year in 1990..2090 {
        for month in 1..=12 {
            let url = format!("https://api.chess.com/pub/player/erik/games/{year}/{month:02}");
            urls.push(url.clone());
            urls.push(url);
        }
    }
    env.server.route(
        "/pub/player/erik/games/archives",
        FakeResponse::json(&serde_json::json!({ "archives": urls }).to_string()),
    );

    let months = env
        .chesscom
        .archives("erik")
        .wait()
        .expect("arquivos")
        .value;

    assert_eq!(months.len(), 600);
    assert!(months.windows(2).all(|w| w[0] < w[1]), "sem repetição");
    assert_eq!(months.last(), Some(&YearMonth::new(2089, 12).expect("mês")));
}

#[test]
fn chesscom_errors_are_typed() {
    let env = env_enabled();
    env.server
        .route("/pub/player/fantasma", FakeResponse::status(404));
    assert_eq!(
        env.chesscom.profile("fantasma").wait().expect_err("404"),
        ProviderError::UnknownName {
            provider: ProviderId::ChessCom,
            name: "fantasma".to_owned()
        }
    );
    env.server.route(
        "/pub/player/torto/games/archives",
        FakeResponse::json("{\"x\":1}"),
    );
    assert!(matches!(
        env.chesscom.archives("torto").wait(),
        Err(ProviderError::Malformed { .. })
    ));
    env.server.route(
        "/pub/player/erik/games/2001/01/pgn",
        FakeResponse::status(404),
    );
    assert_eq!(
        env.chesscom
            .month_games(
                "erik",
                YearMonth::new(2001, 1).expect("mês"),
                &GamesFilter::default()
            )
            .wait()
            .expect_err("mês vazio"),
        ProviderError::NoData
    );
    // 429 deixa o Provedor "limitado até": por último, ou barraria os pedidos acima.
    env.server
        .route("/pub/player/lento", FakeResponse::status(429));
    assert!(matches!(
        env.chesscom.profile("lento").wait(),
        Err(ProviderError::RateLimited { .. })
    ));
    let off = common::env();
    assert!(matches!(
        off.chesscom.profile("erik").wait(),
        Err(ProviderError::Disabled { .. })
    ));
    assert!(off.server.requests().is_empty());
}

#[test]
fn chesscom_revalidates_with_etag_after_the_cache_expires() {
    let env = env_enabled();
    env.server.route_seq(
        "/pub/player/erik",
        vec![fixture("chesscom-profile.http"), FakeResponse::status(304)],
    );
    let first = env.chesscom.profile("erik").wait().expect("1");

    env.clock.advance(Duration::from_secs(11 * 60));
    let second = env.chesscom.profile("erik").wait().expect("2");

    let requests = env.server.requests();
    assert_eq!(requests.len(), 2);
    assert!(
        requests[1].header("if-none-match").is_some(),
        "mandou o ETag"
    );
    assert_eq!(second.freshness, chess_net::Freshness::Revalidated);
    assert_eq!(second.value, first.value);
}

#[test]
fn chesscom_month_games_apply_period_pace_and_color_locally() {
    let env = chesscom_env();
    let total = month_fixture_total();
    assert!(total > 10, "a fixture real tem {total} Partidas");

    let all = env
        .chesscom
        .month_games(
            "erik",
            YearMonth::new(2007, 7).expect("mês"),
            &only_july_2007(),
        )
        .wait()
        .expect("mês")
        .value;
    assert_eq!(all.games.len() + all.skipped, total);
    assert!(all.games.len() > 10);

    // Só de Pretas: o filtro olha o jogador.
    let black = env
        .chesscom
        .month_games(
            "erik",
            YearMonth::new(2007, 7).expect("mês"),
            &GamesFilter {
                color: Some(Color::Black),
                ..only_july_2007()
            },
        )
        .wait()
        .expect("mês")
        .value;
    assert!(black.games.len() < all.games.len() || black.skipped == 0);
    assert!(black.games.iter().all(|g| g.contains("[Black \"erik\"]")));

    // Todas são correspondência (1/259200): bullet não sobra nada.
    let bullet = env
        .chesscom
        .month_games(
            "erik",
            YearMonth::new(2007, 7).expect("mês"),
            &GamesFilter {
                paces: vec![Pace::Bullet],
                ..only_july_2007()
            },
        )
        .wait()
        .expect("mês")
        .value;
    assert!(bullet.games.is_empty());
    assert_eq!(bullet.skipped, total);

    // O período corta por dia.
    let late = env
        .chesscom
        .month_games(
            "erik",
            YearMonth::new(2007, 7).expect("mês"),
            &GamesFilter {
                since: Some(Day::new(2007, 7, 31).expect("data")),
                ..only_july_2007()
            },
        )
        .wait()
        .expect("mês")
        .value;
    assert!(late.games.len() < all.games.len());
}

#[test]
fn chesscom_games_run_months_in_series_newest_first_and_respect_max() {
    let env = env_enabled();
    env.server.route(
        "/pub/player/erik/games/archives",
        FakeResponse::json(
            r#"{"archives":["https://api.chess.com/pub/player/erik/games/2007/07","https://api.chess.com/pub/player/erik/games/2007/08"]}"#,
        ),
    );
    let game = |n: u32, date: &str| {
        format!(
            "[Event \"Live\"]\n[Site \"Chess.com\"]\n[Date \"{date}\"]\n[White \"erik\"]\n[Black \"b{n}\"]\n[Result \"1-0\"]\n[TimeControl \"600\"]\n[Link \"https://www.chess.com/game/live/{}\"]\n\n1. e4 e5 1-0\n\n",
            9000 + n
        )
    };
    env.server.route(
        "/pub/player/erik/games/2007/07/pgn",
        FakeResponse::ok(format!(
            "{}{}",
            game(1, "2007.07.10"),
            game(2, "2007.07.20")
        )),
    );
    env.server.route(
        "/pub/player/erik/games/2007/08/pgn",
        FakeResponse::ok(format!(
            "{}{}",
            game(3, "2007.08.10"),
            game(4, "2007.08.20")
        )),
    );

    let mut feed = env.chesscom.games(
        "erik",
        &GamesFilter {
            max: Some(3),
            ..GamesFilter::default()
        },
    );
    let (chunks, last) = drain(&mut feed);

    assert_eq!(last, GamesEvent::Finished);
    let events: usize = chunks.iter().map(|c| count_events(c)).sum();
    assert_eq!(events, 3, "o máximo vale para o conjunto");
    // Agosto primeiro (mais recente), depois as últimas de julho.
    assert!(chunks[0].contains("b3") && chunks[0].contains("b4"));
    assert!(chunks[1].contains("b2") && !chunks[1].contains("b1"));
    let paths: Vec<String> = env
        .server
        .requests()
        .iter()
        .map(|r| r.path.clone())
        .collect();
    assert_eq!(
        paths,
        vec![
            "/pub/player/erik/games/archives",
            "/pub/player/erik/games/2007/08/pgn",
            "/pub/player/erik/games/2007/07/pgn",
        ]
    );
    let progress = feed.progress();
    assert_eq!(
        (progress.games, progress.months_done, progress.months_total),
        (3, 2, Some(2))
    );
}

#[test]
fn chesscom_import_goes_end_to_end_with_origin_and_does_not_duplicate() {
    let env = chesscom_env();
    let total = month_fixture_total();
    let mut store = Store::open_in_memory().expect("banco");

    let first = import(&mut store, env.chesscom.games("erik", &only_july_2007()));
    assert!(first.new > 10);
    assert_eq!(first.new + first.duplicates + first.errors, total);

    let list = store
        .list_imported(&ImportedQuery::default())
        .expect("lista");
    let origins = store.origins(ME, list[0].id).expect("origens");
    let origin = &origins[0];
    assert_eq!(origin.provider.as_deref(), Some("chess-com"));
    assert_eq!(origin.mode, ImportMode::Api);
    assert_eq!(origin.retrieved_at_ms, T0.as_millis());
    assert_eq!(origin.license.as_deref(), Some("chesscom-terms"));
    let remote = origin.remote_id.as_deref().expect("id remoto do Link");
    assert!(remote.chars().all(|c| c.is_ascii_digit()), "{remote}");

    let second = import(&mut store, env.chesscom.games("erik", &only_july_2007()));
    assert_eq!(second.new, 0);
    assert_eq!(second.duplicates + second.errors, total);
    assert_eq!(
        store
            .list_imported(&ImportedQuery::default())
            .expect("lista")
            .len(),
        list.len()
    );
}

#[test]
fn chesscom_games_fail_typed_and_fair_play_does_not_block_them() {
    let env = env_enabled();
    env.server.route(
        "/pub/player/fantasma/games/archives",
        FakeResponse::status(404),
    );
    let (_, last) = drain(&mut env.chesscom.games("fantasma", &GamesFilter::default()));
    assert!(matches!(
        last,
        GamesEvent::Failed(ProviderError::UnknownName { .. })
    ));

    let off = common::env();
    let (_, last) = drain(&mut off.chesscom.games("erik", &GamesFilter::default()));
    assert!(matches!(
        last,
        GamesEvent::Failed(ProviderError::Disabled { .. })
    ));
    assert!(off.server.requests().is_empty());

    let locked = chesscom_env();
    locked.net.fair_play_guard().engage("Partida online");
    let (chunks, last) = drain(&mut locked.chesscom.games("erik", &only_july_2007()));
    assert_eq!(last, GamesEvent::Finished);
    assert!(!chunks.is_empty());
}
