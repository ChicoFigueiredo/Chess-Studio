//! Limites por Provedor: token bucket (na forma GCRA, equivalente e só com inteiros),
//! `Retry-After` (segundos ou data HTTP) e backoff exponencial com jitter.

use crate::catalog::RateLimit;
use crate::clock::{Timestamp, millis};
use std::time::Duration;

/// Pausa quando o `429` não diz quanto esperar (Lichess e PlayStrategy pedem um minuto).
pub(crate) const DEFAULT_RETRY_AFTER: Duration = Duration::from_secs(60);

/// Token bucket por Provedor: rajada de `burst`, depois um pedido a cada `every`.
#[derive(Debug, Clone)]
pub(crate) struct TokenBucket {
    interval_ms: i64,
    tolerance_ms: i64,
    /// Instante teórico da próxima chegada (GCRA).
    tat: Option<i64>,
}

impl TokenBucket {
    pub fn new(limit: RateLimit) -> Self {
        let interval_ms = millis(limit.every).max(1);
        let burst = i64::from(limit.burst.max(1));
        Self {
            interval_ms,
            tolerance_ms: interval_ms.saturating_mul(burst - 1),
            tat: None,
        }
    }

    /// Reserva uma vaga e devolve o instante em que o pedido pode sair (`now` se já pode).
    pub fn reserve(&mut self, now: Timestamp) -> Timestamp {
        let now = now.as_millis();
        let tat = self.tat.map_or(now, |t| t.max(now));
        let allowed_at = (tat - self.tolerance_ms).max(now);
        self.tat = Some(tat + self.interval_ms);
        Timestamp::from_millis(allowed_at)
    }
}

/// Quantas vezes e com que espera repetir falhas temporárias (5xx, conexão interrompida).
/// Só para `GET`; erros do cliente (4xx) nunca se repetem.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RetryPolicy {
    pub max_retries: u32,
    pub base_delay: Duration,
    pub max_delay: Duration,
}

impl Default for RetryPolicy {
    fn default() -> Self {
        Self {
            max_retries: 2,
            base_delay: Duration::from_millis(500),
            max_delay: Duration::from_secs(30),
        }
    }
}

impl RetryPolicy {
    /// Espera antes da repetição `attempt` (0 = primeira repetição): exponencial com
    /// "equal jitter", entre metade e o total de `base * 2^attempt` (limitado a `max_delay`).
    pub(crate) fn delay(&self, attempt: u32, rng: &mut SplitMix64) -> Duration {
        let factor = 1u32.checked_shl(attempt.min(20)).unwrap_or(u32::MAX);
        let full = self.base_delay.saturating_mul(factor).min(self.max_delay);
        let full_ms = u64::try_from(full.as_millis()).unwrap_or(u64::MAX);
        let half = full_ms / 2;
        let jitter = if full_ms - half == 0 {
            0
        } else {
            rng.next() % (full_ms - half + 1)
        };
        Duration::from_millis(half + jitter)
    }
}

/// Gerador pequeno e determinístico (semente na configuração; testes reprodutíveis).
#[derive(Debug, Clone)]
pub(crate) struct SplitMix64(u64);

impl SplitMix64 {
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

/// `Retry-After` em segundos (`120`) ou data HTTP (`Wed, 11 Nov 2026 12:02:00 GMT`).
pub(crate) fn retry_after(value: &str, now: Timestamp) -> Option<Timestamp> {
    let value = value.trim();
    if let Ok(secs) = value.parse::<u64>() {
        return Some(now.plus(Duration::from_secs(secs.min(24 * 3600))));
    }
    http_date(value).map(|at| at.max(now))
}

/// IMF-fixdate (RFC 9110): `Sun, 06 Nov 1994 08:49:37 GMT`.
fn http_date(value: &str) -> Option<Timestamp> {
    let (_, rest) = value.split_once(", ")?;
    let mut parts = rest.split(' ');
    let day: i64 = parts.next()?.parse().ok()?;
    let month = match parts.next()? {
        "Jan" => 1,
        "Feb" => 2,
        "Mar" => 3,
        "Apr" => 4,
        "May" => 5,
        "Jun" => 6,
        "Jul" => 7,
        "Aug" => 8,
        "Sep" => 9,
        "Oct" => 10,
        "Nov" => 11,
        "Dec" => 12,
        _ => return None,
    };
    let year: i64 = parts.next()?.parse().ok()?;
    let mut hms = parts.next()?.split(':');
    let h: i64 = hms.next()?.parse().ok()?;
    let m: i64 = hms.next()?.parse().ok()?;
    let s: i64 = hms.next()?.parse().ok()?;
    if parts.next()? != "GMT" || !(1..=31).contains(&day) || h > 23 || m > 59 || s > 60 {
        return None;
    }
    let days = days_from_civil(year, month, day);
    Some(Timestamp::from_millis(
        ((days * 24 + h) * 60 + m) * 60_000 + s * 1000,
    ))
}

/// Dias desde 1970-01-01 (algoritmo de Howard Hinnant).
fn days_from_civil(year: i64, month: i64, day: i64) -> i64 {
    let y = if month <= 2 { year - 1 } else { year };
    let era = y.div_euclid(400);
    let yoe = y - era * 400;
    let mp = (month + 9) % 12;
    let doy = (153 * mp + 2) / 5 + day - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    era * 146_097 + doe - 719_468
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn http_dates_match_the_unix_epoch() {
        assert_eq!(
            http_date("Thu, 01 Jan 1970 00:00:00 GMT"),
            Some(Timestamp::from_millis(0))
        );
        assert_eq!(
            http_date("Sun, 06 Nov 1994 08:49:37 GMT"),
            Some(Timestamp::from_millis(784_111_777_000))
        );
        assert_eq!(http_date("ontem"), None);
    }

    #[test]
    fn a_past_date_means_now() {
        let now = Timestamp::from_millis(1_000_000_000_000);
        assert_eq!(retry_after("Thu, 01 Jan 1970 00:00:00 GMT", now), Some(now));
    }

    #[test]
    fn jittered_delays_stay_in_their_window() {
        let policy = RetryPolicy {
            max_retries: 5,
            base_delay: Duration::from_secs(1),
            max_delay: Duration::from_secs(4),
        };
        let mut rng = SplitMix64::new(1);
        for _ in 0..200 {
            let d0 = policy.delay(0, &mut rng);
            assert!((Duration::from_millis(500)..=Duration::from_secs(1)).contains(&d0));
            let d5 = policy.delay(5, &mut rng);
            assert!((Duration::from_secs(2)..=Duration::from_secs(4)).contains(&d5));
        }
    }
}
