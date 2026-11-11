//! Apoio de teste (feature `testing`, só em `[dev-dependencies]`): servidor HTTP falso em
//! porta efêmera de loopback e respostas gravadas no formato do `curl -i` (CX-17). Com
//! `NetConfig::route`, os hosts oficiais vão para o servidor falso; a suíte nunca chama
//! serviço real.
//!
//! ```
//! use chess_net::testing::{FakeResponse, FakeServer};
//! use chess_net::{Net, NetConfig, ProviderId, Request};
//!
//! let server = FakeServer::start();
//! server.route("/masters", FakeResponse::json(r#"{"white":1}"#));
//! let net = Net::open(
//!     NetConfig::in_memory().route("explorer.lichess.org", &server.base_url()),
//! )?;
//! net.set_enabled(ProviderId::Lichess, true)?;
//! let fetched = net
//!     .fetch(Request::get(ProviderId::Lichess, "https://explorer.lichess.org/masters"))
//!     .wait()?;
//! assert_eq!(fetched.body, br#"{"white":1}"#);
//! assert_eq!(server.requests()[0].path, "/masters");
//! # Ok::<(), chess_net::NetError>(())
//! ```

mod recorded;
mod server;

pub use server::{FakeResponse, FakeServer, ReceivedRequest};
