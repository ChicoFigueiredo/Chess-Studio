//! O cofre do sistema operacional, atrás de um trait ([`OsKeyring`]) para que a suíte use um
//! falso em memória (feature `testing`) e nunca grave no cofre real do usuário.
//!
//! A implementação real ([`SystemKeyring`]) usa o crate `keyring` (MIT OR Apache-2.0):
//! Secret Service por `zbus` no Linux (Rust puro: não exige `libdbus` na compilação nem na
//! distribuição; GNOME Keyring, KWallet com a ponte Secret Service, KeePassXC),
//! Credential Manager no Windows e Keychain no macOS.

use chess_net::Secret;
use std::fmt;

/// Nome do serviço sob o qual os segredos ficam no cofre do sistema.
pub const SERVICE: &str = "Chess Studio";

/// Falha do cofre do sistema, com mensagem fixa (os erros do sistema podem citar bytes do
/// segredo; nunca são repassados).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum KeyringError {
    /// O cofre do sistema não responde.
    Unavailable,
    /// O cofre do sistema respondeu com erro.
    Failed(&'static str),
}

impl KeyringError {
    pub(crate) fn detail(self) -> &'static str {
        match self {
            Self::Unavailable => "não responde",
            Self::Failed(detail) => detail,
        }
    }
}

/// Um cofre do sistema: guarda um segredo por conta, dentro do serviço [`SERVICE`].
pub trait OsKeyring: Send + Sync + fmt::Debug {
    /// O cofre do sistema existe e responde (sem gravar nada).
    fn is_available(&self) -> bool;
    fn set(&self, account: &str, secret: &Secret) -> Result<(), KeyringError>;
    /// `Ok(None)` se a conta não tem segredo.
    fn get(&self, account: &str) -> Result<Option<Secret>, KeyringError>;
    /// Apagar o que não existe não é erro.
    fn delete(&self, account: &str) -> Result<(), KeyringError>;
}

/// O cofre do sistema operacional de verdade.
#[derive(Debug, Clone, Copy, Default)]
pub struct SystemKeyring;

fn failed(err: &keyring::Error) -> KeyringError {
    KeyringError::Failed(match err {
        keyring::Error::NoStorageAccess(_) => "acesso negado ou cofre trancado",
        keyring::Error::NoDefaultStore => "nenhum cofre do sistema disponível",
        keyring::Error::TooLong(..) => "segredo longo demais para o cofre do sistema",
        keyring::Error::BadEncoding(_) | keyring::Error::BadDataFormat(..) => {
            "segredo ilegível no cofre do sistema"
        }
        _ => "falha no cofre do sistema",
    })
}

fn entry(account: &str) -> Result<keyring::Entry, KeyringError> {
    keyring::Entry::new(SERVICE, account).map_err(|e| match e {
        keyring::Error::NoDefaultStore => KeyringError::Unavailable,
        other => failed(&other),
    })
}

impl OsKeyring for SystemKeyring {
    fn is_available(&self) -> bool {
        keyring::Entry::store_status().is_ok()
    }

    fn set(&self, account: &str, secret: &Secret) -> Result<(), KeyringError> {
        entry(account)?
            .set_password(secret.expose_secret())
            .map_err(|e| failed(&e))
    }

    fn get(&self, account: &str) -> Result<Option<Secret>, KeyringError> {
        match entry(account)?.get_password() {
            Ok(value) => Ok(Some(Secret::new(value))),
            Err(keyring::Error::NoEntry) => Ok(None),
            Err(e) => Err(failed(&e)),
        }
    }

    fn delete(&self, account: &str) -> Result<(), KeyringError> {
        match entry(account)?.delete_credential() {
            Ok(()) | Err(keyring::Error::NoEntry) => Ok(()),
            Err(e) => Err(failed(&e)),
        }
    }
}
