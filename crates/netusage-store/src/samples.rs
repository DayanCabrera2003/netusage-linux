//! Inserción de muestras (deltas) en la tabla `samples`.
//!
//! Responsabilidad única: persistir en lote los deltas de un instante de
//! muestreo, dentro de una sola transacción (un único fsync por ciclo, no uno
//! por app).

use rusqlite::params;

use crate::error::Result;
use crate::store::Store;

/// Delta de bytes de una app en un intervalo de muestreo.
///
/// No son contadores absolutos: ya vienen desreseteados del demonio, así que
/// `rx_bytes`/`tx_bytes` son siempre `>= 0`.
#[derive(Debug, Clone, Copy)]
pub struct SampleDelta {
    pub app_id: i64,
    pub rx_bytes: i64,
    pub tx_bytes: i64,
}

impl Store {
    /// Inserta los `deltas` con timestamp `ts` (epoch segundos UTC) en una sola
    /// transacción.
    ///
    /// Omite filas con rx y tx a cero para no inflar la tabla. Si ya existe una
    /// fila `(app_id, ts)` (dos muestras en el mismo segundo), se suman los
    /// bytes (idempotencia/acumulación).
    pub fn insert_samples(&mut self, ts: i64, deltas: &[SampleDelta]) -> Result<()> {
        if deltas.is_empty() {
            return Ok(());
        }
        let tx = self.conn.transaction()?;
        {
            let mut stmt = tx.prepare_cached(
                "INSERT INTO samples(app_id, ts, rx_bytes, tx_bytes) VALUES (?1, ?2, ?3, ?4)
                 ON CONFLICT(app_id, ts) DO UPDATE SET
                     rx_bytes = rx_bytes + excluded.rx_bytes,
                     tx_bytes = tx_bytes + excluded.tx_bytes",
            )?;
            for delta in deltas {
                if delta.rx_bytes == 0 && delta.tx_bytes == 0 {
                    continue;
                }
                stmt.execute(params![delta.app_id, ts, delta.rx_bytes, delta.tx_bytes])?;
            }
        }
        tx.commit()?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::SampleDelta;
    use crate::store::Store;

    fn seed_app(store: &mut Store) -> i64 {
        store.upsert_app("/usr/bin/app", "app", 0).unwrap()
    }

    #[test]
    fn inserts_deltas_and_skips_zeros() {
        let mut store = Store::open_in_memory().unwrap();
        let id = seed_app(&mut store);
        store
            .insert_samples(
                100,
                &[
                    SampleDelta {
                        app_id: id,
                        rx_bytes: 10,
                        tx_bytes: 1,
                    },
                    SampleDelta {
                        app_id: id,
                        rx_bytes: 0,
                        tx_bytes: 0,
                    }, // omitida
                ],
            )
            .unwrap();
        store
            .insert_samples(
                102,
                &[SampleDelta {
                    app_id: id,
                    rx_bytes: 5,
                    tx_bytes: 2,
                }],
            )
            .unwrap();

        let count: i64 = store
            .conn
            .query_row("SELECT count(*) FROM samples", [], |r| r.get(0))
            .unwrap();
        assert_eq!(count, 2);

        let sum_rx: i64 = store
            .conn
            .query_row("SELECT SUM(rx_bytes) FROM samples", [], |r| r.get(0))
            .unwrap();
        assert_eq!(sum_rx, 15);
    }

    #[test]
    fn same_app_and_ts_accumulates() {
        let mut store = Store::open_in_memory().unwrap();
        let id = seed_app(&mut store);
        store
            .insert_samples(
                100,
                &[SampleDelta {
                    app_id: id,
                    rx_bytes: 10,
                    tx_bytes: 0,
                }],
            )
            .unwrap();
        store
            .insert_samples(
                100,
                &[SampleDelta {
                    app_id: id,
                    rx_bytes: 7,
                    tx_bytes: 0,
                }],
            )
            .unwrap();
        let rx: i64 = store
            .conn
            .query_row("SELECT rx_bytes FROM samples WHERE ts=100", [], |r| {
                r.get(0)
            })
            .unwrap();
        assert_eq!(rx, 17);
    }
}
