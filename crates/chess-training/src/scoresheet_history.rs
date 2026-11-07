//! Histórico de erros de Súmula por período (V2-SUM-03).
//!
//! Agrega as conferências gravadas (V2-SUM-02) de um Perfil local: totais por classe e a
//! evolução no tempo. É puro: o "agora" entra por parâmetro (nunca o relógio do sistema) e
//! o recorte é feito pelo instante da conferência.
//!
//! Recortes ([`HistoryPeriod`]): `Week` = últimos 7 dias, `Month` = últimos 30 dias, `All` = tudo,
//! todos terminando em `now_ms` (inclusive). A evolução agrupa por dia UTC (semana e mês) ou
//! por mês UTC (tudo), em ordem cronológica.

use crate::scoresheet_check::{Counts, ScoresheetCheck, Verdict};
use std::collections::BTreeMap;

const DAY_MS: i64 = 86_400_000;

/// Período do histórico.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum HistoryPeriod {
    #[default]
    Week,
    Month,
    All,
}

impl HistoryPeriod {
    pub const ALL: [HistoryPeriod; 3] = [Self::Week, Self::Month, Self::All];

    /// Início do recorte (inclusive) para o "agora" dado; `None` = sem limite inferior.
    pub fn since_ms(self, now_ms: i64) -> Option<i64> {
        match self {
            Self::Week => Some(now_ms.saturating_sub(7 * DAY_MS)),
            Self::Month => Some(now_ms.saturating_sub(30 * DAY_MS)),
            Self::All => None,
        }
    }

    /// Início (UTC) do balde que contém `at_ms`: o dia, ou o mês no período `All`.
    pub fn bucket_start_ms(self, at_ms: i64) -> i64 {
        let day = at_ms.div_euclid(DAY_MS);
        match self {
            Self::Week | Self::Month => day * DAY_MS,
            Self::All => {
                let (y, m, _) = civil_from_days(day);
                days_from_civil(y, m, 1) * DAY_MS
            }
        }
    }
}

/// Dias desde 1970-01-01 -> (ano, mês, dia) no calendário gregoriano proléptico.
fn civil_from_days(z: i64) -> (i64, i64, i64) {
    let z = z + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    (yoe + era * 400 + i64::from(m <= 2), m, d)
}

fn days_from_civil(y: i64, m: i64, d: i64) -> i64 {
    let y = if m <= 2 { y - 1 } else { y };
    let era = y.div_euclid(400);
    let yoe = y.rem_euclid(400);
    let mp = if m > 2 { m - 3 } else { m + 9 };
    let doy = (153 * mp + 2) / 5 + d - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    era * 146_097 + doe - 719_468
}

/// Um ponto da evolução: o que foi conferido num balde de tempo.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct HistoryPoint {
    /// Início (UTC) do balde.
    pub bucket_start_ms: i64,
    /// Súmulas conferidas no balde.
    pub sheets: usize,
    pub counts: Counts,
}

/// Totais e evolução de um Perfil num período.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ScoresheetHistory {
    pub period: HistoryPeriod,
    /// Súmulas conferidas no período.
    pub sheets: usize,
    pub totals: Counts,
    /// Em ordem cronológica.
    pub points: Vec<HistoryPoint>,
}

impl Counts {
    /// Quantas entradas há na classe.
    pub fn of(&self, v: Verdict) -> usize {
        match v {
            Verdict::Correct => self.correct,
            Verdict::AcceptedWithCaveat => self.accepted,
            Verdict::WrongNotation => self.wrong_notation,
            Verdict::Swapped => self.swapped,
            Verdict::Missing => self.missing,
            Verdict::Extra => self.extra,
        }
    }

    /// Soma de todas as classes.
    pub fn total(&self) -> usize {
        Verdict::ALL.into_iter().map(|v| self.of(v)).sum()
    }

    fn merge(&mut self, other: &Counts) {
        self.correct += other.correct;
        self.accepted += other.accepted;
        self.wrong_notation += other.wrong_notation;
        self.swapped += other.swapped;
        self.missing += other.missing;
        self.extra += other.extra;
    }
}

impl ScoresheetHistory {
    /// Agrega conferências `(instante, contagens)`; as fora de `[since, now_ms]` do
    /// período são ignoradas.
    pub fn from_counts(
        period: HistoryPeriod,
        now_ms: i64,
        checks: impl IntoIterator<Item = (i64, Counts)>,
    ) -> Self {
        let since = period.since_ms(now_ms);
        let mut totals = Counts::default();
        let mut sheets = 0;
        let mut buckets: BTreeMap<i64, HistoryPoint> = BTreeMap::new();
        for (at, counts) in checks {
            if at > now_ms || since.is_some_and(|s| at < s) {
                continue;
            }
            sheets += 1;
            totals.merge(&counts);
            let start = period.bucket_start_ms(at);
            let point = buckets.entry(start).or_insert(HistoryPoint {
                bucket_start_ms: start,
                sheets: 0,
                counts: Counts::default(),
            });
            point.sheets += 1;
            point.counts.merge(&counts);
        }
        Self {
            period,
            sheets,
            totals,
            points: buckets.into_values().collect(),
        }
    }

    /// Atalho para conferências já montadas.
    pub fn from_checks<'a>(
        period: HistoryPeriod,
        now_ms: i64,
        checks: impl IntoIterator<Item = &'a ScoresheetCheck>,
    ) -> Self {
        Self::from_counts(
            period,
            now_ms,
            checks.into_iter().map(|c| (c.checked_at_ms(), c.counts())),
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn counts(correct: usize, swapped: usize) -> Counts {
        Counts {
            correct,
            swapped,
            ..Counts::default()
        }
    }

    #[test]
    fn civil_round_trip_and_month_buckets() {
        // 2026-11-07T00:00Z = dia 20 764.
        assert_eq!(civil_from_days(20_764), (2026, 11, 7));
        assert_eq!(days_from_civil(2026, 11, 7), 20_764);
        assert_eq!(civil_from_days(0), (1970, 1, 1));
        assert_eq!(civil_from_days(-1), (1969, 12, 31));
        let nov_1 = days_from_civil(2026, 11, 1) * DAY_MS;
        assert_eq!(
            HistoryPeriod::All.bucket_start_ms(20_764 * DAY_MS + 5),
            nov_1
        );
    }

    #[test]
    fn window_is_inclusive_and_buckets_by_day() {
        let now = 100 * DAY_MS;
        let h = ScoresheetHistory::from_counts(
            HistoryPeriod::Week,
            now,
            [
                (now - 7 * DAY_MS, counts(2, 0)),     // limite inferior: dentro
                (now - 7 * DAY_MS - 1, counts(9, 9)), // fora
                (now - DAY_MS + 5, counts(1, 1)),     // ontem
                (now - DAY_MS + 9, counts(1, 0)),     // ontem (mesmo balde)
                (now + 1, counts(7, 7)),              // futuro: fora
            ],
        );
        assert_eq!(h.sheets, 3);
        assert_eq!(h.totals, counts(4, 1));
        assert_eq!(h.points.len(), 2);
        assert_eq!(h.points[1].sheets, 2);
        assert!(h.points[0].bucket_start_ms < h.points[1].bucket_start_ms);
    }
}
