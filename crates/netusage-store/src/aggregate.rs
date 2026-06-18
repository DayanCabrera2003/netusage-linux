//! Queries de agregación temporal: total y por aplicación en un periodo.
//!
//! Responsabilidad única: combinar los rangos de `period.rs` con SQL sobre
//! `samples` y `daily`. La suma cubre ambas fuentes (muestras finas recientes +
//! agregados diarios del histórico) con `UNION ALL`, para no perder ni duplicar
//! tras la compactación de retención.

use chrono::{DateTime, Utc};
use rusqlite::params;

use crate::error::Result;
use crate::period::{self, Period};
use crate::store::Store;

/// Total de bytes de un periodo.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct UsageTotal {
    pub rx_bytes: i64,
    pub tx_bytes: i64,
}

/// Uso de una aplicación en un periodo.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AppUsage {
    pub app_key: String,
    pub display_name: String,
    pub rx_bytes: i64,
    pub tx_bytes: i64,
}

impl Store {
    /// Total rx/tx de la máquina en el periodo.
    pub fn usage_total(&self, period: Period, now: DateTime<Utc>) -> Result<UsageTotal> {
        let cfg = self.load_config()?;
        let b = period::bounds(period, &cfg, now)?;
        let (rx_bytes, tx_bytes) = self.conn.query_row(
            "SELECT COALESCE(SUM(rx_bytes), 0), COALESCE(SUM(tx_bytes), 0) FROM (
                 SELECT rx_bytes, tx_bytes FROM samples WHERE ts >= ?1 AND ts < ?2
                 UNION ALL
                 SELECT rx_bytes, tx_bytes FROM daily WHERE day >= ?1 AND day < ?2
             )",
            params![b.start, b.end],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )?;
        Ok(UsageTotal { rx_bytes, tx_bytes })
    }

    /// Uso por aplicación en el periodo, ordenado por consumo total descendente.
    pub fn usage_by_app(&self, period: Period, now: DateTime<Utc>) -> Result<Vec<AppUsage>> {
        let cfg = self.load_config()?;
        let b = period::bounds(period, &cfg, now)?;
        let mut stmt = self.conn.prepare(
            "SELECT a.app_key, a.display_name,
                    COALESCE(SUM(s.rx), 0), COALESCE(SUM(s.tx), 0)
             FROM (
                 SELECT app_id, rx_bytes AS rx, tx_bytes AS tx FROM samples
                     WHERE ts >= ?1 AND ts < ?2
                 UNION ALL
                 SELECT app_id, rx_bytes AS rx, tx_bytes AS tx FROM daily
                     WHERE day >= ?1 AND day < ?2
             ) s
             JOIN apps a ON a.id = s.app_id
             GROUP BY s.app_id
             ORDER BY (SUM(s.rx) + SUM(s.tx)) DESC, a.display_name ASC",
        )?;
        let rows = stmt.query_map(params![b.start, b.end], |row| {
            Ok(AppUsage {
                app_key: row.get(0)?,
                display_name: row.get(1)?,
                rx_bytes: row.get(2)?,
                tx_bytes: row.get(3)?,
            })
        })?;
        let mut out = Vec::new();
        for row in rows {
            out.push(row?);
        }
        Ok(out)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::samples::SampleDelta;
    use chrono::TimeZone;
    use chrono_tz::Tz;

    /// Crea un store UTC con dos apps y devuelve sus ids.
    fn seeded() -> (Store, i64, i64) {
        let mut store = Store::open_in_memory().unwrap();
        // Config UTC explícita (la por defecto ya es UTC).
        let firefox = store.upsert_app("/firefox", "firefox", 0).unwrap();
        let chromium = store.upsert_app("/chromium", "chromium", 0).unwrap();
        (store, firefox, chromium)
    }

    fn ts(y: i32, m: u32, d: u32, h: u32) -> i64 {
        Tz::UTC
            .with_ymd_and_hms(y, m, d, h, 0, 0)
            .unwrap()
            .timestamp()
    }

    fn now_at(y: i32, m: u32, d: u32, h: u32) -> DateTime<Utc> {
        Tz::UTC
            .with_ymd_and_hms(y, m, d, h, 0, 0)
            .unwrap()
            .with_timezone(&Utc)
    }

    #[test]
    fn today_sums_only_todays_samples() {
        let (mut store, ff, _cr) = seeded();
        // Muestra de hoy y de ayer (UTC).
        store
            .insert_samples(
                ts(2026, 3, 11, 10),
                &[SampleDelta {
                    app_id: ff,
                    rx_bytes: 100,
                    tx_bytes: 10,
                }],
            )
            .unwrap();
        store
            .insert_samples(
                ts(2026, 3, 10, 10),
                &[SampleDelta {
                    app_id: ff,
                    rx_bytes: 999,
                    tx_bytes: 99,
                }],
            )
            .unwrap();

        let total = store
            .usage_total(Period::Today, now_at(2026, 3, 11, 12))
            .unwrap();
        assert_eq!(
            total,
            UsageTotal {
                rx_bytes: 100,
                tx_bytes: 10
            }
        );
    }

    #[test]
    fn by_app_orders_by_consumption() {
        let (mut store, ff, cr) = seeded();
        store
            .insert_samples(
                ts(2026, 3, 11, 9),
                &[
                    SampleDelta {
                        app_id: ff,
                        rx_bytes: 50,
                        tx_bytes: 0,
                    },
                    SampleDelta {
                        app_id: cr,
                        rx_bytes: 500,
                        tx_bytes: 0,
                    },
                ],
            )
            .unwrap();
        let list = store
            .usage_by_app(Period::ThisMonth, now_at(2026, 3, 11, 12))
            .unwrap();
        assert_eq!(list.len(), 2);
        assert_eq!(list[0].display_name, "chromium"); // más consumo arriba
        assert_eq!(list[1].rx_bytes, 50);
    }

    #[test]
    fn union_samples_and_daily_without_loss_or_dup() {
        let (mut store, ff, _cr) = seeded();
        // Parte como muestra fina y parte como daily, ambas dentro del mes.
        store
            .insert_samples(
                ts(2026, 3, 11, 9),
                &[SampleDelta {
                    app_id: ff,
                    rx_bytes: 30,
                    tx_bytes: 3,
                }],
            )
            .unwrap();
        store
            .conn
            .execute(
                "INSERT INTO daily(app_id, day, rx_bytes, tx_bytes) VALUES (?1, ?2, 70, 7)",
                params![ff, ts(2026, 3, 5, 0)],
            )
            .unwrap();
        let total = store
            .usage_total(Period::ThisMonth, now_at(2026, 3, 11, 12))
            .unwrap();
        assert_eq!(
            total,
            UsageTotal {
                rx_bytes: 100,
                tx_bytes: 10
            }
        );
    }
}
