//! Fluxo NDJSON que se reconecta sozinho: backoff exponencial com jitter pelo relógio
//! injetado (nada de laço de reconexão) e o intervalo sem dados registrado.

use chess_net::{Clock, Net, NetError, Request, Stream, StreamEvent, Timestamp};
use std::fmt;
use std::sync::Arc;
use std::time::Duration;

/// Espera entre reconexões: exponencial a partir de `base`, limitada a `max`, com "equal
/// jitter" (entre metade e o total), como a camada de rede faz nas repetições.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Backoff {
    pub base: Duration,
    pub max: Duration,
}

impl Default for Backoff {
    fn default() -> Self {
        Self {
            base: Duration::from_secs(1),
            max: Duration::from_secs(60),
        }
    }
}

impl Backoff {
    /// Espera antes da reconexão `retry` (0 = a primeira).
    pub(crate) fn delay(&self, retry: u32, rng: &mut Jitter) -> Duration {
        let factor = 1u32.checked_shl(retry.min(20)).unwrap_or(u32::MAX);
        let full = self.base.saturating_mul(factor).min(self.max);
        let full_ms = u64::try_from(full.as_millis()).unwrap_or(u64::MAX);
        let half = full_ms / 2;
        let span = full_ms - half;
        let jitter = if span == 0 {
            0
        } else {
            rng.next() % (span + 1)
        };
        Duration::from_millis(half + jitter)
    }
}

/// Gerador pequeno e determinístico (SplitMix64) para o jitter.
#[derive(Debug, Clone)]
pub(crate) struct Jitter(u64);

impl Jitter {
    pub fn new(seed: u64) -> Self {
        Self(seed)
    }

    pub fn next(&mut self) -> u64 {
        self.0 = self.0.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut z = self.0;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        z ^ (z >> 31)
    }
}

/// Estado do fluxo, para a interface.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LinkStatus {
    /// Aberto, à espera da primeira mensagem.
    Connecting,
    /// Recebendo.
    Live,
    /// Caiu; a tentativa `attempt` abre em `at` (relógio injetado).
    Retrying { attempt: u32, at: Timestamp },
    /// Encerrado (Partida terminada ou erro que não passa sozinho).
    Closed,
}

/// O que chegou do fluxo numa consulta.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum LinkItem {
    /// O fluxo voltou depois de cair: o intervalo sem dados.
    Restored {
        from: Timestamp,
        to: Timestamp,
    },
    Line(String),
    /// O Provedor encerrou o fluxo.
    Ended,
    Failed(NetError),
}

pub(crate) struct Link {
    net: Net,
    clock: Arc<dyn Clock>,
    request: Request,
    stream: Option<Stream>,
    status: LinkStatus,
    retries: u32,
    backoff: Backoff,
    rng: Jitter,
    down_since: Option<Timestamp>,
}

impl fmt::Debug for Link {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Link")
            .field("status", &self.status)
            .field("retries", &self.retries)
            .finish_non_exhaustive()
    }
}

impl Link {
    /// Abre o fluxo na hora.
    pub fn open(
        net: Net,
        clock: Arc<dyn Clock>,
        request: Request,
        backoff: Backoff,
        seed: u64,
    ) -> Self {
        let mut link = Self {
            net,
            clock,
            request,
            stream: None,
            status: LinkStatus::Connecting,
            retries: 0,
            backoff,
            rng: Jitter::new(seed),
            down_since: None,
        };
        link.connect();
        link
    }

    fn connect(&mut self) {
        self.stream = Some(self.net.stream(self.request.clone()));
        self.status = LinkStatus::Connecting;
    }

    pub fn status(&self) -> LinkStatus {
        self.status
    }

    /// Nunca bloqueia: reabre se o prazo da reconexão passou e entrega o que chegou.
    pub fn poll(&mut self) -> Vec<LinkItem> {
        let mut items = Vec::new();
        if let LinkStatus::Retrying { at, .. } = self.status
            && self.clock.now() >= at
        {
            self.connect();
        }
        let Some(stream) = &self.stream else {
            return items;
        };
        while let Some(event) = stream.try_next() {
            match event {
                StreamEvent::Line(line) => {
                    if self.status != LinkStatus::Live {
                        self.status = LinkStatus::Live;
                        self.retries = 0;
                        if let Some(from) = self.down_since.take() {
                            items.push(LinkItem::Restored {
                                from,
                                to: self.clock.now(),
                            });
                        }
                    }
                    items.push(LinkItem::Line(line));
                }
                StreamEvent::Ended => {
                    self.stream = None;
                    items.push(LinkItem::Ended);
                    break;
                }
                StreamEvent::Failed(error) => {
                    self.stream = None;
                    items.push(LinkItem::Failed(error));
                    break;
                }
            }
        }
        items
    }

    /// O fluxo caiu: tenta de novo depois do backoff (e nunca antes de `not_before`, o
    /// prazo de um `429`).
    pub fn retry(&mut self, not_before: Option<Timestamp>) {
        self.stream = None;
        let now = self.clock.now();
        self.down_since.get_or_insert(now);
        let delay = self.backoff.delay(self.retries, &mut self.rng);
        self.retries = self.retries.saturating_add(1);
        let at = now.plus(delay);
        let at = not_before.map_or(at, |n| n.max(at));
        self.status = LinkStatus::Retrying {
            attempt: self.retries,
            at,
        };
    }

    /// Reabre já (para consultar o estado completo); não conta como queda.
    pub fn reconnect_now(&mut self) {
        if self.status != LinkStatus::Closed {
            self.connect();
        }
    }

    pub fn close(&mut self) {
        if let Some(stream) = self.stream.take() {
            stream.close();
        }
        self.status = LinkStatus::Closed;
    }
}

/// Erros que não passam sozinhos: reconectar seria laço.
pub(crate) fn is_fatal(error: &NetError) -> bool {
    match error {
        NetError::Http { status, .. } => (400..500).contains(status) && *status != 429,
        NetError::RateLimited { .. }
        | NetError::Timeout { .. }
        | NetError::Unreachable { .. }
        | NetError::Cancelled
        | NetError::BodyTooLarge { .. } => false,
        _ => true,
    }
}

/// O prazo pedido pelo Provedor, se o erro é um `429`.
pub(crate) fn not_before(error: &NetError) -> Option<Timestamp> {
    match error {
        NetError::RateLimited { until, .. } => Some(*until),
        _ => None,
    }
}

impl Drop for Link {
    fn drop(&mut self) {
        if let Some(stream) = self.stream.take() {
            stream.close();
        }
    }
}
