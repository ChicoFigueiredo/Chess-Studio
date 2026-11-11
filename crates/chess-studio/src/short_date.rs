//! Data curta `DD/MM/AAAA` de um instante (UTC). O app não lê o fuso do sistema, então a data
//! só aparece onde um dia a mais ou a menos não engana: rótulos de importações e consultas
//! antigas.

/// `(ano, mês, dia)` de um número de dias desde 1970-01-01 (calendário gregoriano proléptico).
fn civil(days: i64) -> (i64, i64, i64) {
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1_460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let day = doy - (153 * mp + 2) / 5 + 1;
    let month = if mp < 10 { mp + 3 } else { mp - 9 };
    (yoe + era * 400 + i64::from(month <= 2), month, day)
}

/// `DD/MM/AAAA` (dia em UTC) de um instante em milissegundos desde a época Unix.
pub(crate) fn date_text(ms: i64) -> String {
    let (y, m, d) = civil(ms.div_euclid(1000).div_euclid(86_400));
    format!("{d:02}/{m:02}/{y:04}")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn instants_become_day_month_year() {
        assert_eq!(date_text(0), "01/01/1970");
        assert_eq!(date_text(1_794_398_400_000), "11/11/2026");
        assert_eq!(date_text(1_709_164_800_000), "29/02/2024");
        assert_eq!(date_text(-86_400_000), "31/12/1969");
    }
}
