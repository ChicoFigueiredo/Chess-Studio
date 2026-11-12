//! FICS como visitante (#237, CX-04, CX-18): o protocolo ICS (texto, linha a linha) sobre o
//! transporte ICS do `chess-net`, sem interface.
//!
//! - [`Fics`]: o cliente. [`Fics::connect`] com [`FicsLogin::Guest`] abre a conexão pelo gate
//!   da camada de rede (Provedor ligado, catálogo, Trava de fair play, estado da rede).
//!   [`FicsLogin::Account`] (com senha) é **recusado antes de qualquer rede**: o FICS não
//!   oferece canal cifrado comprovado, e a senha nunca trafega em texto puro
//!   ([`FicsError::InsecureChannel`]; `docs/decisoes/2026-11-11-fics.md`).
//! - [`FicsSession`]: a sessão, consultável por quadro. Entra como visitante, busca
//!   adversário, aceita desafio, joga (intenção enviada, envio único), abandona, propõe e
//!   responde empate, aborta e observa Partidas. Só xadrez padrão; variantes recusadas.
//! - O parser das mensagens (prompt, `style 12`, início e fim de Partida) fica separado do
//!   transporte e do estado da sessão; entrada hostil nunca derruba nada.
//!
//! ```no_run
//! use chess_providers::{Fics, FicsLogin, FicsNotice};
//! # fn demo(fics: Fics, store: &mut chess_store::Store) -> Result<(), chess_providers::FicsError> {
//! let mut session = fics.connect(FicsLogin::Guest)?;
//! let mut id = None;
//! // A cada quadro (nunca bloqueia):
//! let polled = session.poll();
//! for notice in &polled.notices {
//!     if let FicsNotice::LoggedIn { handle } = notice {
//!         println!("no FICS como {handle}");
//!     }
//! }
//! if polled.created {
//!     let game = session.game().expect("criada");
//!     id = store.create_game(game.setup(), &Default::default()).ok();
//! }
//! if let Some(id) = id {
//!     store.append_events(id, &polled.events).ok(); // só o que o FICS confirmou
//! }
//! // Depois do login: buscar adversário (5 min + 0 s, sem rating) e, na Partida, jogar.
//! session.seek(5, 0)?;
//! # let mv = "e2e4".parse().unwrap();
//! let _ = session.play(mv);
//! # Ok(())
//! # }
//! ```

mod parse;
mod session;
mod tracker;

pub use parse::FicsChallenge;
pub use session::{FicsNotice, FicsOutgoing, FicsPoll, FicsSession, FicsStatus, ObservedGame};

use crate::ProviderError;
use chess_game::Rejection;
use chess_net::{Clock, IcsRequest, Net, ProviderId, Secret};
use std::fmt;
use std::sync::Arc;
use std::time::Duration;

/// Chave do Provedor no catálogo e no `OnlineGame` das Partidas do FICS.
pub(crate) const PROVIDER_KEY: &str = "fics";

/// Prazo padrão para o FICS confirmar a sessão de visitante (relógio injetado).
pub const DEFAULT_LOGIN_DEADLINE: Duration = Duration::from_secs(60);

/// Como entrar no FICS.
#[derive(Debug, Clone)]
pub enum FicsLogin {
    /// Visitante: nenhum segredo; o FICS dá um nome `GuestXXXX` e só Partidas sem rating.
    Guest,
    /// Conta registrada com senha: recusada enquanto não houver canal cifrado comprovado
    /// (CX-18). O `Debug` não mostra a senha, e ela é zerada ao descartar.
    Account { username: String, password: Secret },
}

/// Por que o FICS (ou o app) recusou algo. Em português, sem segredo.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum FicsError {
    /// Login com senha recusado: o FICS não tem canal cifrado comprovado e a senha iria em
    /// texto puro. Nada foi enviado; nenhuma conexão foi aberta.
    InsecureChannel,
    /// O servidor pediu senha durante a entrada como visitante: o app não responde com senha
    /// e fechou a conexão.
    PasswordRequested,
    /// O FICS não confirmou a sessão de visitante no prazo.
    LoginTimeout,
    /// A conexão caiu (ou o FICS a fechou). Com Partida em andamento, o FICS a dá como
    /// perdida por desconexão de visitante; o resultado não pôde ser confirmado.
    ConnectionClosed { game_in_progress: bool },
    /// Só xadrez padrão: o desafio ou a Partida foi recusado.
    UnsupportedVariant(String),
    /// A Partida local e o FICS não concordam: nenhum Lance local até um tabuleiro coerente.
    Desynced,
    /// Agora não dá (sem login, sem Partida, Partida em andamento…).
    NotAllowed(&'static str),
    /// O domínio recusou (Lance ilegal, não é a vez, já há Lance enviado…).
    Rejected(Rejection),
    /// Falha da camada de rede (Provedor desligado, sem conexão…).
    Provider(ProviderError),
}

impl fmt::Display for FicsError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InsecureChannel => f.write_str(
                "O FICS não oferece canal cifrado comprovado: a senha iria em texto puro pela \
                 rede (o timeseal só embaralha com uma chave pública, não protege). Entre como \
                 visitante; nada foi enviado.",
            ),
            Self::PasswordRequested => f.write_str(
                "O servidor pediu senha na entrada como visitante. O app nunca envia senha sem \
                 canal cifrado e encerrou a conexão.",
            ),
            Self::LoginTimeout => {
                f.write_str("O FICS não confirmou a entrada como visitante a tempo.")
            }
            Self::ConnectionClosed {
                game_in_progress: true,
            } => f.write_str(
                "A conexão com o FICS caiu durante a Partida. No FICS, a Partida de visitante \
                 que cai é perdida por desconexão; o resultado não pôde ser confirmado.",
            ),
            Self::ConnectionClosed {
                game_in_progress: false,
            } => f.write_str("A conexão com o FICS foi encerrada."),
            Self::UnsupportedVariant(v) => write!(
                f,
                "A variante «{v}» não é aceita: o app só joga xadrez padrão. Nada foi criado."
            ),
            Self::Desynced => f.write_str(
                "A Partida local e o FICS não concordam; espere o próximo tabuleiro do FICS.",
            ),
            Self::NotAllowed(why) => write!(f, "Agora não: {why}."),
            Self::Rejected(r) => r.fmt(f),
            Self::Provider(e) => e.fmt(f),
        }
    }
}

impl std::error::Error for FicsError {}

/// Cliente do FICS sobre a camada de rede. Barato de clonar.
#[derive(Clone)]
pub struct Fics {
    net: Net,
    clock: Arc<dyn Clock>,
    login_deadline: Duration,
}

impl fmt::Debug for Fics {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Fics").finish_non_exhaustive()
    }
}

impl Fics {
    pub fn new(net: Net, clock: Arc<dyn Clock>) -> Self {
        Self {
            net,
            clock,
            login_deadline: DEFAULT_LOGIN_DEADLINE,
        }
    }

    /// Prazo para o FICS confirmar a sessão de visitante.
    pub fn login_deadline(mut self, deadline: Duration) -> Self {
        self.login_deadline = deadline;
        self
    }

    /// Há canal cifrado comprovado para mandar senha? Hoje não: o único endereço ICS do
    /// FICS no catálogo é texto puro, e o app não tem transporte ICS cifrado.
    /// Mudar isto pede canal cifrado no catálogo **e** transporte ICS cifrado no app, com
    /// nova decisão (CX-18).
    pub fn password_login_available() -> bool {
        false
    }

    /// Um nome de jogador do FICS: de 3 a 17 letras (`help register`), sem espaço, número nem
    /// símbolo. É o que o desafio por nome aceita: nada mais vai na linha do `match`.
    pub fn valid_handle(name: &str) -> bool {
        (3..=17).contains(&name.len()) && name.bytes().all(|b| b.is_ascii_alphabetic())
    }

    /// Abre a sessão. Só visitante: com senha, recusa **antes** de abrir qualquer conexão
    /// ([`FicsError::InsecureChannel`]). O consentimento e o catálogo são conferidos pelo gate
    /// da rede; um Provedor desligado volta como [`FicsNotice::Closed`] no primeiro `poll`, sem
    /// conexão.
    pub fn connect(&self, login: FicsLogin) -> Result<FicsSession, FicsError> {
        if let FicsLogin::Account { .. } = login {
            // A senha (zerada ao descartar `login`) nunca chega perto da rede.
            return Err(FicsError::InsecureChannel);
        }
        let request = IcsRequest::to_catalog(ProviderId::Fics)
            .ok_or(FicsError::NotAllowed("o FICS não está no catálogo"))?;
        let link = self.net.connect_ics(request);
        Ok(FicsSession::new(
            self.net.clone(),
            self.clock.clone(),
            link,
            self.login_deadline,
        ))
    }
}
