//! Datas de calendário dos filtros (período). Sem fuso e sem relógio: são só números que o
//! Provedor entende (milissegundos UTC para o Lichess, ano/mês para o Chess.com).

use crate::error::ProviderError;
use std::fmt;

/// Ano e mês (UTC).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct YearMonth {
    pub year: u16,
    pub month: u8,
}

impl YearMonth {
    pub fn new(year: u16, month: u8) -> Result<Self, ProviderError> {
        if (1..=12).contains(&month) && year >= 1970 {
            Ok(Self { year, month })
        } else {
            Err(ProviderError::invalid(format!(
                "mês {year}-{month} inválido"
            )))
        }
    }

    /// Lê `AAAA-MM` ou `AAAA/MM`.
    pub fn parse(text: &str) -> Option<Self> {
        let (y, m) = text.split_once(['-', '/'])?;
        Self::new(y.parse().ok()?, m.parse().ok()?).ok()
    }

    /// O mês seguinte; no último mês representável (dezembro de 65535) devolve o próprio mês
    /// em vez de estourar (SEC-05).
    pub fn next(self) -> Self {
        if self.month == 12 {
            match self.year.checked_add(1) {
                Some(year) => Self { year, month: 1 },
                None => self,
            }
        } else {
            Self {
                year: self.year,
                month: self.month + 1,
            }
        }
    }

    /// Primeiro dia do mês.
    pub fn first_day(self) -> Day {
        Day {
            year: self.year,
            month: self.month,
            day: 1,
        }
    }

    /// Último dia do mês.
    pub fn last_day(self) -> Day {
        Day {
            year: self.year,
            month: self.month,
            day: days_in_month(self.year, self.month),
        }
    }
}

impl fmt::Display for YearMonth {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{:04}-{:02}", self.year, self.month)
    }
}

/// Dia de calendário (UTC).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Day {
    pub year: u16,
    pub month: u8,
    pub day: u8,
}

impl Day {
    pub fn new(year: u16, month: u8, day: u8) -> Result<Self, ProviderError> {
        if year >= 1970
            && (1..=12).contains(&month)
            && (1..=days_in_month(year, month)).contains(&day)
        {
            Ok(Self { year, month, day })
        } else {
            Err(ProviderError::invalid(format!(
                "data {year}-{month}-{day} inválida"
            )))
        }
    }

    /// Lê a tag `Date` do PGN (`2026.11.07`); `??` e datas incompletas dão `None`.
    pub fn parse_pgn(text: &str) -> Option<Self> {
        let mut parts = text.trim().split('.');
        let year = parts.next()?.parse().ok()?;
        let month = parts.next()?.parse().ok()?;
        let day = parts.next()?.parse().ok()?;
        Self::new(year, month, day).ok()
    }

    pub fn year_month(self) -> YearMonth {
        YearMonth {
            year: self.year,
            month: self.month,
        }
    }

    /// Milissegundos desde 1970-01-01T00:00:00Z no início do dia.
    pub fn start_ms(self) -> i64 {
        days_from_civil(
            i64::from(self.year),
            i64::from(self.month),
            i64::from(self.day),
        ) * 86_400_000
    }

    /// Último milissegundo do dia.
    pub fn end_ms(self) -> i64 {
        self.start_ms() + 86_400_000 - 1
    }
}

impl fmt::Display for Day {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{:04}-{:02}-{:02}", self.year, self.month, self.day)
    }
}

fn is_leap(year: u16) -> bool {
    (year.is_multiple_of(4) && !year.is_multiple_of(100)) || year.is_multiple_of(400)
}

fn days_in_month(year: u16, month: u8) -> u8 {
    match month {
        1 | 3 | 5 | 7 | 8 | 10 | 12 => 31,
        4 | 6 | 9 | 11 => 30,
        _ if is_leap(year) => 29,
        _ => 28,
    }
}

/// Dias desde 1970-01-01 (algoritmo de Howard Hinnant).
fn days_from_civil(y: i64, m: i64, d: i64) -> i64 {
    let y = if m <= 2 { y - 1 } else { y };
    let era = y.div_euclid(400);
    let yoe = y - era * 400;
    let mp = (m + 9) % 12;
    let doy = (153 * mp + 2) / 5 + d - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    era * 146_097 + doe - 719_468
}

/// Ano e mês de um instante em milissegundos UTC.
pub(crate) fn year_month_of_ms(ms: i64) -> YearMonth {
    let z = ms.div_euclid(86_400_000) + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = yoe + era * 400 + i64::from(m <= 2);
    YearMonth {
        year: u16::try_from(y).unwrap_or(1970),
        month: u8::try_from(m).unwrap_or(1),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn days_convert_to_unix_milliseconds() {
        let day = Day::new(2026, 11, 11).expect("data");
        assert_eq!(day.start_ms(), 1_794_355_200_000);
        assert_eq!(day.end_ms(), 1_794_355_200_000 + 86_399_999);
        assert_eq!(Day::new(1970, 1, 1).expect("data").start_ms(), 0);
    }

    #[test]
    fn the_month_after_the_last_representable_one_does_not_overflow() {
        let last = YearMonth {
            year: u16::MAX,
            month: 12,
        };
        assert_eq!(last.next(), last);
    }

    #[test]
    fn leap_years_and_bad_dates() {
        assert!(Day::new(2024, 2, 29).is_ok());
        assert!(Day::new(2026, 2, 29).is_err());
        assert!(Day::new(2026, 13, 1).is_err());
        assert_eq!(YearMonth::new(2024, 2).expect("mês").last_day().day, 29);
    }

    #[test]
    fn milliseconds_map_back_to_the_month() {
        assert_eq!(
            year_month_of_ms(1_794_398_400_000),
            YearMonth::new(2026, 11).expect("mês")
        );
        assert_eq!(year_month_of_ms(0), YearMonth::new(1970, 1).expect("mês"));
    }

    #[test]
    fn pgn_dates_and_months_parse() {
        assert_eq!(
            Day::parse_pgn("2026.11.07"),
            Some(Day::new(2026, 11, 7).expect("data"))
        );
        assert_eq!(Day::parse_pgn("2026.??.??"), None);
        assert_eq!(
            YearMonth::parse("2007/07"),
            Some(YearMonth::new(2007, 7).expect("mês"))
        );
        assert_eq!(
            YearMonth::new(2026, 11).expect("mês").to_string(),
            "2026-11"
        );
        assert_eq!(
            YearMonth::new(2026, 12).expect("mês").next().to_string(),
            "2027-01"
        );
    }
}
