//! Apoio comum dos testes: relógio falso, servidor falso e uma rede em memória com os hosts
//! dos Provedores apontando para o servidor falso. A suíte nunca chama serviço real (CX-17).
#![allow(dead_code)]

use chess_net::testing::FakeServer;
use chess_net::{FakeClock, Net, NetConfig, ProviderId, Request, Timestamp};
use std::sync::Arc;
use std::time::{Duration, Instant};

/// 2026-11-11T12:00:00Z.
pub const T0: Timestamp = Timestamp::from_millis(1_794_398_400_000);

pub const HOSTS: [&str; 6] = [
    "lichess.org",
    "explorer.lichess.org",
    "tablebase.lichess.org",
    "api.chess.com",
    "chess-api.com",
    "playstrategy.org",
];

pub fn clock() -> Arc<FakeClock> {
    Arc::new(FakeClock::new(T0))
}

/// Servidor falso que carimba cada pedido com o relógio falso.
pub fn server(clock: &Arc<FakeClock>) -> FakeServer {
    FakeServer::with_clock(clock.clone())
}

/// Configuração em memória: todos os hosts de [`HOSTS`] vão para `server`.
pub fn config(server: &FakeServer, clock: &Arc<FakeClock>) -> NetConfig {
    let mut config = NetConfig::in_memory().clock(clock.clone()).jitter_seed(7);
    for host in HOSTS {
        config = config.route(host, &server.base_url());
    }
    config
}

pub fn net(server: &FakeServer, clock: &Arc<FakeClock>) -> Net {
    Net::open(config(server, clock)).expect("abre a rede")
}

/// Rede com os Provedores ligados para o Perfil local ativo.
pub fn net_enabled(server: &FakeServer, clock: &Arc<FakeClock>, providers: &[ProviderId]) -> Net {
    let net = net(server, clock);
    for provider in providers {
        net.set_enabled(*provider, true).expect("liga o Provedor");
    }
    net
}

/// Explorador do Lichess (mestres) depois de 1. e4.
pub fn masters() -> Request {
    Request::get(ProviderId::Lichess, "https://explorer.lichess.org/masters").query("play", "e2e4")
}

/// Espera (tempo real, curto) até `cond` valer; para condições vindas de outras threads.
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

pub fn secs(n: u64) -> Duration {
    Duration::from_secs(n)
}
