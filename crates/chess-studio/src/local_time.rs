//! Hora local do computador, num lugar só (#239). A Central, o Explorador e a Biblioteca
//! mostram "às 14:32", "ontem às 09:10" ou "em 09/11 às 21:00" em vez de "há N min" e de
//! horários em UTC.
//!
//! O fuso vem do sistema (`jiff`, Rust puro, sem `unsafe` no projeto). Se o sistema não o
//! informa, [`LocalClock::when`] devolve `None` e quem chama cai no relativo ("há 3 min"). Nos
//! testes o fuso é injetado: [`LocalClock::fixed`] e [`testing::with`].

use crate::i18n::I18n;
use jiff::tz::TimeZone;
use jiff::{Timestamp, Zoned};
use std::sync::OnceLock;

/// O fuso em uso, ou nenhum.
#[derive(Clone, Debug)]
pub struct LocalClock {
    zone: Option<TimeZone>,
}

impl LocalClock {
    /// O fuso do sistema; sem ele (contêiner sem `/etc/localtime`, por exemplo), nenhum.
    pub fn system() -> Self {
        Self {
            zone: TimeZone::try_system().ok(),
        }
    }

    /// Fuso fixo, em minutos a leste de UTC (Brasília: -180). Para os testes.
    #[cfg(test)]
    pub fn fixed(offset_minutes: i32) -> Self {
        let zone = jiff::tz::Offset::from_seconds(offset_minutes * 60)
            .ok()
            .map(TimeZone::fixed);
        Self { zone }
    }

    /// Sem fuso: tudo cai no relativo.
    #[cfg(test)]
    pub fn unknown() -> Self {
        Self { zone: None }
    }

    /// O relógio que as telas usam: o do sistema (lido uma vez) ou, nos testes, o injetado.
    pub fn current() -> Self {
        #[cfg(test)]
        if let Some(injected) = testing::injected() {
            return injected;
        }
        static SYSTEM: OnceLock<LocalClock> = OnceLock::new();
        SYSTEM.get_or_init(LocalClock::system).clone()
    }

    #[cfg(test)]
    pub fn is_known(&self) -> bool {
        self.zone.is_some()
    }

    fn zoned(&self, at_ms: i64) -> Option<Zoned> {
        let zone = self.zone.clone()?;
        let instant = Timestamp::from_millisecond(at_ms).ok()?;
        Some(instant.to_zoned(zone))
    }

    /// `14:32` na hora local.
    pub fn clock(&self, at_ms: i64) -> Option<String> {
        let z = self.zoned(at_ms)?;
        Some(format!("{:02}:{:02}", z.hour(), z.minute()))
    }

    /// `09/11` na hora local.
    pub fn day_month(&self, at_ms: i64) -> Option<String> {
        let z = self.zoned(at_ms)?;
        Some(format!("{:02}/{:02}", z.day(), z.month()))
    }

    /// `09/11/2026` na hora local.
    pub fn date(&self, at_ms: i64) -> Option<String> {
        let z = self.zoned(at_ms)?;
        Some(format!("{:02}/{:02}/{:04}", z.day(), z.month(), z.year()))
    }

    /// "às 14:32", "ontem às 09:10", "amanhã às 00:10" ou "em 09/11 às 21:00" (com o ano, se
    /// não for o atual), na hora local. `None` sem fuso: quem chama usa o relativo.
    pub fn when(&self, i18n: &I18n, now_ms: i64, at_ms: i64) -> Option<String> {
        let now = self.zoned(now_ms)?;
        let at = self.zoned(at_ms)?;
        let time = format!("{:02}:{:02}", at.hour(), at.minute());
        let day = at.date();
        let today = now.date();
        let key = if day == today {
            "time-today"
        } else if today.yesterday().is_ok_and(|d| d == day) {
            "time-yesterday"
        } else if today.tomorrow().is_ok_and(|d| d == day) {
            "time-tomorrow"
        } else {
            let date = if day.year() == today.year() {
                format!("{:02}/{:02}", day.day(), day.month())
            } else {
                format!("{:02}/{:02}/{:04}", day.day(), day.month(), day.year())
            };
            return Some(i18n.t_with("time-other-day", &[("date", &date), ("time", &time)]));
        };
        Some(i18n.t_with(key, &[("time", &time)]))
    }
}

#[cfg(test)]
pub mod testing {
    //! Fuso injetado por thread: cada teste roda na sua, então não há disputa.
    use super::LocalClock;
    use std::cell::RefCell;

    thread_local! {
        static INJECTED: RefCell<Option<LocalClock>> = const { RefCell::new(None) };
    }

    pub fn injected() -> Option<LocalClock> {
        INJECTED.with(|slot| slot.borrow().clone())
    }

    /// Roda `body` com `clock` no lugar do fuso do sistema.
    pub fn with<T>(clock: LocalClock, body: impl FnOnce() -> T) -> T {
        INJECTED.with(|slot| *slot.borrow_mut() = Some(clock));
        let out = body();
        INJECTED.with(|slot| *slot.borrow_mut() = None);
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 2026-11-11 15:30:00 UTC (12:30 em Brasília).
    const MID_DAY: i64 = 1_794_411_000_000;
    const MINUTE: i64 = 60_000;
    const HOUR: i64 = 60 * MINUTE;
    const DAY: i64 = 24 * HOUR;

    fn brasilia() -> LocalClock {
        LocalClock::fixed(-180)
    }

    #[test]
    fn reads_the_clock_in_the_injected_zone() {
        assert_eq!(brasilia().clock(MID_DAY).as_deref(), Some("12:30"));
        assert_eq!(LocalClock::fixed(0).clock(MID_DAY).as_deref(), Some("15:30"));
        assert_eq!(LocalClock::fixed(330).clock(MID_DAY).as_deref(), Some("21:00"));
    }

    #[test]
    fn the_local_day_can_differ_from_the_utc_day() {
        // 01:00 UTC de 12/11 ainda é 22:00 de 11/11 em Brasília.
        let at = MID_DAY + 9 * HOUR + 30 * MINUTE;
        assert_eq!(brasilia().day_month(at).as_deref(), Some("11/11"));
        assert_eq!(LocalClock::fixed(0).day_month(at).as_deref(), Some("12/11"));
        assert_eq!(brasilia().date(at).as_deref(), Some("11/11/2026"));
    }

    #[test]
    fn without_a_zone_nothing_is_formatted() {
        let none = LocalClock::unknown();
        assert!(!none.is_known());
        assert_eq!(none.clock(MID_DAY), None);
        assert_eq!(none.day_month(MID_DAY), None);
        assert_eq!(none.date(MID_DAY), None);
        assert_eq!(none.when(&I18n::pt_br(), MID_DAY, MID_DAY), None);
    }

    #[test]
    fn when_says_today_yesterday_tomorrow_and_other_days() {
        let i18n = I18n::pt_br();
        let z = brasilia();
        let say = |at| z.when(&i18n, MID_DAY, at).expect("com fuso");
        assert_eq!(say(MID_DAY - 20 * MINUTE), "às 12:10");
        assert_eq!(say(MID_DAY - DAY), "ontem às 12:30");
        assert_eq!(say(MID_DAY + DAY), "amanhã às 12:30");
        assert_eq!(say(MID_DAY - 3 * DAY), "em 08/11 às 12:30");
        assert_eq!(say(MID_DAY - 400 * DAY), "em 07/10/2025 às 12:30");
    }

    #[test]
    fn today_follows_the_local_midnight_not_the_utc_one() {
        let i18n = I18n::pt_br();
        // Agora: 22:30 de 11/11 em Brasília (01:30 UTC de 12/11). 21:00 local ainda é hoje.
        let now = MID_DAY + 10 * HOUR;
        let nine_pm_local = MID_DAY + 8 * HOUR + 30 * MINUTE;
        assert_eq!(
            brasilia().when(&i18n, now, nine_pm_local).as_deref(),
            Some("às 21:00")
        );
    }

    #[test]
    fn the_system_clock_never_panics() {
        // Com ou sem fuso no ambiente, só não pode falhar.
        let _ = LocalClock::system().clock(MID_DAY);
        let _ = LocalClock::current();
    }

    #[test]
    fn injection_is_scoped_to_the_closure() {
        let seen = testing::with(brasilia(), || LocalClock::current().clock(MID_DAY));
        assert_eq!(seen.as_deref(), Some("12:30"));
        assert!(testing::injected().is_none());
    }
}
