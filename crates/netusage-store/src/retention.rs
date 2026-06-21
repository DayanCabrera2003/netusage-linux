//! Retención: compactación de muestras finas a agregados diarios y purga.
//!
//! Responsabilidad única: mantener acotada la tabla `samples` agregando las
//! muestras antiguas por día local en `daily`, y purgar el histórico diario más
//! viejo que la retención configurada. Todo en una transacción.

use std::collections::HashMap;

use chrono::{DateTime, Utc};
use rusqlite::params;

use crate::config::StoreConfig;
use crate::error::Result;
use crate::period;
use crate::store::Store;

/// Segundos en un día (la retención se expresa en días).
const DAY_SECS: i64 = 86_400;

impl Store {
    /// Compacta las muestras anteriores a la retención fina en `daily` y purga el
    /// `daily` más viejo que la retención diaria.
    pub fn run_retention(&mut self, cfg: &StoreConfig, now: DateTime<Utc>) -> Result<()> {
        let now_ts = now.timestamp();
        let fine_cutoff = now_ts - cfg.fine_retention_days as i64 * DAY_SECS;
        // Corte de purga: inicio del ciclo del mes anterior. Todo lo previo ya no
        // se muestra en ningun periodo de la UI (hoy/semana/mes/mes anterior).
        let purge_cutoff = crate::period::bounds(crate::period::Period::LastMonth, cfg, now)?.start;

        // Agregar en memoria las muestras viejas por (app_id, día local).
        let mut rollup: HashMap<(i64, i64), (i64, i64)> = HashMap::new();
        {
            let mut stmt = self
                .conn
                .prepare("SELECT app_id, ts, rx_bytes, tx_bytes FROM samples WHERE ts < ?1")?;
            let rows = stmt.query_map([fine_cutoff], |row| {
                Ok((
                    row.get::<_, i64>(0)?,
                    row.get::<_, i64>(1)?,
                    row.get::<_, i64>(2)?,
                    row.get::<_, i64>(3)?,
                ))
            })?;
            for row in rows {
                let (app_id, ts, rx, tx) = row?;
                let day = period::day_start_epoch(cfg, ts)?;
                let entry = rollup.entry((app_id, day)).or_insert((0, 0));
                entry.0 += rx;
                entry.1 += tx;
            }
        }

        let tx = self.conn.transaction()?;
        {
            let mut upsert = tx.prepare_cached(
                "INSERT INTO daily(app_id, day, rx_bytes, tx_bytes) VALUES (?1, ?2, ?3, ?4)
                 ON CONFLICT(app_id, day) DO UPDATE SET
                     rx_bytes = rx_bytes + excluded.rx_bytes,
                     tx_bytes = tx_bytes + excluded.tx_bytes",
            )?;
            for ((app_id, day), (rx, tx_bytes)) in &rollup {
                upsert.execute(params![app_id, day, rx, tx_bytes])?;
            }
        }
        // Orden: compactacion (arriba) y luego purga, en la misma transaccion.
        tx.execute("DELETE FROM samples WHERE ts < ?1", [fine_cutoff])?;
        // Por si fine_retention_days fuese mayor que ~2 meses: barre tambien las
        // muestras finas previas al corte que no se compactaron.
        tx.execute("DELETE FROM samples WHERE ts < ?1", [purge_cutoff])?;
        tx.execute("DELETE FROM daily WHERE day < ?1", [purge_cutoff])?;
        tx.commit()?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use crate::config::StoreConfig;
    use crate::samples::SampleDelta;
    use crate::store::Store;
    use chrono::{DateTime, Utc};

    const DAY: i64 = 86_400;

    fn now() -> DateTime<Utc> {
        DateTime::from_timestamp(1_770_000_000, 0).unwrap()
    }

    #[test]
    fn old_samples_roll_up_recent_stay_and_old_daily_purged() {
        let mut store = Store::open_in_memory().unwrap();
        let cfg = StoreConfig {
            fine_retention_days: 14,
            ..StoreConfig::default()
        };
        let app = store.upsert_app("/app", "app", 0).unwrap();
        let now_ts = now().timestamp();

        // Muestra vieja (hace 20 días) y reciente (hace 1 día).
        let old_ts = now_ts - 20 * DAY;
        let recent_ts = now_ts - DAY;
        store
            .insert_samples(
                old_ts,
                &[SampleDelta {
                    app_id: app,
                    rx_bytes: 100,
                    tx_bytes: 10,
                }],
            )
            .unwrap();
        store
            .insert_samples(
                recent_ts,
                &[SampleDelta {
                    app_id: app,
                    rx_bytes: 5,
                    tx_bytes: 1,
                }],
            )
            .unwrap();
        // Un agregado diario muy viejo (hace 800 días) que debe purgarse.
        store
            .conn
            .execute(
                "INSERT INTO daily(app_id, day, rx_bytes, tx_bytes) VALUES (?1, ?2, 1, 1)",
                rusqlite::params![app, now_ts - 800 * DAY],
            )
            .unwrap();

        store.run_retention(&cfg, now()).unwrap();

        // La muestra vieja se fue de samples; la reciente sigue.
        let samples: i64 = store
            .conn
            .query_row("SELECT count(*) FROM samples", [], |r| r.get(0))
            .unwrap();
        assert_eq!(samples, 1);

        // La vieja está agregada en daily con sus bytes.
        let daily_rx: i64 = store
            .conn
            .query_row(
                "SELECT COALESCE(SUM(rx_bytes),0) FROM daily WHERE rx_bytes >= 100",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(daily_rx, 100);

        // El daily de hace 800 días se purgó.
        let purged: i64 = store
            .conn
            .query_row("SELECT count(*) FROM daily WHERE rx_bytes = 1", [], |r| {
                r.get(0)
            })
            .unwrap();
        assert_eq!(purged, 0);
    }

    #[test]
    fn purges_before_last_month_keeps_this_and_last_month() {
        use chrono::TimeZone;
        let mut store = Store::open_in_memory().unwrap();
        let cfg = StoreConfig {
            cycle_start_day: 1,
            timezone: "UTC".to_string(),
            ..StoreConfig::default()
        };
        let app = store.upsert_app("/app", "app", 0).unwrap();
        // now = 2026-03-15 12:00 UTC -> mes actual desde 1 mar, mes anterior desde 1 feb.
        let now = Utc.with_ymd_and_hms(2026, 3, 15, 12, 0, 0).unwrap();
        let day = |y, m, d| Utc.with_ymd_and_hms(y, m, d, 0, 0, 0).unwrap().timestamp();
        // Tres agregados diarios: este mes, mes anterior, y dos meses atras.
        for (d, rx) in [
            (day(2026, 3, 5), 1i64),
            (day(2026, 2, 10), 2),
            (day(2026, 1, 10), 3),
        ] {
            store
                .conn
                .execute(
                    "INSERT INTO daily(app_id, day, rx_bytes, tx_bytes) VALUES (?1, ?2, ?3, 0)",
                    rusqlite::params![app, d, rx],
                )
                .unwrap();
        }

        store.run_retention(&cfg, now).unwrap();

        // El de enero (anterior al 1 feb) se purga; febrero y marzo quedan.
        let remaining: Vec<i64> = {
            let mut stmt = store
                .conn
                .prepare("SELECT rx_bytes FROM daily ORDER BY rx_bytes")
                .unwrap();
            let rows = stmt.query_map([], |r| r.get::<_, i64>(0)).unwrap();
            rows.map(|r| r.unwrap()).collect()
        };
        assert_eq!(remaining, vec![1, 2]); // marzo (1) y febrero (2); enero (3) purgado
    }

    #[test]
    fn keeps_current_week_when_it_started_in_the_previous_month() {
        use chrono::TimeZone;
        let mut store = Store::open_in_memory().unwrap();
        let cfg = StoreConfig {
            cycle_start_day: 1,
            week_start: crate::config::WeekStart::Monday,
            timezone: "UTC".to_string(),
            ..StoreConfig::default()
        };
        let app = store.upsert_app("/app", "app", 0).unwrap();
        // now = miercoles 2026-04-01; con inicio de semana en lunes, la semana en
        // curso empezo el 2026-03-30 (mes anterior por calendario). El corte de
        // purga (1 mar, inicio del mes anterior) NO debe tocar esa semana.
        let now = Utc.with_ymd_and_hms(2026, 4, 1, 12, 0, 0).unwrap();
        let ts_week_start = Utc
            .with_ymd_and_hms(2026, 3, 30, 10, 0, 0)
            .unwrap()
            .timestamp();
        store
            .insert_samples(
                ts_week_start,
                &[crate::samples::SampleDelta {
                    app_id: app,
                    rx_bytes: 7,
                    tx_bytes: 0,
                }],
            )
            .unwrap();
        // Diario de dos meses atras (anterior al corte 1 mar): debe purgarse.
        let day_old = Utc
            .with_ymd_and_hms(2026, 2, 15, 0, 0, 0)
            .unwrap()
            .timestamp();
        store
            .conn
            .execute(
                "INSERT INTO daily(app_id, day, rx_bytes, tx_bytes) VALUES (?1, ?2, 9, 0)",
                rusqlite::params![app, day_old],
            )
            .unwrap();

        store.run_retention(&cfg, now).unwrap();

        // La muestra de la semana en curso sobrevive; el diario viejo se purga.
        let week_sample: i64 = store
            .conn
            .query_row("SELECT count(*) FROM samples WHERE rx_bytes = 7", [], |r| {
                r.get(0)
            })
            .unwrap();
        assert_eq!(week_sample, 1);
        let old_daily: i64 = store
            .conn
            .query_row("SELECT count(*) FROM daily WHERE rx_bytes = 9", [], |r| {
                r.get(0)
            })
            .unwrap();
        assert_eq!(old_daily, 0);
    }
}
