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
        let daily_cutoff = now_ts - cfg.daily_retention_days as i64 * DAY_SECS;

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
        tx.execute("DELETE FROM samples WHERE ts < ?1", [fine_cutoff])?;
        tx.execute("DELETE FROM daily WHERE day < ?1", [daily_cutoff])?;
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
            daily_retention_days: 730,
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
}
