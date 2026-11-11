//! Apoio dos testes: relógio falso, servidor falso e uma rede em memória com os hosts dos
//! Provedores apontando para o servidor falso. A suíte nunca chama serviço real (CX-17).
#![allow(dead_code)]

use chess_net::testing::{FakeResponse, FakeServer};
use chess_net::{FakeClock, Net, NetConfig, ProviderId, Timestamp};
use chess_providers::{ChessCom, Lichess};
use std::path::Path;
use std::sync::Arc;
use std::time::{Duration, Instant};

/// 2026-11-11T12:00:00Z.
pub const T0: Timestamp = Timestamp::from_millis(1_794_398_400_000);

pub const HOSTS: [&str; 4] = [
    "lichess.org",
    "explorer.lichess.org",
    "tablebase.lichess.ovh",
    "api.chess.com",
];

pub struct Env {
    pub clock: Arc<FakeClock>,
    pub server: FakeServer,
    pub net: Net,
    pub lichess: Lichess,
    pub chesscom: ChessCom,
}

/// Rede com o servidor falso, sem nenhum Provedor ligado.
pub fn env() -> Env {
    let clock = Arc::new(FakeClock::new(T0));
    let server = FakeServer::with_clock(clock.clone());
    let mut config = NetConfig::in_memory().clock(clock.clone()).jitter_seed(7);
    for host in HOSTS {
        config = config.route(host, &server.base_url());
    }
    let net = Net::open(config).expect("abre a rede");
    let lichess = Lichess::new(net.clone(), clock.clone());
    let chesscom = ChessCom::new(net.clone(), clock.clone());
    Env {
        clock,
        server,
        net,
        lichess,
        chesscom,
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
