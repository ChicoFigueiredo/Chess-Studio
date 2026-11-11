//! Camada de rede por Provedor (ADR-0006): a **única** porta de saída do app para serviços
//! externos de xadrez. Sem Bevy e sem egui; rede síncrona com `ureq` em threads próprias
//! (CX-20), sem runtime assíncrono.
//!
//! # O que este crate garante
//!
//! Todo pedido passa por um ponto único (o *gate*), na ordem:
//!
//! 1. **Consentimento** (CX-09): o Provedor nasce desligado e só é chamado se o Perfil local
//!    o ligou. Desligado, o pedido é recusado com [`NetError::ProviderDisabled`] antes de
//!    abrir qualquer conexão.
//! 2. **Catálogo**: o endereço precisa ser HTTPS e estar entre os [`Endpoint`]s do Provedor;
//!    é o catálogo, e não quem chama, que diz o que o pedido faz ([`Capability`]).
//! 3. **Trava de fair play** ([`FairPlayGuard`], ADR-0007): com a trava ativa, pedidos que
//!    calculam (Explorador, avaliação em cache, finais, Engine remota) são recusados.
//! 4. **Autorização**: o cabeçalho `Authorization` vem do Cofre ([`AuthorizationSource`],
//!    etapa 3), preso a um host, e nunca aparece em `Debug`, erro ou log.
//! 5. **Limites**: token bucket por Provedor, `429`/`Retry-After` (o Provedor fica
//!    "limitado até"), backoff com jitter para falhas temporárias.
//!
//! Depois do gate vêm o cache com Origem ([`Provenance`]) em SQLite, por Perfil local, com
//! `ETag`/`If-None-Match` e leitura offline, e o estado da rede para a barra de status.
//!
//! # Uso
//!
//! ```no_run
//! use chess_net::{Net, NetConfig, ProviderId, Request};
//! use std::time::Duration;
//!
//! let net = Net::open(NetConfig::at_path("rede.db"))?;
//! net.set_enabled(ProviderId::Lichess, true)?; // depois do aviso de consentimento
//!
//! let mut pending = net.fetch(
//!     Request::get(ProviderId::Lichess, "https://explorer.lichess.org/masters")
//!         .position("fen", "rnbqkbnr/pppppppp/8/8/4P3/8/PPPP1PPP/RNBQKBNR b KQkq - 0 1")
//!         .cache_for(Duration::from_secs(7 * 24 * 3600)),
//! );
//! // A cada quadro (nunca bloqueia):
//! if let Some(result) = pending.try_take() {
//!     let fetched = result?;
//!     println!("{} bytes de {:?}", fetched.body.len(), fetched.provenance);
//! }
//! # Ok::<(), chess_net::NetError>(())
//! ```
//!
//! O tempo vem de um [`Clock`] injetado ([`FakeClock`] nos testes); a suíte usa o servidor
//! falso de [`testing`] (feature `testing`) e nunca chama serviço real (CX-17).

pub mod catalog;
mod clock;
mod error;
mod guard;
mod limits;
mod ndjson;
mod net;
mod request;
mod response;
mod secret;
mod status;
mod store;
mod transport;

#[cfg(feature = "testing")]
pub mod testing;

pub use catalog::{
    AuthMethod, CATALOG_VERSION, Capability, Category, Cost, Endpoint, Provider, ProviderId,
    RateLimit, Service, ServiceStatus, Variants, catalog, provider,
};
pub use clock::{Clock, FakeClock, SystemClock, Timestamp};
pub use error::NetError;
pub use guard::FairPlayGuard;
pub use limits::RetryPolicy;
pub use ndjson::{NdjsonError, NdjsonItem, NdjsonReader};
pub use net::{Net, NetConfig, Pending, Stream, StreamEvent};
pub use request::{Method, Request};
pub use response::{Fetched, Freshness, ImportMode, Provenance};
pub use secret::{Authorization, AuthorizationSource, ConnectionId, NoVault, Secret};
pub use status::{NetworkStatus, Operation, ProviderState};

use serde::{Deserialize, Serialize};

/// User-Agent genérico do app: nome, versão e o repositório público; nenhum dado do usuário.
pub const USER_AGENT: &str = concat!(
    "ChessStudio/",
    env!("CARGO_PKG_VERSION"),
    " (+https://github.com/ChicoFigueiredo/Chess-Studio)"
);

/// Perfil local dono do consentimento e do cache: o mesmo número do `ProfileId` do
/// `chess-store` (o app converte; este crate não depende do banco das Partidas).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
pub struct LocalProfileId(pub i64);
