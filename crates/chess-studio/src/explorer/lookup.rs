//! Consulta de rede que nunca trava a interface: espera a Posição parar (`Settle`), cancela o
//! pedido antigo ao mudar de Posição e entrega o resultado por quadro (`Lookup::poll`).
//!
//! Sem Bevy nem egui: o tempo entra como `Instant` injetado, para testar sem esperar.

use chess_providers::{
    Answer, ExplorerEvent, ExplorerPosition, ExplorerStream, ProviderError, Query,
};
use std::sync::{Mutex, PoisonError};
use std::time::{Duration, Instant};

/// Quanto tempo a Posição precisa ficar parada antes de o primeiro pedido sair. Navegar pela
/// Notação segurando a seta não dispara um pedido por Lance.
pub const SETTLE_DELAY: Duration = Duration::from_millis(300);

/// Detecta quando uma chave (Posição + fonte + nome) parou de mudar.
#[derive(Debug, Clone)]
pub struct Settle<K> {
    key: Option<K>,
    since: Option<Instant>,
    fired: bool,
}

impl<K> Default for Settle<K> {
    fn default() -> Self {
        Self {
            key: None,
            since: None,
            fired: false,
        }
    }
}

impl<K: PartialEq + Clone> Settle<K> {
    /// Chame a cada quadro com a chave atual. Devolve `true` **uma única vez** por chave: no
    /// primeiro quadro em que ela já durou [`SETTLE_DELAY`].
    pub fn poll(&mut self, key: &K, now: Instant) -> bool {
        if self.key.as_ref() != Some(key) {
            self.key = Some(key.clone());
            self.since = Some(now);
            self.fired = false;
        }
        if self.fired {
            return false;
        }
        let waited = self
            .since
            .map_or(Duration::ZERO, |s| now.saturating_duration_since(s));
        if waited >= SETTLE_DELAY {
            self.fired = true;
            return true;
        }
        false
    }

    /// A chave atual ainda espera para disparar.
    pub fn is_settling(&self) -> bool {
        self.key.is_some() && !self.fired
    }

    /// Falta quanto para disparar (para pedir o próximo quadro à interface).
    pub fn remaining(&self, now: Instant) -> Option<Duration> {
        if !self.is_settling() {
            return None;
        }
        let waited = self
            .since
            .map_or(Duration::ZERO, |s| now.saturating_duration_since(s));
        Some(SETTLE_DELAY.saturating_sub(waited))
    }

    /// Esquece a chave: a próxima consulta recomeça a espera (e dispara de novo para a mesma
    /// chave, como no "Tentar de novo").
    pub fn reset(&mut self) {
        self.key = None;
        self.since = None;
        self.fired = false;
    }
}

/// O que um pedido pendente entrega a cada quadro.
pub enum Polled<T> {
    Waiting,
    /// Resultado parcial (fluxo do Explorador de jogador): vale mostrar, ainda vem mais.
    Partial(Answer<T>),
    Done(Answer<T>),
    Failed(ProviderError),
}

/// Um pedido que a interface consulta por quadro e pode cancelar.
pub trait Pending<T>: Send {
    fn poll(&mut self) -> Polled<T>;
    fn cancel(&mut self);
}

impl<T: Send> Pending<T> for Query<T> {
    fn poll(&mut self) -> Polled<T> {
        match self.try_take() {
            None => Polled::Waiting,
            Some(Ok(answer)) => Polled::Done(answer),
            Some(Err(error)) => Polled::Failed(error),
        }
    }

    fn cancel(&mut self) {
        Query::cancel(self);
    }
}

impl Pending<ExplorerPosition> for ExplorerStream {
    fn poll(&mut self) -> Polled<ExplorerPosition> {
        let mut latest = None;
        while let Some(event) = self.try_next() {
            match event {
                ExplorerEvent::Partial(answer) => latest = Some(answer),
                ExplorerEvent::Finished => {
                    return match self.latest().cloned() {
                        Some(answer) => Polled::Done(answer),
                        None => Polled::Failed(ProviderError::NoData),
                    };
                }
                // O último parcial continua válido quando o fluxo cai no meio.
                ExplorerEvent::Failed(error) => {
                    return match self.latest().cloned() {
                        Some(answer) => Polled::Done(answer),
                        None => Polled::Failed(error),
                    };
                }
            }
        }
        match latest {
            Some(answer) => Polled::Partial(answer),
            None => Polled::Waiting,
        }
    }

    fn cancel(&mut self) {
        self.close();
    }
}

/// Situação de uma consulta.
#[derive(Debug, Clone, PartialEq)]
pub enum LookupState<T> {
    /// Nada pedido (ainda).
    Idle,
    /// Pedido a caminho; `partial` é o melhor resultado parcial, se o fluxo já mandou um.
    Loading {
        partial: Option<Answer<T>>,
    },
    Done(Answer<T>),
    Failed(ProviderError),
}

/// Uma consulta em andamento ou concluída, para a chave `key`.
pub struct Lookup<T> {
    key: Option<String>,
    /// Em `Mutex` só para o `Lookup` ser `Sync` (recurso do Bevy); nunca há disputa.
    pending: Option<Mutex<Box<dyn Pending<T>>>>,
    state: LookupState<T>,
    sent: usize,
}

impl<T> Default for Lookup<T> {
    fn default() -> Self {
        Self {
            key: None,
            pending: None,
            state: LookupState::Idle,
            sent: 0,
        }
    }
}

impl<T: Clone> Lookup<T> {
    fn cancel_pending(&mut self) {
        if let Some(pending) = self.pending.take() {
            pending
                .into_inner()
                .unwrap_or_else(PoisonError::into_inner)
                .cancel();
        }
    }

    /// Cancela o pedido atual (se houver) e volta a não ter nada.
    pub fn clear(&mut self) {
        self.cancel_pending();
        self.key = None;
        self.state = LookupState::Idle;
    }

    /// Começa a consulta de `key`, cancelando a anterior.
    pub fn start(&mut self, key: String, pending: impl Pending<T> + 'static) {
        self.clear();
        self.key = Some(key);
        self.pending = Some(Mutex::new(Box::new(pending)));
        self.state = LookupState::Loading { partial: None };
        self.sent += 1;
    }

    /// Recolhe o que já chegou (nunca bloqueia).
    pub fn poll(&mut self) {
        let Some(pending) = self.pending.as_mut() else {
            return;
        };
        match pending
            .get_mut()
            .unwrap_or_else(PoisonError::into_inner)
            .poll()
        {
            Polled::Waiting => {}
            Polled::Partial(answer) => {
                self.state = LookupState::Loading {
                    partial: Some(answer),
                };
            }
            Polled::Done(answer) => {
                self.pending = None;
                self.state = LookupState::Done(answer);
            }
            Polled::Failed(error) => {
                self.pending = None;
                self.state = LookupState::Failed(error);
            }
        }
    }

    pub fn key(&self) -> Option<&str> {
        self.key.as_deref()
    }

    pub fn state(&self) -> &LookupState<T> {
        &self.state
    }

    /// Quantos pedidos esta consulta já iniciou (diagnóstico e testes).
    #[cfg(test)]
    pub fn sent(&self) -> usize {
        self.sent
    }
}

impl<T> Drop for Lookup<T> {
    fn drop(&mut self) {
        if let Some(pending) = self.pending.take() {
            pending
                .into_inner()
                .unwrap_or_else(PoisonError::into_inner)
                .cancel();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_key_fires_once_after_the_settle_delay() {
        let t0 = Instant::now();
        let mut settle = Settle::default();
        assert!(!settle.poll(&"e4", t0));
        assert!(settle.is_settling());
        assert!(!settle.poll(&"e4", t0 + Duration::from_millis(299)));
        assert!(settle.poll(&"e4", t0 + Duration::from_millis(300)));
        // Uma vez só: os quadros seguintes com a mesma chave não repetem o pedido.
        assert!(!settle.poll(&"e4", t0 + Duration::from_millis(900)));
        assert!(!settle.is_settling());
    }

    #[test]
    fn a_key_that_keeps_changing_never_fires() {
        let t0 = Instant::now();
        let mut settle = Settle::default();
        for (i, key) in ["a", "b", "c", "d", "e", "f"].into_iter().enumerate() {
            let at = t0 + Duration::from_millis(100 * i as u64);
            assert!(!settle.poll(&key, at), "{key}");
        }
        // Parou em "f": 300 ms depois da última mudança, dispara.
        assert!(settle.poll(&"f", t0 + Duration::from_millis(500 + 300)));
    }

    #[test]
    fn remaining_counts_down_and_reset_allows_the_same_key_again() {
        let t0 = Instant::now();
        let mut settle = Settle::default();
        settle.poll(&1, t0);
        assert_eq!(
            settle.remaining(t0 + Duration::from_millis(100)),
            Some(Duration::from_millis(200))
        );
        assert!(settle.poll(&1, t0 + SETTLE_DELAY));
        assert_eq!(settle.remaining(t0 + SETTLE_DELAY), None);
        settle.reset();
        assert!(!settle.poll(&1, t0 + SETTLE_DELAY));
        assert!(settle.poll(&1, t0 + SETTLE_DELAY * 2));
    }
}
