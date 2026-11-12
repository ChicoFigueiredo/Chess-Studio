//! Opening Explorer do Lichess: mestres, Lichess e jogador (fluxo).

mod common;

use chess_net::testing::FakeResponse;
use chess_net::{Freshness, ProviderId};
use chess_providers::{
    Color, ExplorerEvent, LichessFilter, MastersFilter, Pace, PlayerFilter, ProviderError,
    YearMonth,
};
use common::*;
use std::time::Duration;

#[test]
fn masters_happy_path_returns_moves_games_opening_and_provenance() {
    let env = env_enabled();
    env.server
        .route("/masters", fixture("lichess-explorer-masters.http"));

    let answer = env
        .lichess
        .masters(AFTER_E4, &MastersFilter::default())
        .wait()
        .expect("mestres");

    let position = &answer.value;
    assert_eq!(position.outcomes.total(), 3000);
    assert_eq!(position.outcomes.white, 1000);
    assert_eq!(position.moves.len(), 2);
    assert_eq!(position.moves[0].uci, "c7c5");
    assert_eq!(position.moves[0].san, "c5");
    assert_eq!(position.moves[0].outcomes.draws, 350);
    assert_eq!(position.moves[0].average_rating, Some(2500));
    assert_eq!(position.games.len(), 2);
    assert_eq!(position.games[0].id, "AbCdEfGh");
    assert_eq!(position.games[0].winner, Some(Color::White));
    assert_eq!(position.games[1].winner, None);
    assert_eq!(position.games[0].white.rating, Some(2750));
    let opening = position.opening.as_ref().expect("abertura");
    assert_eq!(
        (opening.eco.as_str(), opening.name.as_str()),
        ("B00", "King's Pawn Game")
    );

    assert_eq!(answer.provenance.provider, ProviderId::Lichess);
    assert_eq!(answer.provenance.queried_at, T0);
    assert!(answer.provenance.license.contains("Lichess"));
    assert!(
        answer
            .provenance
            .source
            .starts_with("https://explorer.lichess.org/masters")
    );
    assert_eq!(answer.freshness, Freshness::Network);

    let sent = &env.server.requests()[0];
    assert_eq!(sent.path, "/masters");
    // A chave de Posição normaliza o FEN (sem contadores nem en passant não capturável).
    assert_eq!(
        sent.query_value("fen").as_deref(),
        Some("rnbqkbnr/pppppppp/8/8/4P3/8/PPPP1PPP/RNBQKBNR b KQkq - 0 1")
    );
}

#[test]
fn masters_filters_go_in_the_request_and_the_second_call_hits_the_cache() {
    let env = env_enabled();
    env.server
        .route("/masters", fixture("lichess-explorer-masters.http"));
    let filter = MastersFilter {
        since_year: Some(2000),
        until_year: Some(2020),
        moves: Some(5),
        top_games: Some(2),
    };

    env.lichess.masters(AFTER_E4, &filter).wait().expect("1");
    let second = env.lichess.masters(AFTER_E4, &filter).wait().expect("2");

    assert_eq!(env.server.requests().len(), 1, "o segundo veio do cache");
    assert_eq!(second.freshness, Freshness::Cache);
    let sent = &env.server.requests()[0];
    assert_eq!(sent.query_value("since").as_deref(), Some("2000"));
    assert_eq!(sent.query_value("until").as_deref(), Some("2020"));
    assert_eq!(sent.query_value("moves").as_deref(), Some("5"));
    assert_eq!(sent.query_value("topGames").as_deref(), Some("2"));
}

#[test]
fn lichess_explorer_sends_pace_rating_and_period_filters() {
    let env = env_enabled();
    env.server
        .route("/lichess", fixture("lichess-explorer-lichess.http"));
    let filter = LichessFilter {
        paces: vec![Pace::Blitz, Pace::Rapid],
        ratings: vec![1800, 2000],
        since: Some(YearMonth::new(2026, 1).expect("mês")),
        until: Some(YearMonth::new(2026, 10).expect("mês")),
        recent_games: Some(3),
        ..LichessFilter::default()
    };

    let answer = env
        .lichess
        .explorer(AFTER_E4, &filter)
        .wait()
        .expect("lichess");

    assert_eq!(answer.value.outcomes.total(), 1_000_000);
    let recent = &answer.value.games[0];
    assert!(recent.recent);
    assert_eq!(recent.speed.as_deref(), Some("blitz"));
    let sent = &env.server.requests()[0];
    assert_eq!(sent.query_value("speeds").as_deref(), Some("blitz,rapid"));
    assert_eq!(sent.query_value("ratings").as_deref(), Some("1800,2000"));
    assert_eq!(sent.query_value("since").as_deref(), Some("2026-01"));
    assert_eq!(sent.query_value("until").as_deref(), Some("2026-10"));
    assert_eq!(sent.query_value("variant").as_deref(), Some("standard"));
}

#[test]
fn an_unknown_rating_band_is_refused_before_the_network() {
    let env = env_enabled();
    let filter = LichessFilter {
        ratings: vec![1234],
        ..LichessFilter::default()
    };
    let error = env
        .lichess
        .explorer(AFTER_E4, &filter)
        .wait()
        .expect_err("faixa");
    assert!(
        matches!(error, ProviderError::InvalidInput { .. }),
        "{error:?}"
    );
    assert!(env.server.requests().is_empty());
}

#[test]
fn explorer_http_failures_are_typed() {
    let env = env_enabled();
    env.server.route("/masters", FakeResponse::status(404));
    let error = env
        .lichess
        .masters(AFTER_E4, &MastersFilter::default())
        .wait()
        .expect_err("404");
    assert_eq!(error, ProviderError::NoData);

    // A resposta real de 2026-11-11 (401): com o token da Conexão, quer dizer que ela
    // expirou ou foi revogada; a Conexão fica marcada no Cofre e o token continua lá.
    env.server
        .route("/masters", fixture("lichess-explorer-masters-401.http"));
    let other = "rnbqkbnr/pppppppp/8/8/3P4/8/PPP1PPPP/RNBQKBNR b KQkq d3 0 1";
    let error = env
        .lichess
        .masters(other, &MastersFilter::default())
        .wait()
        .expect_err("401");
    assert_eq!(
        error,
        ProviderError::ConnectionRejected {
            provider: ProviderId::Lichess,
            status: 401
        }
    );
    assert!(
        error.to_string().contains("expirou ou foi revogada"),
        "{error}"
    );
    let reference = env
        .vault
        .reference(EXPLORER_CONNECTION)
        .expect("referência");
    assert_eq!(reference.rejected.map(|r| r.status), Some(401));
    assert!(
        env.vault.read(EXPLORER_CONNECTION).is_ok(),
        "nada local se perde"
    );
}

#[test]
fn without_a_lichess_connection_the_explorer_says_so_before_the_network() {
    let env = env_enabled();
    env.lichess.use_connection(None);

    let error = env
        .lichess
        .masters(AFTER_E4, &MastersFilter::default())
        .wait()
        .expect_err("sem Conexão");
    assert_eq!(
        error,
        ProviderError::SignInRequired {
            provider: ProviderId::Lichess
        }
    );
    assert!(
        error.to_string().contains("requer Conexão do Lichess"),
        "{error}"
    );
    let error = env
        .lichess
        .explorer(AFTER_E4, &LichessFilter::default())
        .wait()
        .expect_err("sem Conexão");
    assert!(matches!(error, ProviderError::SignInRequired { .. }));
    let mut stream =
        env.lichess
            .player_explorer(AFTER_E4, "someone", Color::White, &PlayerFilter::default());
    let event = eventually_event(&mut stream);
    assert!(
        matches!(
            event,
            Some(ExplorerEvent::Failed(ProviderError::SignInRequired { .. }))
        ),
        "{event:?}"
    );
    assert_eq!(env.server.connections(), 0, "nada saiu sem a Conexão");

    // Conexão escolhida, mas o segredo foi apagado do Cofre: a mesma recusa, sem rede.
    env.lichess.use_connection(Some(EXPLORER_CONNECTION));
    env.vault.delete(EXPLORER_CONNECTION).expect("apaga");
    let error = env
        .lichess
        .masters(AFTER_E4, &MastersFilter::default())
        .wait()
        .expect_err("sem segredo");
    assert!(matches!(error, ProviderError::SignInRequired { .. }));
    assert_eq!(env.server.connections(), 0);
}

#[test]
fn the_explorer_carries_the_token_of_its_connection_and_nothing_else_does() {
    let env = env_enabled();
    env.server
        .route("/masters", fixture("lichess-explorer-masters.http"));
    env.server
        .route("/standard", fixture("lichess-tablebase-kpk.http"));

    env.lichess
        .masters(AFTER_E4, &MastersFilter::default())
        .wait()
        .expect("mestres");
    let _ = env.lichess.tablebase(KPK).wait();

    let sent = env.server.requests();
    let bearer = format!("Bearer {EXPLORER_TOKEN}");
    let masters = sent.iter().find(|r| r.path == "/masters").expect("mestres");
    assert_eq!(masters.header("authorization"), Some(bearer.as_str()));
    let finals = sent.iter().find(|r| r.path == "/standard").expect("finais");
    assert_eq!(finals.header("authorization"), None, "finais sem token");
}

fn eventually_event(stream: &mut chess_providers::ExplorerStream) -> Option<ExplorerEvent> {
    let start = std::time::Instant::now();
    while start.elapsed() < Duration::from_secs(30) {
        if let Some(event) = stream.try_next() {
            return Some(event);
        }
        std::thread::sleep(Duration::from_millis(5));
    }
    None
}

#[test]
fn explorer_rate_limit_malformed_body_and_bad_fen() {
    let env = env_enabled();
    env.server.route(
        "/masters",
        FakeResponse::status(429).header("retry-after", "60"),
    );
    let error = env
        .lichess
        .masters(AFTER_E4, &MastersFilter::default())
        .wait()
        .expect_err("429");
    let ProviderError::RateLimited { provider, until } = error else {
        panic!("esperava RateLimited, veio {error:?}");
    };
    assert_eq!(provider, ProviderId::Lichess);
    assert_eq!(until, T0.plus(Duration::from_secs(60)));
    assert!(error_text(&env).contains("pausa"));

    let env = env_enabled();
    env.server
        .route("/masters", FakeResponse::json("{\"nope\":1}"));
    let error = env
        .lichess
        .masters(AFTER_E4, &MastersFilter::default())
        .wait()
        .expect_err("malformado");
    assert!(
        matches!(error, ProviderError::Malformed { .. }),
        "{error:?}"
    );

    let error = env
        .lichess
        .masters("isto não é FEN", &MastersFilter::default())
        .wait()
        .expect_err("fen");
    assert!(
        matches!(error, ProviderError::InvalidInput { .. }),
        "{error:?}"
    );
}

fn error_text(env: &Env) -> String {
    env.lichess
        .masters(AFTER_E4, &MastersFilter::default())
        .wait()
        .expect_err("ainda limitado")
        .to_string()
}

#[test]
fn disabled_provider_sends_nothing() {
    let env = env();
    env.server
        .route("/masters", fixture("lichess-explorer-masters.http"));

    let error = env
        .lichess
        .masters(AFTER_E4, &MastersFilter::default())
        .wait()
        .expect_err("desligado");

    assert_eq!(
        error,
        ProviderError::Disabled {
            provider: ProviderId::Lichess
        }
    );
    assert!(env.server.requests().is_empty());
    assert_eq!(env.server.connections(), 0);
}

#[test]
fn fair_play_lock_refuses_every_explorer_endpoint() {
    let env = env_enabled();
    env.server
        .route("/masters", fixture("lichess-explorer-masters.http"));
    env.net.fair_play_guard().engage("Partida online");

    let masters = env
        .lichess
        .masters(AFTER_E4, &MastersFilter::default())
        .wait();
    let lichess = env
        .lichess
        .explorer(AFTER_E4, &LichessFilter::default())
        .wait();
    let mut player =
        env.lichess
            .player_explorer(AFTER_E4, "alguem", Color::White, &PlayerFilter::default());

    let locked = ProviderError::FairPlayLocked {
        provider: ProviderId::Lichess,
    };
    assert_eq!(masters.expect_err("trava"), locked);
    assert_eq!(lichess.expect_err("trava"), locked);
    assert_eq!(first_event(&mut player), ExplorerEvent::Failed(locked));
    assert!(env.server.requests().is_empty());
}

fn first_event(stream: &mut chess_providers::ExplorerStream) -> ExplorerEvent {
    let mut event = None;
    assert!(
        eventually(Duration::from_secs(30), || {
            event = stream.try_next();
            event.is_some()
        }),
        "o fluxo não entregou nada"
    );
    event.expect("evento")
}

#[test]
fn player_explorer_delivers_partial_results_before_the_end() {
    let env = env_enabled();
    // Fluxo que nunca termina sozinho: cada linha é um parcial.
    let line = br#"{"white":3,"draws":1,"black":2,"moves":[{"uci":"e7e5","san":"e5","white":2,"draws":1,"black":1,"averageOpponentRating":1700,"performance":1720}],"queuePosition":4}
"#;
    env.server
        .route("/player", FakeResponse::stream_forever(line));

    let mut stream = env.lichess.player_explorer(
        AFTER_E4,
        "alguem",
        Color::Black,
        &PlayerFilter {
            paces: vec![Pace::Rapid],
            ..PlayerFilter::default()
        },
    );

    let ExplorerEvent::Partial(first) = first_event(&mut stream) else {
        panic!("esperava um parcial");
    };
    assert!(!stream.is_finished(), "o parcial chegou antes do fim");
    assert_eq!(first.value.outcomes.total(), 6);
    assert_eq!(first.value.queue_position, Some(4));
    assert_eq!(first.value.moves[0].average_rating, Some(1700));
    assert_eq!(first.value.moves[0].performance, Some(1720));
    assert_eq!(first.provenance.provider, ProviderId::Lichess);
    assert_eq!(first.provenance.queried_at, T0);
    assert!(matches!(
        first_event(&mut stream),
        ExplorerEvent::Partial(_)
    ));
    assert!(stream.latest().is_some());

    stream.close();
    assert!(stream.is_finished());
    assert!(
        eventually(Duration::from_secs(30), || env.net.status().active_requests
            == 0),
        "o fluxo ficou preso depois de fechar"
    );
    let sent = &env.server.requests()[0];
    assert_eq!(sent.query_value("player").as_deref(), Some("alguem"));
    assert_eq!(sent.query_value("color").as_deref(), Some("black"));
    assert_eq!(sent.query_value("speeds").as_deref(), Some("rapid"));
}

#[test]
fn player_explorer_recorded_stream_ends_with_the_last_partial_as_the_final() {
    let env = env_enabled();
    env.server
        .route("/player", fixture("lichess-explorer-player.http"));

    let mut stream =
        env.lichess
            .player_explorer(AFTER_E4, "alguem", Color::White, &PlayerFilter::default());
    let mut partials = Vec::new();
    loop {
        match first_event(&mut stream) {
            ExplorerEvent::Partial(answer) => partials.push(answer),
            ExplorerEvent::Finished => break,
            ExplorerEvent::Failed(e) => panic!("falhou: {e}"),
        }
    }
    assert_eq!(partials.len(), 2);
    assert!(partials[0].value.outcomes.total() < partials[1].value.outcomes.total());
    assert_eq!(stream.latest().expect("final").value.outcomes.total(), 60);
    assert!(stream.is_finished());
}

#[test]
fn player_explorer_stream_failures_are_typed() {
    let env = env_enabled();
    env.server.route("/player", FakeResponse::status(429));
    let mut stream =
        env.lichess
            .player_explorer(AFTER_E4, "alguem", Color::White, &PlayerFilter::default());
    assert!(matches!(
        first_event(&mut stream),
        ExplorerEvent::Failed(ProviderError::RateLimited { .. })
    ));

    let env2 = env_enabled();
    env2.server
        .route("/player", FakeResponse::stream(&[b"isto nao e json\n"]));
    let mut stream =
        env2.lichess
            .player_explorer(AFTER_E4, "alguem", Color::White, &PlayerFilter::default());
    assert!(matches!(
        first_event(&mut stream),
        ExplorerEvent::Failed(ProviderError::Malformed { .. })
    ));

    // Nome com caracteres de caminho nunca chega à rede.
    let mut stream = env2.lichess.player_explorer(
        AFTER_E4,
        "../../etc",
        Color::White,
        &PlayerFilter::default(),
    );
    assert!(matches!(
        first_event(&mut stream),
        ExplorerEvent::Failed(ProviderError::InvalidInput { .. })
    ));
}

#[test]
fn dropping_a_query_cancels_the_pending_request() {
    let env = env_enabled();
    env.server.route("/masters", FakeResponse::stall());

    let query = env.lichess.masters(AFTER_E4, &MastersFilter::default());
    assert!(env.server.wait_for_requests(1, Duration::from_secs(30)));
    drop(query);

    // O pedido foi abandonado: a fila do Provedor se libera no fim do prazo, mas a consulta
    // já não é de ninguém e um novo pedido a outro caminho não fica preso atrás dela.
    let mut again = env.lichess.masters(AFTER_E4, &MastersFilter::default());
    again.cancel();
    assert_eq!(
        again.wait_timeout(Duration::from_secs(30)),
        Some(Err(ProviderError::Cancelled))
    );
}

#[test]
fn counts_that_overflow_do_not_panic() {
    // SEC-05: `white + draws + black` estourava com números vindos da rede.
    let env = env_enabled();
    env.server.route(
        "/masters",
        FakeResponse::json(
            "{\"white\":18446744073709551615,\"draws\":1,\"black\":1,\"moves\":[],\"topGames\":[]}",
        ),
    );

    let answer = env
        .lichess
        .masters(AFTER_E4, &MastersFilter::default())
        .wait()
        .expect("mestres");

    assert_eq!(answer.value.outcomes.total(), u64::MAX);
    assert_eq!(answer.value.outcomes.white, u64::MAX);
}

fn wire_games(prefix: &str, count: usize) -> String {
    let games: Vec<String> = (0..count)
        .map(|i| {
            format!(
                "{{\"id\":\"{prefix}{i}\",\"white\":{{\"name\":\"a\"}},\"black\":{{\"name\":\"b\"}}}}"
            )
        })
        .collect();
    format!("[{}]", games.join(","))
}

#[test]
fn a_huge_games_list_is_capped_and_deduplicated_fast() {
    // SEC-07: a deduplicação comparava cada recente com todas as de topo (quadrática).
    let env = env_enabled();
    let body = format!(
        "{{\"white\":1,\"draws\":1,\"black\":1,\"moves\":[],\"topGames\":{},\"recentGames\":{}}}",
        wire_games("t", 60_000),
        wire_games("t", 60_000),
    );
    env.server.route("/masters", FakeResponse::json(&body));

    let started = std::time::Instant::now();
    let answer = env
        .lichess
        .masters(AFTER_E4, &MastersFilter::default())
        .wait()
        .expect("mestres");

    assert!(
        started.elapsed() < Duration::from_secs(5),
        "{:?}",
        started.elapsed()
    );
    let games = &answer.value.games;
    assert!(games.len() <= 400, "{}", games.len());
    assert!(
        games.iter().all(|g| !g.recent),
        "ids repetidos entram uma vez"
    );
}
