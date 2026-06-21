//! Borrado manual de datos por rango de periodo.
//!
//! Responsabilidad unica: borrar de `samples` y `daily` las filas de un periodo
//! `[start, end)`, espejando como las consultas de agregacion leen ambos tramos.

use chrono::{DateTime, Utc};

use crate::error::Result;
use crate::period::{self, Period};
use crate::store::Store;

impl Store {
    /// Borra de `samples` y `daily` todas las filas del `period` indicado,
    /// en el rango `[start, end)` que da `period::bounds`. Devuelve
    /// `(filas_samples, filas_daily)` borradas. Todo en una transaccion minima.
    pub fn delete_period(&mut self, period: Period, now: DateTime<Utc>) -> Result<(usize, usize)> {
        let cfg = self.load_config()?;
        let b = period::bounds(period, &cfg, now)?;
        let tx = self.conn.transaction()?;
        let n_samples = tx.execute(
            "DELETE FROM samples WHERE ts >= ?1 AND ts < ?2",
            rusqlite::params![b.start, b.end],
        )?;
        let n_daily = tx.execute(
            "DELETE FROM daily WHERE day >= ?1 AND day < ?2",
            rusqlite::params![b.start, b.end],
        )?;
        tx.commit()?;
        Ok((n_samples, n_daily))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::TimeZone;

    #[test]
    fn delete_period_removes_only_in_range_in_both_tables() {
        let mut store = Store::open_in_memory().unwrap();
        let app = store.upsert_app("/app", "app", 0).unwrap();
        // Sin config guardada -> default (UTC, ciclo 1). Hoy = 2026-03-15.
        let now = Utc.with_ymd_and_hms(2026, 3, 15, 12, 0, 0).unwrap();
        let ts_today = Utc.with_ymd_and_hms(2026, 3, 15, 9, 0, 0).unwrap().timestamp();
        let ts_yesterday = Utc.with_ymd_and_hms(2026, 3, 14, 9, 0, 0).unwrap().timestamp();
        let day_today = Utc.with_ymd_and_hms(2026, 3, 15, 0, 0, 0).unwrap().timestamp();
        let day_yesterday = Utc.with_ymd_and_hms(2026, 3, 14, 0, 0, 0).unwrap().timestamp();

        store
            .insert_samples(ts_today, &[crate::samples::SampleDelta { app_id: app, rx_bytes: 1, tx_bytes: 1 }])
            .unwrap();
        store
            .insert_samples(ts_yesterday, &[crate::samples::SampleDelta { app_id: app, rx_bytes: 2, tx_bytes: 2 }])
            .unwrap();
        for d in [day_today, day_yesterday] {
            store
                .conn
                .execute(
                    "INSERT INTO daily(app_id, day, rx_bytes, tx_bytes) VALUES (?1, ?2, 1, 1)",
                    rusqlite::params![app, d],
                )
                .unwrap();
        }

        let (s, d) = store.delete_period(Period::Today, now).unwrap();
        assert_eq!((s, d), (1, 1)); // borra solo lo de hoy en cada tabla

        let samples: i64 = store.conn.query_row("SELECT count(*) FROM samples", [], |r| r.get(0)).unwrap();
        let daily: i64 = store.conn.query_row("SELECT count(*) FROM daily", [], |r| r.get(0)).unwrap();
        assert_eq!((samples, daily), (1, 1)); // queda lo de ayer en cada tabla
    }
}
