#![allow(clippy::unwrap_used)]
//! Pacotes de problemas do Lichess pela rede (#238): `FakeServer` servindo um `.csv.zst`
//! sintético grande. Nenhuma chamada real (CX-17), e o arquivo real nunca é baixado.

use chess_net::testing::{FakeResponse, FakeServer};
use chess_net::{FakeClock, Net, NetConfig, ProviderId, Timestamp};
use chess_providers::{Lichess, ProviderError, PuzzleEvent, PuzzleOutcome, PuzzleRequest};
use chess_training::{Catalog, PuzzleFilter, list_packs, load_packs};
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::{Duration, Instant};

const PATH: &str = "/lichess_db_puzzle.csv.zst";
const HEADER: &str =
    "PuzzleId,FEN,Moves,Rating,RatingDeviation,Popularity,NbPlays,Themes,GameUrl,OpeningTags\n";
/// Posição ANTES do lance do adversário (Pretas jogam b7b6; depois Ra8#).
const BEFORE: &str = "6k1/1p3ppp/8/8/8/8/5PPP/R5K1 b - - 0 1";
/// 2026-11-11T12:00:00Z.
const T0: Timestamp = Timestamp::from_millis(1_794_398_400_000);
const WAIT: Duration = Duration::from_secs(60);

struct Env {
    server: FakeServer,
    net: Net,
    lichess: Lichess,
    dir: PathBuf,
}

fn temp_dir() -> PathBuf {
    static NEXT: AtomicUsize = AtomicUsize::new(0);
    let n = NEXT.fetch_add(1, Ordering::Relaxed);
    let dir = std::env::temp_dir().join(format!(
        "chess-providers-puzzles-{}-{n}",
        std::process::id()
    ));
    let _ = std::fs::remove_dir_all(&dir);
    dir
}

fn env(enabled: bool) -> Env {
    let clock = Arc::new(FakeClock::new(T0));
    let server = FakeServer::with_clock(clock.clone());
    let net = Net::open(
        NetConfig::in_memory()
            .clock(clock.clone())
            .jitter_seed(7)
            .route("database.lichess.org", &server.base_url()),
    )
    .unwrap();
    if enabled {
        net.set_enabled(ProviderId::Lichess, true).unwrap();
    }
    let lichess = Lichess::new(net.clone(), clock);
    Env {
        server,
        net,
        lichess,
        dir: temp_dir().join("pacotes"),
    }
}

/// CSV sintético: `rows` problemas com id, rating e temas variados (gerador congruencial, sem
/// padrão curto para o Zstandard não reduzir tudo a quase nada).
fn csv(rows: u64) -> String {
    const THEMES: [&str; 6] = [
        "mate mateIn2 short",
        "fork middlegame",
        "pin endgame",
        "mateIn2 backRankMate",
        "crushing opening",
        "hangingPiece oneMove",
    ];
    let mut text = String::from(HEADER);
    let mut x = 12345_u64;
    for i in 0..rows {
        x = x
            .wrapping_mul(6_364_136_223_846_793_005)
            .wrapping_add(1_442_695_040_888_963_407);
        let rating = 600 + (x >> 33) % 2200;
        let theme = THEMES[((x >> 20) % 6) as usize];
        text += &format!(
            "{:05x}{i},{BEFORE},b7b6 a1a8,{rating},75,90,{},{theme},https://lichess.org/{:05x}{i},\n",
            (x >> 40) & 0xfffff,
            x % 10_000,
            (x >> 40) & 0xfffff
        );
    }
    text
}

fn zst(text: &str) -> Vec<u8> {
    ruzstd::encoding::compress_to_vec(text.as_bytes(), ruzstd::encoding::CompressionLevel::Fastest)
}

fn request(env: &Env, filter: PuzzleFilter) -> PuzzleRequest {
    PuzzleRequest {
        filter,
        dir: env.dir.clone(),
        id: "lichess-teste".into(),
        name: "Lichess: teste".into(),
    }
}

/// Espera o fim e devolve o que veio (progressos incluídos).
fn finish(job: &mut chess_providers::PuzzleDownload) -> (Vec<PuzzleEvent>, PuzzleEvent) {
    let start = Instant::now();
    let mut seen = Vec::new();
    while start.elapsed() < WAIT {
        while let Some(event) = job.try_next() {
            if matches!(event, PuzzleEvent::Finished(_) | PuzzleEvent::Failed(_)) {
                return (seen, event);
            }
            seen.push(event);
        }
        std::thread::sleep(Duration::from_millis(5));
    }
    panic!("a baixada não terminou");
}

fn outcome(event: PuzzleEvent) -> PuzzleOutcome {
    match event {
        PuzzleEvent::Finished(outcome) => outcome,
        other => panic!("esperava Finished, veio {other:?}"),
    }
}

fn filter(themes: &[&str], max_count: usize) -> PuzzleFilter {
    PuzzleFilter {
        themes: themes.iter().map(|t| (*t).to_owned()).collect(),
        max_count,
        ..PuzzleFilter::default()
    }
}

#[test]
fn a_selecao_atendida_encerra_o_download_cedo_e_instala_o_pacote_com_a_origem() {
    let env = env(true);
    let compressed = zst(&csv(60_000));
    let total = compressed.len() as u64;
    assert!(
        total > 1_000_000,
        "o arquivo sintético é grande ({total} bytes)"
    );
    env.server.route(PATH, FakeResponse::ok(compressed));

    let mut job = env.lichess.download_puzzles(request(
        &env,
        PuzzleFilter {
            min_rating: Some(1200),
            max_rating: Some(1800),
            ..filter(&["mateIn2"], 25)
        },
    ));
    let (progress, end) = finish(&mut job);
    let outcome = outcome(end);

    assert!(!outcome.cancelled);
    assert_eq!(outcome.report.imported, 25);
    // Parou na 25ª seleção: muito menos linhas lidas do que as 60 mil do arquivo.
    assert!(outcome.report.lines_read < 5_000, "{:?}", outcome.report);
    let last = progress
        .iter()
        .rev()
        .find_map(|e| match e {
            PuzzleEvent::Progress(p) => Some(*p),
            _ => None,
        })
        .expect("avisou o andamento");
    assert_eq!(last.total, Some(total));
    assert!(
        last.downloaded < total / 4,
        "baixou {} de {total} bytes",
        last.downloaded
    );
    assert_eq!(last.selected, 25);

    // Origem do arquivo e do pacote.
    assert_eq!(outcome.provenance.provider, ProviderId::Lichess);
    assert_eq!(
        outcome.provenance.license,
        "CC0-1.0 (exportações gerais; conferir cada conjunto)"
    );
    assert_eq!(outcome.provenance.mode, chess_net::ImportMode::Download);
    let pack = outcome.pack.expect("instalou");
    assert_eq!(pack.count, 25);
    assert_eq!(pack.provider.as_deref(), Some("lichess"));
    assert_eq!(pack.license.as_deref(), Some("CC0-1.0"));
    assert_eq!(pack.downloaded_on.as_deref(), Some("2026-11-11"));
    assert_eq!(list_packs(&env.dir), vec![pack]);

    // Os Exercícios carregam e obedecem à seleção.
    let (loaded, errors) = load_packs(&env.dir, &Catalog::default());
    assert!(errors.is_empty());
    assert_eq!(loaded.len(), 25);
    assert!(loaded.iter().all(|e| {
        e.themes.iter().any(|t| t == "mateIn2")
            && e.rating.is_some_and(|r| (1200..=1800).contains(&r))
    }));
    // O corpo não foi para o cache da rede.
    let cached = env
        .net
        .read_cache(&chess_net::Request::get(
            ProviderId::Lichess,
            format!("https://database.lichess.org{PATH}"),
        ))
        .unwrap();
    assert!(cached.is_none());
}

#[test]
fn provedor_desligado_nao_baixa_nada_nem_instala() {
    let env = env(false);
    env.server.route(PATH, FakeResponse::ok(zst(&csv(100))));

    let mut job = env.lichess.download_puzzles(request(&env, filter(&[], 10)));
    let (_, end) = finish(&mut job);

    assert!(matches!(
        end,
        PuzzleEvent::Failed(ProviderError::Disabled {
            provider: ProviderId::Lichess
        })
    ));
    assert!(env.server.requests().is_empty(), "nenhum pedido saiu");
    assert!(list_packs(&env.dir).is_empty());
    assert!(!env.dir.exists());
}

#[test]
fn cancelar_no_meio_nao_deixa_pacote_pela_metade() {
    let env = env(true);
    // Seleção raríssima: a leitura percorre o arquivo todo, dando tempo de cancelar.
    env.server.route(PATH, FakeResponse::ok(zst(&csv(60_000))));
    let mut job = env
        .lichess
        .download_puzzles(request(&env, filter(&["mateIn5"], 100)));

    let start = Instant::now();
    loop {
        match job.try_next() {
            Some(PuzzleEvent::Progress(p)) if p.read > 0 => break,
            Some(PuzzleEvent::Finished(_) | PuzzleEvent::Failed(_)) => break,
            _ => {}
        }
        assert!(start.elapsed() < WAIT);
        std::thread::sleep(Duration::from_millis(2));
    }
    job.cancel();
    let (_, end) = finish(&mut job);
    let outcome = outcome(end);

    assert!(outcome.cancelled || outcome.pack.is_none());
    assert!(outcome.pack.is_none());
    assert!(list_packs(&env.dir).is_empty());
    // Nem o arquivo temporário da instalação atômica sobra.
    let leftovers = std::fs::read_dir(&env.dir).map_or(0, Iterator::count);
    assert_eq!(leftovers, 0);
}

#[test]
fn selecao_sem_resultado_nao_instala_pacote() {
    let env = env(true);
    env.server.route(PATH, FakeResponse::ok(zst(&csv(500))));
    let mut job = env
        .lichess
        .download_puzzles(request(&env, filter(&["mateIn5"], 10)));
    let (_, end) = finish(&mut job);
    let outcome = outcome(end);
    assert!(!outcome.cancelled);
    assert_eq!(outcome.report.imported, 0);
    assert_eq!(outcome.report.lines_read, 500);
    assert!(outcome.pack.is_none());
    assert!(list_packs(&env.dir).is_empty());
}

#[test]
fn queda_do_servidor_vira_erro_em_portugues_e_nao_instala() {
    let env = env(true);
    env.server.route(PATH, FakeResponse::status(503));
    let mut job = env.lichess.download_puzzles(request(&env, filter(&[], 10)));
    let (_, end) = finish(&mut job);
    assert!(matches!(end, PuzzleEvent::Failed(_)), "{end:?}");
    assert!(list_packs(&env.dir).is_empty());
}

#[test]
fn arquivo_que_nao_e_zstd_falha_sem_instalar() {
    let env = env(true);
    env.server.route(
        PATH,
        FakeResponse::ok(b"isto nao e zstd, e um html de erro".to_vec()),
    );
    let mut job = env.lichess.download_puzzles(request(&env, filter(&[], 10)));
    let (_, end) = finish(&mut job);
    assert!(matches!(end, PuzzleEvent::Failed(_)), "{end:?}");
    assert!(list_packs(&env.dir).is_empty());
}
