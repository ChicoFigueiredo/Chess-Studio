//! Dados de demonstração sem rede (`--demo-data`, #275): o app abre com um Perfil local de
//! exemplo num banco **temporário**, para testar a usabilidade sem conta nem internet.
//!
//! O que existe nele:
//!
//! - um Perfil local "Demonstração" (nunca o banco do usuário: a pasta fica no diretório
//!   temporário do sistema e é apagada ao sair; `--db` é ignorado);
//! - Partidas importadas de Origens variadas (Lichess e Chess.com por API, PlayStrategy, e
//!   arquivos de TWIC, ChessBase, PGN Mentor e Queen Alice), todas fictícias;
//! - um pacote de Exercícios ("Demonstração: táticas");
//! - respostas do Explorador (finais exatos e avaliação em cache) já no cache da rede,
//!   para aparecerem quando o Provedor for ligado, sem pedido nenhum;
//! - Conexões fictícias por nome público, **todas desligadas**: nenhum Provedor está ligado,
//!   nenhuma Conexão está autorizada e o Cofre é o falso (memória), nunca o real.
//!
//! Nenhum código aqui abre socket: a rede continua atrás do consentimento por Provedor (ADR-0006).

use bevy::prelude::*;
use chess_net::{LocalProfileId, Net, NetConfig, ProviderId, Request, Timestamp};
use chess_providers::ttl;
use chess_store::{
    ConnectionKind, ImportMode, ImportOptions, ImportSource, NewConnection, ProfileId, Provenance,
    Store,
};
use chess_training::{PuzzleFilter, import_lichess_csv, install_pack_dated};
use std::io::Cursor;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};

/// Nome do Perfil local de exemplo.
pub const PROFILE_NAME: &str = "Demonstração";

/// A flag de linha de comando.
pub const FLAG: &str = "--demo-data";

/// Marca o app em modo de demonstração: a rede usa o Cofre falso e o banco temporário.
#[derive(Resource, Debug, Clone, Copy)]
pub struct DemoMode;

/// A pasta temporária dos dados de demonstração; apagada ao descartar.
#[derive(Debug)]
pub struct DemoWorkspace {
    dir: PathBuf,
}

impl DemoWorkspace {
    /// Cria e popula a pasta no diretório temporário do sistema.
    pub fn create() -> Result<Self, String> {
        static NEXT: AtomicUsize = AtomicUsize::new(0);
        let n = NEXT.fetch_add(1, Ordering::Relaxed);
        Self::create_in(
            &std::env::temp_dir(),
            &format!("{}-{n}", std::process::id()),
        )
    }

    /// Como [`Self::create`], numa base dada (testes).
    pub fn create_in(base: &Path, label: &str) -> Result<Self, String> {
        let dir = base.join(format!("chess-studio-demo-{label}"));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).map_err(|e| format!("{}: {e}", dir.display()))?;
        let workspace = Self { dir };
        populate(&workspace.dir)?;
        Ok(workspace)
    }

    /// O banco de Partidas de demonstração.
    pub fn db_path(&self) -> PathBuf {
        self.dir.join("chess-studio.db")
    }

    pub fn dir(&self) -> &Path {
        &self.dir
    }
}

impl Drop for DemoWorkspace {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.dir);
    }
}

/// O banco que o app abre: com `--demo-data`, sempre o de demonstração (nem `--db` nem o
/// banco do usuário); senão o `--db` ou o padrão do usuário.
pub fn resolve_db(
    demo: Option<&DemoWorkspace>,
    flag_db: Option<PathBuf>,
    user_db: impl FnOnce(Option<PathBuf>) -> Option<PathBuf>,
) -> Option<PathBuf> {
    match demo {
        Some(demo) => Some(demo.db_path()),
        None => user_db(flag_db),
    }
}

// --- Partidas -----------------------------------------------------------------------------

struct Batch {
    source: ImportSource,
    mode: ImportMode,
    file: Option<&'static str>,
    pgn: &'static str,
}

const LICHESS: &str = r#"[Event "Rápida rated"]
[Site "https://lichess.org/DEMO0001"]
[Date "2026.10.04"]
[White "usuario_demo"]
[Black "adversaria_a"]
[Result "1-0"]
[WhiteElo "1612"]
[BlackElo "1588"]
[TimeControl "600+5"]
[ECO "C23"]

1. e4 e5 2. Bc4 Nc6 3. Qh5 Nf6 4. Qxf7# 1-0

[Event "Rápida rated"]
[Site "https://lichess.org/DEMO0002"]
[Date "2026.10.06"]
[White "adversario_b"]
[Black "usuario_demo"]
[Result "1/2-1/2"]
[WhiteElo "1630"]
[BlackElo "1617"]
[TimeControl "600+5"]
[ECO "C65"]

1. e4 e5 2. Nf3 Nc6 3. Bb5 a6 4. Ba4 Nf6 5. O-O Be7 6. Re1 b5 7. Bb3 d6 8. c3 O-O 9. h3 Na5 10. Bc2 c5 11. d4 Qc7 1/2-1/2

[Event "Blitz rated"]
[Site "https://lichess.org/DEMO0003"]
[Date "2026.10.09"]
[White "usuario_demo"]
[Black "adversario_c"]
[Result "0-1"]
[WhiteElo "1598"]
[BlackElo "1644"]
[TimeControl "180+2"]
[ECO "B90"]

1. e4 c5 2. Nf3 d6 3. d4 cxd4 4. Nxd4 Nf6 5. Nc3 a6 6. Be3 e5 7. Nb3 Be6 8. f3 Be7 9. Qd2 O-O 10. O-O-O Nbd7 0-1
"#;

const CHESS_COM: &str = r#"[Event "Live Chess"]
[Site "Chess.com"]
[Date "2026.10.12"]
[White "usuario_demo"]
[Black "rival_d"]
[Result "1-0"]
[WhiteElo "1540"]
[BlackElo "1522"]
[TimeControl "900+10"]
[ECO "D37"]
[Link "https://www.chess.com/game/live/9000000001"]

1. d4 d5 2. c4 e6 3. Nc3 Nf6 4. Bg5 Be7 5. e3 O-O 6. Nf3 Nbd7 7. Rc1 c6 8. Bd3 dxc4 9. Bxc4 Nd5 10. Bxe7 Qxe7 11. O-O Nxc3 12. Rxc3 e5 1-0

[Event "Live Chess"]
[Site "Chess.com"]
[Date "2026.10.13"]
[White "rival_e"]
[Black "usuario_demo"]
[Result "0-1"]
[WhiteElo "1561"]
[BlackElo "1549"]
[TimeControl "900+10"]
[ECO "A30"]
[Link "https://www.chess.com/game/live/9000000002"]

1. c4 e5 2. Nc3 Nf6 3. Nf3 Nc6 4. g3 d5 5. cxd5 Nxd5 6. Bg2 Nb6 7. O-O Be7 8. d3 O-O 0-1
"#;

const PLAYSTRATEGY: &str = r#"[Event "Rated Rapid game"]
[Site "https://playstrategy.org/DEMOps01"]
[Date "2026.10.15"]
[White "usuario_demo"]
[Black "jogador_f"]
[Result "1-0"]
[TimeControl "600+0"]
[ECO "D02"]

1. d4 d5 2. Bf4 Nf6 3. e3 c5 4. c3 Nc6 5. Nd2 e6 6. Ngf3 Bd6 7. Bg3 O-O 8. Bd3 b6 1-0
"#;

const TWIC: &str = r#"[Event "Torneio de Demonstração"]
[Site "Cidade Exemplo"]
[Date "2026.09.20"]
[Round "1"]
[White "Exemplo, Alberto"]
[Black "Modelo, Beatriz"]
[Result "1/2-1/2"]
[WhiteElo "2410"]
[BlackElo "2388"]
[ECO "E92"]

1. d4 Nf6 2. c4 g6 3. Nc3 Bg7 4. e4 d6 5. Nf3 O-O 6. Be2 e5 7. O-O Nc6 8. d5 Ne7 1/2-1/2

[Event "Torneio de Demonstração"]
[Site "Cidade Exemplo"]
[Date "2026.09.21"]
[Round "2"]
[White "Teste, Carlos"]
[Black "Exemplo, Alberto"]
[Result "1-0"]
[WhiteElo "2365"]
[BlackElo "2410"]
[ECO "C11"]

1. e4 e6 2. d4 d5 3. Nc3 Bb4 4. e5 c5 5. a3 Bxc3+ 6. bxc3 Ne7 7. Qg4 Qc7 8. Qxg7 Rg8 9. Qxh7 cxd4 1-0
"#;

const CHESSBASE: &str = r#"[Event "Partidas de estudo"]
[Site "?"]
[Date "2026.08.02"]
[White "Mestre Fictício"]
[Black "Aluno Fictício"]
[Result "1-0"]
[ECO "B12"]

1. e4 c6 2. d4 d5 3. Nc3 dxe4 4. Nxe4 Bf5 5. Ng3 Bg6 6. h4 h6 7. Nf3 Nd7 8. h5 Bh7 9. Bd3 Bxd3 10. Qxd3 e6 1-0
"#;

const PGN_MENTOR: &str = r#"[Event "Coleção de Demonstração"]
[Site "?"]
[Date "1985.01.01"]
[White "Jogador Clássico"]
[Black "Jogadora Clássica"]
[Result "0-1"]
[ECO "C00"]

1. e4 e6 2. d3 d5 3. Nd2 Nf6 4. Ngf3 c5 5. g3 Nc6 6. Bg2 Be7 7. O-O O-O 0-1
"#;

const QUEEN_ALICE: &str = r#"[Event "Amistoso de Demonstração"]
[Site "?"]
[Date "2026.07.14"]
[White "Amiga Um"]
[Black "Amigo Dois"]
[Result "1/2-1/2"]

1. Nf3 Nf6 2. c4 c5 3. Nc3 Nc6 4. g3 g6 5. Bg2 Bg7 6. O-O O-O 1/2-1/2
"#;

fn batches() -> [Batch; 8] {
    [
        Batch {
            source: ImportSource::Lichess,
            mode: ImportMode::Api,
            file: None,
            pgn: LICHESS,
        },
        Batch {
            source: ImportSource::ChessCom,
            mode: ImportMode::Api,
            file: None,
            pgn: CHESS_COM,
        },
        Batch {
            source: ImportSource::PlayStrategy,
            mode: ImportMode::Api,
            file: None,
            pgn: PLAYSTRATEGY,
        },
        Batch {
            source: ImportSource::Twic,
            mode: ImportMode::Manual,
            file: Some("twic-demo.pgn"),
            pgn: TWIC,
        },
        Batch {
            source: ImportSource::ChessBase,
            mode: ImportMode::Manual,
            file: Some("chessbase-demo.pgn"),
            pgn: CHESSBASE,
        },
        Batch {
            source: ImportSource::PgnMentor,
            mode: ImportMode::Manual,
            file: Some("pgnmentor-demo.pgn"),
            pgn: PGN_MENTOR,
        },
        Batch {
            source: ImportSource::QueenAlice,
            mode: ImportMode::Manual,
            file: Some("queen-alice-demo.pgn"),
            pgn: QUEEN_ALICE,
        },
        // Um arquivo sem Origem conhecida: a Partida entra como "arquivo".
        Batch {
            source: ImportSource::Other,
            mode: ImportMode::File,
            file: Some("outro-demo.pgn"),
            pgn: QUEEN_ALICE_OTHER,
        },
    ]
}

const QUEEN_ALICE_OTHER: &str = r#"[Event "Arquivo avulso"]
[Site "?"]
[Date "2026.06.01"]
[White "Avulso Um"]
[Black "Avulso Dois"]
[Result "*"]

1. d4 Nf6 2. Bf4 e6 3. e3 Bd6 4. Bg3 O-O *
"#;

// --- Exercícios ----------------------------------------------------------------------------

/// Mates na última linha, em formato de problemas do Lichess (fictícios, CC0 do projeto).
const PUZZLES: &str = "PuzzleId,FEN,Moves,Rating,RatingDeviation,Popularity,NbPlays,Themes,GameUrl,OpeningTags\n\
demo01,6k1/1p3ppp/8/8/8/8/5PPP/R5K1 b - - 0 1,b7b6 a1a8,820,75,90,10,mate mateIn1 backRankMate short,,\n\
demo02,6k1/p4ppp/8/8/8/8/5PPP/3R2K1 b - - 0 1,a7a6 d1d8,900,75,90,10,mate mateIn1 backRankMate short,,\n\
demo03,6k1/p4ppp/8/8/8/8/5PPP/2R3K1 b - - 0 1,a7a5 c1c8,980,75,90,10,mate mateIn1 backRankMate short,,\n\
demo04,6k1/p4ppp/8/8/8/8/5PPP/4R1K1 b - - 0 1,a7a6 e1e8,1050,75,90,10,mate mateIn1 backRankMate short,,\n";

const PACK_ID: &str = "demo-taticas";
const PACK_NAME: &str = "Demonstração: táticas";

fn install_exercises(dir: &Path, profile: ProfileId) -> Result<(), String> {
    let cancel = AtomicBool::new(false);
    let (exercises, report) = import_lichess_csv(
        Cursor::new(PUZZLES),
        &PuzzleFilter::default(),
        &cancel,
        &mut |_| {},
    )
    .map_err(|e| e.to_string())?;
    if exercises.is_empty() {
        return Err(format!("pacote de demonstração vazio: {report:?}"));
    }
    let packs = dir.join("exercise-packs").join(profile.0.to_string());
    let today = "2026-11-11";
    install_pack_dated(&packs, PACK_ID, PACK_NAME, Some(today), &exercises)
        .map(|_| ())
        .map_err(|e| e.to_string())
}

// --- Conexões fictícias, todas desligadas ---------------------------------------------------

/// Nomes públicos fictícios por Provedor. Sem segredo, sem autorização, Provedor desligado.
const CONNECTIONS: [(ImportSource, &str, &str); 3] = [
    (
        ImportSource::Lichess,
        "usuario_demo",
        "Minha conta de exemplo",
    ),
    (
        ImportSource::ChessCom,
        "usuario_demo",
        "Minha conta de exemplo",
    ),
    (
        ImportSource::PlayStrategy,
        "usuario_demo",
        "Minha conta de exemplo",
    ),
];

// --- Cache do Explorador --------------------------------------------------------------------

/// KPK, o exemplo da documentação dos finais exatos.
pub const KPK_FEN: &str = "4k3/6KP/8/8/8/8/7p/8 w - - 0 1";
/// A Posição inicial, para a linha "Em cache (Lichess)" do Painel Análise.
pub const START_FEN: &str = "rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 0 1";

const KPK_BODY: &str = r#"{"checkmate":false,"stalemate":false,"variant_win":false,"variant_loss":false,"insufficient_material":false,"dtz":1,"precise_dtz":1,"dtm":17,"category":"win","moves":[{"uci":"h7h8q","san":"h8=Q+","zeroing":true,"checkmate":false,"stalemate":false,"insufficient_material":false,"dtz":-2,"dtm":-16,"category":"loss"},{"uci":"g7g8","san":"Kg8","zeroing":false,"checkmate":false,"stalemate":false,"insufficient_material":false,"dtz":0,"dtm":0,"category":"draw"},{"uci":"g7f6","san":"Kf6","zeroing":false,"checkmate":false,"stalemate":false,"insufficient_material":false,"dtz":1,"dtm":19,"category":"win"}]}"#;

const START_EVAL_BODY: &str = r#"{"fen":"rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 0 1","knodes":695524,"depth":75,"pvs":[{"moves":"e2e4 e7e5 g1f3 b8c6 f1c4 g8f6 d2d3 f8c5 e1g1 d7d6","cp":19}]}"#;

/// Pedido dos finais exatos, igual ao que o adaptador faz.
fn tablebase_request(fen: &str) -> Request {
    Request::get(
        ProviderId::Lichess,
        "https://tablebase.lichess.ovh/standard",
    )
    .query("fen", fen)
    .cache_for(ttl::TABLEBASE)
}

/// Pedido da avaliação em cache (uma linha), igual ao que o adaptador faz.
fn cloud_eval_request(fen: &str) -> Request {
    Request::get(ProviderId::Lichess, "https://lichess.org/api/cloud-eval")
        .position("fen", fen)
        .query("multiPv", "1")
        .cache_for(ttl::CLOUD_EVAL)
}

/// Grava no cache da rede as respostas do Explorador (sem pedido nenhum). Chega às respostas
/// só depois de ligar o Lichess, como com qualquer cache.
pub fn seed_explorer_cache(net: &Net, now: Timestamp) -> Result<(), String> {
    let two_days_ago = Timestamp::from_millis(now.as_millis() - 2 * 86_400_000);
    for (request, body) in [
        (tablebase_request(KPK_FEN), KPK_BODY),
        (cloud_eval_request(START_FEN), START_EVAL_BODY),
    ] {
        net.seed_cache(
            &request,
            "application/json",
            body.as_bytes().to_vec(),
            two_days_ago,
        )
        .map_err(|e| e.to_string())?;
    }
    Ok(())
}

// --- Montagem -------------------------------------------------------------------------------

/// Preenche `dir` com o banco, o `rede.db` e os pacotes de Exercícios da demonstração.
fn populate(dir: &Path) -> Result<(), String> {
    let db = dir.join("chess-studio.db");
    let mut store = Store::open(&db).map_err(|e| e.to_string())?;
    let profile = store.default_profile().map_err(|e| e.to_string())?;
    store
        .rename_profile(profile, PROFILE_NAME)
        .map_err(|e| e.to_string())?;
    let now_ms = crate::net_service::NetService::now().as_millis();
    for batch in batches() {
        let provenance = Provenance::from_source(batch.source, batch.mode, now_ms);
        let provenance = if batch.source == ImportSource::Other {
            Provenance::unknown(batch.mode, now_ms)
        } else {
            provenance
        };
        let options = ImportOptions::new(batch.file, provenance);
        store
            .import_pgn_with(profile, batch.pgn, &options)
            .map_err(|e| e.to_string())?;
    }
    for (provider, identifier, nickname) in CONNECTIONS {
        let connection = NewConnection::new(
            provider.provider_id(),
            ConnectionKind::PublicName,
            identifier,
        )
        .nickname(nickname);
        store
            .add_connection(profile, &connection)
            .map_err(|e| e.to_string())?;
    }
    install_exercises(dir, profile)?;

    let net = Net::open(
        NetConfig::at_path(dir.join(crate::net_service::NET_DB_FILE))
            .profile(LocalProfileId(profile.0)),
    )
    .map_err(|e| e.to_string())?;
    seed_explorer_cache(&net, Timestamp::from_millis(now_ms))
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;
    use crate::net_service::NetService;
    use chess_net::testing::{FakeResponse, FakeServer};
    use chess_net::{FakeClock, Freshness};
    use chess_providers::Lichess;
    use chess_store::ImportedQuery;
    use std::sync::Arc;

    fn temp_base(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "chess-studio-demo-test-{name}-{}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn the_demo_profile_has_games_from_many_origins_an_exercise_pack_and_switched_off_connections()
    {
        let base = temp_base("content");
        let demo = DemoWorkspace::create_in(&base, "a").unwrap();
        let store = Store::open(demo.db_path()).unwrap();
        let profile = store.default_profile().unwrap();
        assert_eq!(store.profile(profile).unwrap().name, PROFILE_NAME);

        let games = store
            .list_imported(&ImportedQuery {
                profile: Some(profile),
                ..ImportedQuery::default()
            })
            .unwrap();
        assert!(games.len() >= 12, "só {} Partidas", games.len());
        let mut providers: Vec<Option<String>> = games
            .iter()
            .flat_map(|g| store.origins(profile, g.id).unwrap())
            .map(|o| o.provider)
            .collect();
        providers.sort();
        providers.dedup();
        for wanted in [
            ImportSource::Lichess,
            ImportSource::ChessCom,
            ImportSource::PlayStrategy,
            ImportSource::Twic,
            ImportSource::ChessBase,
            ImportSource::PgnMentor,
            ImportSource::QueenAlice,
        ] {
            assert!(
                providers
                    .iter()
                    .any(|p| p.as_deref() == Some(wanted.provider_id())),
                "falta a Origem {wanted:?}: {providers:?}"
            );
        }

        let packs = chess_training::list_packs(
            &demo
                .dir()
                .join("exercise-packs")
                .join(profile.0.to_string()),
        );
        assert_eq!(packs.len(), 1);
        assert_eq!(packs[0].id, PACK_ID);

        let connections = store.list_connections(profile, None).unwrap();
        assert_eq!(connections.len(), CONNECTIONS.len());
        assert!(connections.iter().all(|c| !c.is_authorized()));
        drop(store);
        drop(demo);
        std::fs::remove_dir_all(base).ok();
    }

    #[test]
    fn every_demo_game_replays_with_legal_moves() {
        let base = temp_base("legal");
        let demo = DemoWorkspace::create_in(&base, "b").unwrap();
        let store = Store::open(demo.db_path()).unwrap();
        let profile = store.default_profile().unwrap();
        let games = store
            .list_imported(&ImportedQuery {
                profile: Some(profile),
                ..ImportedQuery::default()
            })
            .unwrap();
        let total: usize = batches()
            .iter()
            .map(|b| chess_pgn::parse_pgn_lenient(b.pgn).games.len())
            .sum();
        let errors: usize = batches()
            .iter()
            .map(|b| chess_pgn::parse_pgn_lenient(b.pgn).errors.len())
            .sum();
        assert_eq!(errors, 0, "Partidas de demonstração com erro");
        assert_eq!(games.len(), total);
        drop(store);
        drop(demo);
        std::fs::remove_dir_all(base).ok();
    }

    #[test]
    fn the_workspace_is_removed_when_dropped() {
        let base = temp_base("drop");
        let demo = DemoWorkspace::create_in(&base, "c").unwrap();
        let dir = demo.dir().to_path_buf();
        assert!(dir.join("chess-studio.db").exists());
        assert!(dir.join("rede.db").exists());
        drop(demo);
        assert!(!dir.exists());
        std::fs::remove_dir_all(base).ok();
    }

    /// O banco do usuário (um arquivo sentinela) nunca é aberto, trocado nem alterado, e `--db`
    /// não desvia a demonstração para ele.
    #[test]
    fn the_real_database_is_never_touched() {
        let base = temp_base("real");
        let real = base.join("chess-studio.db");
        std::fs::write(&real, b"banco do usuario, nao mexa").unwrap();
        let before = std::fs::metadata(&real).unwrap().modified().unwrap();

        let demo = DemoWorkspace::create_in(&base, "d").unwrap();
        let chosen = resolve_db(Some(&demo), Some(real.clone()), |flag| {
            panic!("o banco do usuário foi consultado: {flag:?}")
        })
        .unwrap();
        assert_eq!(chosen, demo.db_path());
        assert_ne!(chosen, real);
        assert!(chosen.starts_with(std::env::temp_dir()));

        // A rede de demonstração mora ao lado do banco de demonstração, com o Cofre falso.
        let service = NetService::open_demo(&NetService::db_path_beside(&chosen));
        assert!(service.is_persistent());
        assert!(!service.is_enabled(ProviderId::Lichess));
        drop(service);

        assert_eq!(std::fs::read(&real).unwrap(), b"banco do usuario, nao mexa");
        assert_eq!(
            std::fs::metadata(&real).unwrap().modified().unwrap(),
            before
        );
        // Sem a demonstração, a regra de sempre vale.
        let normal = resolve_db(None, Some(real.clone()), |flag| flag).unwrap();
        assert_eq!(normal, real);
        drop(demo);
        std::fs::remove_dir_all(base).ok();
    }

    /// Zero pedidos: com o Provedor desligado nada sai; ligado, as respostas do Explorador vêm
    /// do cache e o servidor (que receberia qualquer pedido) não vê nenhum.
    #[test]
    fn the_cached_explorer_answers_need_zero_network_requests() {
        let now = Timestamp::from_millis(1_794_398_400_000);
        let clock = Arc::new(FakeClock::new(now));
        let server = FakeServer::with_clock(clock.clone());
        server.route("/", FakeResponse::json("{}"));
        let mut config = NetConfig::in_memory().clock(clock.clone()).jitter_seed(7);
        for host in [
            "lichess.org",
            "explorer.lichess.org",
            "tablebase.lichess.ovh",
        ] {
            config = config.route(host, &server.base_url());
        }
        let net = Net::open(config).unwrap();
        seed_explorer_cache(&net, now).unwrap();
        let service = NetService::from_net(net, clock, false);
        let lichess: &Lichess = service.lichess();

        // Desligado: o consentimento barra, sem pedido.
        assert!(lichess.tablebase(KPK_FEN).wait().is_err());
        assert!(lichess.cloud_eval(START_FEN, 1).wait().is_err());
        assert!(server.requests().is_empty());

        // Ligado: finais exatos e avaliação saem do cache, ainda sem pedido.
        service.set_enabled(ProviderId::Lichess, true).unwrap();
        let table = lichess.tablebase(KPK_FEN).wait().unwrap();
        assert_eq!(table.freshness, Freshness::Cache);
        assert!(!table.value.moves.is_empty());
        let eval = lichess.cloud_eval(START_FEN, 1).wait().unwrap();
        assert_eq!(eval.freshness, Freshness::Cache);
        assert!(eval.value.is_some());
        assert!(server.requests().is_empty(), "{:?}", server.requests());
    }
}
