//! Relógio injetado. A lógica de limites, cache e Origem só conhece instantes vindos daqui;
//! nos testes o [`FakeClock`] faz "esperar" avançar o tempo na hora.

use serde::{Deserialize, Serialize};
use std::fmt;
use std::sync::atomic::{AtomicBool, AtomicI64, Ordering};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

/// Instante de parede em milissegundos desde 1970-01-01 UTC (como os `*_at_ms` do
/// `chess-store`). Persistível: a idade do cache sobrevive ao reinício do app.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct Timestamp(i64);

impl Timestamp {
    pub const fn from_millis(ms: i64) -> Self {
        Self(ms)
    }

    pub const fn as_millis(self) -> i64 {
        self.0
    }

    pub fn plus(self, duration: Duration) -> Self {
        Self(self.0.saturating_add(millis(duration)))
    }

    /// Tempo decorrido desde `earlier`; zero se `earlier` está no futuro (relógio voltou).
    pub fn since(self, earlier: Timestamp) -> Duration {
        Duration::from_millis(u64::try_from(self.0.saturating_sub(earlier.0)).unwrap_or(0))
    }
}

pub(crate) fn millis(duration: Duration) -> i64 {
    i64::try_from(duration.as_millis()).unwrap_or(i64::MAX)
}

/// Fonte de tempo da camada de rede.
pub trait Clock: Send + Sync + fmt::Debug {
    fn now(&self) -> Timestamp;

    /// Espera até `deadline`. Devolve `false` se `cancel` foi acionado antes (o pedido foi
    /// cancelado e não deve seguir).
    fn sleep_until(&self, deadline: Timestamp, cancel: &AtomicBool) -> bool;
}

/// Relógio do sistema; a espera é fatiada para atender ao cancelamento.
#[derive(Debug, Clone, Copy, Default)]
pub struct SystemClock;

impl Clock for SystemClock {
    fn now(&self) -> Timestamp {
        let ms = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_or(0, |d| i64::try_from(d.as_millis()).unwrap_or(i64::MAX));
        Timestamp(ms)
    }

    fn sleep_until(&self, deadline: Timestamp, cancel: &AtomicBool) -> bool {
        const SLICE: Duration = Duration::from_millis(25);
        loop {
            if cancel.load(Ordering::Acquire) {
                return false;
            }
            let left = deadline.since(self.now());
            if left.is_zero() {
                return true;
            }
            std::thread::sleep(left.min(SLICE));
        }
    }
}

/// Relógio de teste: parado até alguém avançá-lo; esperar só avança o ponteiro.
#[derive(Debug)]
pub struct FakeClock {
    now: AtomicI64,
}

impl FakeClock {
    pub fn new(start: Timestamp) -> Self {
        Self {
            now: AtomicI64::new(start.0),
        }
    }

    pub fn advance(&self, by: Duration) {
        self.now.fetch_add(millis(by), Ordering::SeqCst);
    }

    pub fn set(&self, to: Timestamp) {
        self.now.store(to.0, Ordering::SeqCst);
    }
}

impl Clock for FakeClock {
    fn now(&self) -> Timestamp {
        Timestamp(self.now.load(Ordering::SeqCst))
    }

    fn sleep_until(&self, deadline: Timestamp, cancel: &AtomicBool) -> bool {
        if cancel.load(Ordering::Acquire) {
            return false;
        }
        self.now.fetch_max(deadline.0, Ordering::SeqCst);
        true
    }
}
