//! Cálculo puro de límites de periodo (hoy, semana, mes, mes anterior).
//!
//! Responsabilidad única: dado `StoreConfig` y un instante de referencia UTC,
//! devolver el rango `[start, end)` en epoch segundos UTC que delimita el
//! periodo. Sin SQL. Toda la aritmética de calendario (zona horaria, DST, ciclo
//! de facturación, meses cortos) se resuelve aquí con `chrono` + `chrono-tz`.
//!
//! Convención de DST: el inicio de un día local se toma como su **primer
//! instante real**. En el cambio de otoño (medianoche ambigua) se elige la
//! primera ocurrencia; en el de primavera (medianoche inexistente, p. ej.
//! America/Santiago) se toma el instante del salto. Esto hace los rangos
//! deterministas y de duración correcta (días de 23 o 25 horas).

use chrono::{DateTime, Datelike, Duration, NaiveDate, Utc};
use chrono_tz::Tz;

use crate::config::StoreConfig;
use crate::error::Result;

/// Periodo consultable.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Period {
    Today,
    ThisWeek,
    ThisMonth,
    LastMonth,
}

/// Rango `[start, end)` en epoch segundos UTC.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PeriodBounds {
    pub start: i64,
    pub end: i64,
}

/// Calcula el rango del `period` para la configuración y el instante dados.
pub fn bounds(period: Period, cfg: &StoreConfig, now_utc: DateTime<Utc>) -> Result<PeriodBounds> {
    let tz = cfg.tz()?;
    let today = now_utc.with_timezone(&tz).date_naive();

    let (start_date, end_date) = match period {
        Period::Today => (today, today.succ_opt().unwrap()),
        Period::ThisWeek => {
            let start = week_start_date(today, cfg);
            (start, start + Duration::days(7))
        }
        Period::ThisMonth => month_cycle(today, cfg),
        Period::LastMonth => {
            let (this_start, _) = month_cycle(today, cfg);
            let (sy, sm) = (this_start.year(), this_start.month());
            let (py, pm) = prev_month(sy, sm);
            (cycle_date(py, pm, cfg.cycle_start_day), this_start)
        }
    };

    Ok(PeriodBounds {
        start: local_start_of_day_epoch(&tz, start_date),
        end: local_start_of_day_epoch(&tz, end_date),
    })
}

/// Epoch UTC del inicio del día local que contiene al instante `ts` (epoch
/// segundos UTC). Lo usa la retención para agrupar muestras por día local.
pub(crate) fn day_start_epoch(cfg: &StoreConfig, ts: i64) -> Result<i64> {
    let tz = cfg.tz()?;
    let date = DateTime::from_timestamp(ts, 0)
        .expect("epoch válido")
        .with_timezone(&tz)
        .date_naive();
    Ok(local_start_of_day_epoch(&tz, date))
}

/// Fecha del inicio de la semana que contiene a `today`, según `week_start`.
fn week_start_date(today: NaiveDate, cfg: &StoreConfig) -> NaiveDate {
    let from_monday = |d: chrono::Weekday| d.num_days_from_monday();
    let offset = (from_monday(today.weekday()) + 7 - from_monday(cfg.week_start.weekday())) % 7;
    today - Duration::days(offset as i64)
}

/// Calcula `(inicio, fin)` del ciclo de facturación que contiene a `today`.
fn month_cycle(today: NaiveDate, cfg: &StoreConfig) -> (NaiveDate, NaiveDate) {
    let (y, m) = (today.year(), today.month());
    let this = cycle_date(y, m, cfg.cycle_start_day);
    let start = if today >= this {
        this
    } else {
        let (py, pm) = prev_month(y, m);
        cycle_date(py, pm, cfg.cycle_start_day)
    };
    let (sy, sm) = (start.year(), start.month());
    let (ny, nm) = next_month(sy, sm);
    let end = cycle_date(ny, nm, cfg.cycle_start_day);
    (start, end)
}

/// Fecha del arranque del ciclo en un mes dado, ajustando el día al último día
/// del mes si `cycle_day` lo excede (p. ej. día 31 en febrero -> 28/29).
fn cycle_date(year: i32, month: u32, cycle_day: u8) -> NaiveDate {
    let day = (cycle_day as u32).min(days_in_month(year, month));
    NaiveDate::from_ymd_opt(year, month, day).expect("día de ciclo válido tras el ajuste")
}

/// Número de días del mes.
fn days_in_month(year: i32, month: u32) -> u32 {
    let (ny, nm) = next_month(year, month);
    let first_this = NaiveDate::from_ymd_opt(year, month, 1).unwrap();
    let first_next = NaiveDate::from_ymd_opt(ny, nm, 1).unwrap();
    (first_next - first_this).num_days() as u32
}

/// Mes anterior a `(year, month)`.
fn prev_month(year: i32, month: u32) -> (i32, u32) {
    if month == 1 {
        (year - 1, 12)
    } else {
        (year, month - 1)
    }
}

/// Mes siguiente a `(year, month)`.
fn next_month(year: i32, month: u32) -> (i32, u32) {
    if month == 12 {
        (year + 1, 1)
    } else {
        (year, month + 1)
    }
}

/// Epoch UTC del primer instante real del día local `date`.
///
/// Para un día normal es la medianoche local. Si la medianoche no existe (salto
/// de primavera a las 00:00) se toma el primer instante válido del día; si es
/// ambigua (otoño) se toma la primera ocurrencia. Itera por horas para cubrir
/// cualquier hora a la que ocurra la transición.
fn local_start_of_day_epoch(tz: &Tz, date: NaiveDate) -> i64 {
    use chrono::TimeZone;
    for hour in 0..24 {
        let naive = date.and_hms_opt(hour, 0, 0).unwrap();
        if let Some(dt) = tz.from_local_datetime(&naive).earliest() {
            return dt.timestamp();
        }
    }
    // Inalcanzable: algún instante del día siempre existe.
    unreachable!("ningún instante válido en el día {date}")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::WeekStart;
    use chrono::TimeZone;
    use chrono_tz::Tz;

    fn cfg(cycle: u8, week: WeekStart, tz: &str) -> StoreConfig {
        StoreConfig {
            cycle_start_day: cycle,
            week_start: week,
            timezone: tz.to_string(),
            ..StoreConfig::default()
        }
    }

    /// Epoch UTC de una medianoche local, para comparar fronteras esperadas.
    fn local_midnight(tz: Tz, y: i32, m: u32, d: u32) -> i64 {
        tz.with_ymd_and_hms(y, m, d, 0, 0, 0).unwrap().timestamp()
    }

    #[test]
    fn this_month_cycle_day_1() {
        let madrid: Tz = "Europe/Madrid".parse().unwrap();
        let now = madrid
            .with_ymd_and_hms(2026, 3, 10, 12, 0, 0)
            .unwrap()
            .with_timezone(&Utc);
        let b = bounds(
            Period::ThisMonth,
            &cfg(1, WeekStart::Monday, "Europe/Madrid"),
            now,
        )
        .unwrap();
        assert_eq!(b.start, local_midnight(madrid, 2026, 3, 1));
        assert_eq!(b.end, local_midnight(madrid, 2026, 4, 1));

        let last = bounds(
            Period::LastMonth,
            &cfg(1, WeekStart::Monday, "Europe/Madrid"),
            now,
        )
        .unwrap();
        assert_eq!(last.start, local_midnight(madrid, 2026, 2, 1));
        assert_eq!(last.end, local_midnight(madrid, 2026, 3, 1));
    }

    #[test]
    fn this_month_cycle_day_15_before_cycle() {
        let madrid: Tz = "Europe/Madrid".parse().unwrap();
        // 10 de marzo es anterior al día 15: el "mes" empezó el 15 de febrero.
        let now = madrid
            .with_ymd_and_hms(2026, 3, 10, 12, 0, 0)
            .unwrap()
            .with_timezone(&Utc);
        let b = bounds(
            Period::ThisMonth,
            &cfg(15, WeekStart::Monday, "Europe/Madrid"),
            now,
        )
        .unwrap();
        assert_eq!(b.start, local_midnight(madrid, 2026, 2, 15));
        assert_eq!(b.end, local_midnight(madrid, 2026, 3, 15));

        let last = bounds(
            Period::LastMonth,
            &cfg(15, WeekStart::Monday, "Europe/Madrid"),
            now,
        )
        .unwrap();
        assert_eq!(last.start, local_midnight(madrid, 2026, 1, 15));
        assert_eq!(last.end, local_midnight(madrid, 2026, 2, 15));
    }

    #[test]
    fn cycle_day_31_clamps_in_february() {
        // 2026 no es bisiesto: febrero tiene 28 días.
        assert_eq!(
            cycle_date(2026, 2, 31),
            NaiveDate::from_ymd_opt(2026, 2, 28).unwrap()
        );
        // 2024 es bisiesto: 29.
        assert_eq!(
            cycle_date(2024, 2, 31),
            NaiveDate::from_ymd_opt(2024, 2, 29).unwrap()
        );
    }

    #[test]
    fn week_start_monday_vs_sunday() {
        let utc_cfg = |w| cfg(1, w, "UTC");
        let utc: Tz = "UTC".parse().unwrap();
        // 2026-03-11 es miércoles.
        let now = utc
            .with_ymd_and_hms(2026, 3, 11, 12, 0, 0)
            .unwrap()
            .with_timezone(&Utc);
        let mon = bounds(Period::ThisWeek, &utc_cfg(WeekStart::Monday), now).unwrap();
        assert_eq!(mon.start, local_midnight(utc, 2026, 3, 9)); // lunes
        let sun = bounds(Period::ThisWeek, &utc_cfg(WeekStart::Sunday), now).unwrap();
        assert_eq!(sun.start, local_midnight(utc, 2026, 3, 8)); // domingo
    }

    #[test]
    fn today_is_23h_on_spring_dst_madrid() {
        let madrid: Tz = "Europe/Madrid".parse().unwrap();
        // 2026-03-29: salto de primavera (02:00 -> 03:00); el día dura 23 h.
        let now = madrid
            .with_ymd_and_hms(2026, 3, 29, 12, 0, 0)
            .unwrap()
            .with_timezone(&Utc);
        let b = bounds(
            Period::Today,
            &cfg(1, WeekStart::Monday, "Europe/Madrid"),
            now,
        )
        .unwrap();
        assert_eq!(b.end - b.start, 23 * 3600);
    }

    #[test]
    fn today_is_25h_on_autumn_dst_madrid() {
        let madrid: Tz = "Europe/Madrid".parse().unwrap();
        // 2026-10-25: salto de otoño (03:00 -> 02:00); el día dura 25 h.
        let now = madrid
            .with_ymd_and_hms(2026, 10, 25, 12, 0, 0)
            .unwrap()
            .with_timezone(&Utc);
        let b = bounds(
            Period::Today,
            &cfg(1, WeekStart::Monday, "Europe/Madrid"),
            now,
        )
        .unwrap();
        assert_eq!(b.end - b.start, 25 * 3600);
    }

    #[test]
    fn southern_hemisphere_dst_santiago_today_is_valid_range() {
        // America/Santiago cambia de hora alrededor de medianoche; el rango del
        // día debe ser válido (start < end) y no nulo.
        let now = "America/Santiago"
            .parse::<Tz>()
            .unwrap()
            .with_ymd_and_hms(2026, 9, 7, 12, 0, 0)
            .unwrap()
            .with_timezone(&Utc);
        let b = bounds(
            Period::Today,
            &cfg(1, WeekStart::Monday, "America/Santiago"),
            now,
        )
        .unwrap();
        assert!(b.start < b.end);
        assert!(b.end - b.start >= 23 * 3600);
    }
}
