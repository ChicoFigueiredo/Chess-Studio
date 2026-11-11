//! Apoio de teste (feature `testing`, só em `[dev-dependencies]`): servidor HTTP falso em
//! porta efêmera de loopback, respostas gravadas no formato do `curl -i` (CX-17) e o
//! servidor ICS falso ([`FakeIcsServer`], TCP de texto, #237). Com
//! `NetConfig::route`, os hosts oficiais vão para o servidor falso; a suíte nunca chama
//! serviço real.
//!
//! ```
//! use chess_net::testing::{FakeResponse, FakeServer};
//! use chess_net::{Net, NetConfig, ProviderId, Request};
//!
//! let server = FakeServer::start();
//! server.route("/standard", FakeResponse::json(r#"{"category":"win"}"#));
//! let net = Net::open(
//!     NetConfig::in_memory().route("tablebase.lichess.ovh", &server.base_url()),
//! )?;
//! net.set_enabled(ProviderId::Lichess, true)?;
//! let fetched = net
//!     .fetch(Request::get(ProviderId::Lichess, "https://tablebase.lichess.ovh/standard"))
//!     .wait()?;
//! assert_eq!(fetched.body, br#"{"category":"win"}"#);
//! assert_eq!(server.requests()[0].path, "/standard");
//! # Ok::<(), chess_net::NetError>(())
//! ```

mod ics;
mod recorded;
mod server;

pub use ics::FakeIcsServer;
pub use server::{FakeResponse, FakeServer, LiveFeed, ReceivedRequest};
