//! O Cofre (_SecretVault_, ADR-0006, CX-10): onde ficam os segredos das Conexões (tokens,
//! chaves). Sem Bevy e sem egui.
//!
//! - **Cofre do sistema operacional primeiro** ([`SystemKeyring`]): Secret Service no Linux
//!   (por `zbus`, Rust puro), Credential Manager no Windows, Keychain no macOS. O cofre do
//!   sistema fica atrás do trait [`OsKeyring`]; a suíte usa um falso em memória (feature
//!   `testing`) e nunca toca o cofre real do usuário.
//! - **Sem cofre do sistema, cofre próprio** em arquivo cifrado com senha-mestra: Argon2id +
//!   XChaCha20-Poly1305, formato versionado (ver o módulo `own`), criado pelo usuário
//!   ([`SecretVault::create_own`]), desbloqueado só quando um segredo é necessário,
//!   bloqueável, com troca de senha. **Nunca** texto puro em disco e **nunca** troca de cofre
//!   em silêncio: sem nenhum dos dois, o Cofre fica [`VaultStatus::Unavailable`] e recusa
//!   guardar.
//! - **Interface única**: situação ([`SecretVault::status`]), guardar, ler, apagar e listar
//!   referências ([`SecretRef`], sem segredo). `Debug` e erros nunca mostram segredo; o que é
//!   secreto é zerado na memória ao descartar (`zeroize`).
//! - Implementa [`chess_net::AuthorizationSource`]: com `NetConfig::vault(Arc::new(cofre))`, um
//!   pedido com `.connection(id)` leva o `Authorization` só para os endereços autenticados do
//!   Provedor daquela Conexão, e só no Perfil local dono dela. Um `401`/`403` marca a
//!   referência ([`SecretRef::rejected`]) sem apagar nada.
//!
//! ```no_run
//! use chess_net::{ConnectionId, LocalProfileId, NetConfig, ProviderId, Secret};
//! use chess_vault::{SecretVault, VaultConfig, VaultStatus};
//! use std::sync::Arc;
//!
//! let vault = SecretVault::open(VaultConfig::new("dados/cofre"))?;
//! if vault.status() == VaultStatus::Unavailable {
//!     // A interface explica e pede uma senha-mestra nova.
//!     vault.create_own(&Secret::new("uma senha longa"))?;
//! }
//! vault.store(LocalProfileId(1), ProviderId::Lichess, ConnectionId(7), Secret::new("lip_…"))?;
//! let config = NetConfig::at_path("dados/rede.db").vault(Arc::new(vault.clone()));
//! # Ok::<(), chess_vault::VaultError>(())
//! ```

mod error;
mod index;
mod keyring;
mod own;
mod vault;

#[cfg(feature = "testing")]
pub mod testing;

pub use error::{MIN_PASSWORD_CHARS, VaultError};
pub use index::{Rejection, SecretRef};
pub use keyring::{KeyringError, OsKeyring, SERVICE, SystemKeyring};
pub use own::KdfParams;
pub use vault::{SecretVault, StagedSecret, VaultConfig, VaultStatus};
