//! Apoio de teste (feature `testing`, só em `[dev-dependencies]`): um cofre do sistema falso,
//! em memória, para a suíte nunca depender de haver cofre na máquina nem gravar no cofre
//! real do usuário.

use crate::keyring::{KeyringError, OsKeyring};
use chess_net::Secret;
use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Mutex, MutexGuard, PoisonError};

/// Cofre do sistema falso. Pode fingir que não existe ([`MemoryKeyring::unavailable`]) ou que
/// falha ([`MemoryKeyring::set_failing`]).
#[derive(Debug)]
pub struct MemoryKeyring {
    entries: Mutex<HashMap<String, Secret>>,
    available: AtomicBool,
    failing: AtomicBool,
}

impl Default for MemoryKeyring {
    fn default() -> Self {
        Self {
            entries: Mutex::new(HashMap::new()),
            available: AtomicBool::new(true),
            failing: AtomicBool::new(false),
        }
    }
}

fn lock<T>(mutex: &Mutex<T>) -> MutexGuard<'_, T> {
    mutex.lock().unwrap_or_else(PoisonError::into_inner)
}

impl MemoryKeyring {
    pub fn new() -> Self {
        Self::default()
    }

    /// Sistema sem cofre (Linux sem Secret Service, sessão sem D-Bus).
    pub fn unavailable() -> Self {
        let keyring = Self::default();
        keyring.available.store(false, Ordering::SeqCst);
        keyring
    }

    /// Cada operação passa a falhar (D-Bus caiu, acesso negado).
    pub fn set_failing(&self, failing: bool) {
        self.failing.store(failing, Ordering::SeqCst);
    }

    /// Contas com segredo, em ordem.
    pub fn accounts(&self) -> Vec<String> {
        let mut out: Vec<String> = lock(&self.entries).keys().cloned().collect();
        out.sort();
        out
    }

    fn check(&self) -> Result<(), KeyringError> {
        if !self.available.load(Ordering::SeqCst) {
            return Err(KeyringError::Unavailable);
        }
        if self.failing.load(Ordering::SeqCst) {
            return Err(KeyringError::Failed("falha simulada"));
        }
        Ok(())
    }
}

impl OsKeyring for MemoryKeyring {
    fn is_available(&self) -> bool {
        self.available.load(Ordering::SeqCst)
    }

    fn set(&self, account: &str, secret: &Secret) -> Result<(), KeyringError> {
        self.check()?;
        lock(&self.entries).insert(account.to_owned(), secret.clone());
        Ok(())
    }

    fn get(&self, account: &str) -> Result<Option<Secret>, KeyringError> {
        self.check()?;
        Ok(lock(&self.entries).get(account).cloned())
    }

    fn delete(&self, account: &str) -> Result<(), KeyringError> {
        self.check()?;
        lock(&self.entries).remove(account);
        Ok(())
    }
}
