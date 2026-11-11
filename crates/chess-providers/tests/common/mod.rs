//! Apoio dos testes: relógio falso, servidor falso e uma rede em memória com os hosts dos
//! Provedores apontando para o servidor falso. A suíte nunca chama serviço real (CX-17).
#![allow(dead_code)]

use chess_net::testing::{FakeResponse, FakeServer};
use chess_net::{
    ConnectionId, FakeClock, LocalProfileId, Net, NetConfig, ProviderId, Secret, Timestamp,
};
use chess_providers::{ChessCom, Lichess};
use chess_vault::testing::MemoryKeyring;
use chess_vault::{SecretVault, VaultConfig};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::{Duration, Instant};

/// 2026-11-11T12:00:00Z.
pub const T0: Timestamp = Timestamp::from_millis(1_794_398_400_000);

pub const HOSTS: [&str; 6] = [
    "lichess.org",
    "explorer.lichess.org",
    "tablebase.lichess.ovh",
    "api.chess.com",
    "playstrategy.org",
    "chess-api.com",
];

/// Conexões de teste da etapa 5 (#234): a de Engine do Lichess e as do PlayStrategy (jogador
/// e Engine). Cada uma tem o seu token no Cofre falso.
pub const LICHESS_ENGINE_CONNECTION: ConnectionId = ConnectionId(2);
pub const LICHESS_ENGINE_TOKEN: &str = "lip_TOKEN_DA_ENGINE_DE_TESTE";
pub const PLAYSTRATEGY_CONNECTION: ConnectionId = ConnectionId(3);
pub const PLAYSTRATEGY_TOKEN: &str = "ps_TOKEN_DO_JOGADOR_DE_TESTE";
pub const PLAYSTRATEGY_ENGINE_CONNECTION: ConnectionId = ConnectionId(4);
pub const PLAYSTRATEGY_ENGINE_TOKEN: &str = "ps_TOKEN_DA_ENGINE_DE_TESTE";

/// Conexão autorizada do Lichess que o Explorador usa nos testes (o Explorador exige
/// Conexão desde 2026-11-11).
pub const EXPLORER_CONNECTION: ConnectionId = ConnectionId(1);
pub const EXPLORER_TOKEN: &str = "lip_TOKEN_DO_EXPLORADOR_DE_TESTE";

pub struct Env {
    pub clock: Arc<FakeClock>,
    pub server: FakeServer,
    pub net: Net,
    pub lichess: Lichess,
    /// O mesmo cliente com o perfil do PlayStrategy.
    pub playstrategy: Lichess,
    pub chesscom: ChessCom,
    /// Cofre com o cofre do sistema falso (em memória) e o token de [`EXPLORER_CONNECTION`].
    pub vault: SecretVault,
    pub keyring: Arc<MemoryKeyring>,
    /// Diretório de dados do teste (Cofre e, quando pedido, o banco da rede).
    pub dir: PathBuf,
}

/// Diretório temporário novo e único (apagado se já existia).
pub fn temp_dir(name: &str) -> PathBuf {
    static NEXT: AtomicUsize = AtomicUsize::new(0);
    let n = NEXT.fetch_add(1, Ordering::Relaxed);
    let dir =
        std::env::temp_dir().join(format!("chess-providers-{name}-{}-{n}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    dir
}

/// Guarda no Cofre os tokens das Conexões da etapa 5 (Engine do Lichess e as do PlayStrategy).
/// Fica fora de `env` para não mudar o que os testes anteriores veem no cofre do sistema.
pub fn store_stage5_tokens(env: &Env) {
    for (provider, connection, token) in [
        (
            ProviderId::Lichess,
            LICHESS_ENGINE_CONNECTION,
            LICHESS_ENGINE_TOKEN,
        ),
        (
            ProviderId::PlayStrategy,
            PLAYSTRATEGY_CONNECTION,
            PLAYSTRATEGY_TOKEN,
        ),
        (
            ProviderId::PlayStrategy,
            PLAYSTRATEGY_ENGINE_CONNECTION,
            PLAYSTRATEGY_ENGINE_TOKEN,
        ),
    ] {
        env.vault
            .store(LocalProfileId(1), provider, connection, Secret::new(token))
            .expect("guarda o token de teste");
    }
}

/// Rede com o servidor falso, sem nenhum Provedor ligado.
pub fn env() -> Env {
    env_with(MemoryKeyring::new(), false)
}

/// Rede com o servidor falso e o Cofre sobre `keyring`; `net_on_disk` grava o banco da rede
/// no diretório de dados (para varrer o disco atrás de segredo).
pub fn env_with(keyring: MemoryKeyring, net_on_disk: bool) -> Env {
    let clock = Arc::new(FakeClock::new(T0));
    let server = FakeServer::with_clock(clock.clone());
    let dir = temp_dir("env");
    let keyring = Arc::new(keyring);
    let vault = SecretVault::open(
        VaultConfig::new(dir.join("cofre"))
            .keyring(keyring.clone())
            .clock(clock.clone()),
    )
    .expect("abre o Cofre");
    if vault.status() == chess_vault::VaultStatus::System {
        vault
            .store(
                LocalProfileId(1),
                ProviderId::Lichess,
                EXPLORER_CONNECTION,
                Secret::new(EXPLORER_TOKEN),
            )
            .expect("guarda o token do Explorador");
    }
    let base = if net_on_disk {
        NetConfig::at_path(dir.join("rede.db"))
    } else {
        NetConfig::in_memory()
    };
    let mut config = base
        .clock(clock.clone())
        .jitter_seed(7)
        .vault(Arc::new(vault.clone()));
    for host in HOSTS {
        config = config.route(host, &server.base_url());
    }
    let net = Net::open(config).expect("abre a rede");
    let lichess = Lichess::new(net.clone(), clock.clone());
    lichess.use_connection(Some(EXPLORER_CONNECTION));
    let playstrategy = Lichess::playstrategy(net.clone(), clock.clone());
    let chesscom = ChessCom::new(net.clone(), clock.clone());
    Env {
        clock,
        server,
        net,
        lichess,
        playstrategy,
        chesscom,
        vault,
        keyring,
        dir,
    }
}

/// Rede com Lichess e Chess.com ligados.
pub fn env_enabled() -> Env {
    let env = env();
    for provider in [ProviderId::Lichess, ProviderId::ChessCom] {
        env.net.set_enabled(provider, true).expect("liga");
    }
    env
}

pub fn fixture(name: &str) -> FakeResponse {
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures")
        .join(name);
    FakeResponse::recorded(&path).unwrap_or_else(|e| panic!("{name}: {e}"))
}

pub fn eventually(timeout: Duration, mut cond: impl FnMut() -> bool) -> bool {
    let start = Instant::now();
    while start.elapsed() < timeout {
        if cond() {
            return true;
        }
        std::thread::sleep(Duration::from_millis(10));
    }
    cond()
}

pub const AFTER_E4: &str = "rnbqkbnr/pppppppp/8/8/4P3/8/PPPP1PPP/RNBQKBNR b KQkq e3 0 1";
pub const START: &str = "rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 0 1";
/// KPK (exemplo da documentação do Lichess).
pub const KPK: &str = "4k3/6KP/8/8/8/8/7p/8 w - - 0 1";
