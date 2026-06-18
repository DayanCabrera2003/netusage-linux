//! Persistencia periódica de las muestras por aplicación.
//!
//! Responsabilidad única: en cada ciclo de muestreo, convertir los totales
//! acumulados por app (del agregador de la Fase 2) en deltas y guardarlos en el
//! `Store`. Lanza la retención con baja frecuencia.
//!
//! Nota de diseño (desviación del plan): el plan describía un hilo `sampler`
//! propio. Como el `supervisor` de la Fase 2 ya tiene el bucle de muestreo y
//! produce la lista por app cada intervalo, el `Sampler` es un colaborador al
//! que el supervisor llama una vez por ciclo (`tick`), no un hilo aparte. Evita
//! duplicar el bucle y leer los mapas dos veces.

use anyhow::{Context, Result};
use chrono::{DateTime, Utc};
use netusage_store::{SampleDelta, Store, StoreConfig};

use crate::aggregator::AppUsage;
use crate::counters::DeltaTracker;

/// Cada cuánto, como mínimo, se ejecuta la retención (una vez al día).
const RETENTION_INTERVAL_SECS: i64 = 24 * 3600;

/// Persiste las muestras por app en cada ciclo.
pub struct Sampler {
    store: Store,
    config: StoreConfig,
    deltas: DeltaTracker,
    last_retention_ts: i64,
}

impl Sampler {
    /// Crea el sampler sobre un `Store` ya abierto y su configuración.
    pub fn new(store: Store, config: StoreConfig) -> Self {
        Self {
            store,
            config,
            deltas: DeltaTracker::new(),
            last_retention_ts: 0,
        }
    }

    /// Persiste un ciclo: por cada app, delta respecto al absoluto anterior, y un
    /// `insert_samples` en lote. Dispara la retención si toca.
    pub fn tick(&mut self, usages: &[AppUsage], now: DateTime<Utc>) -> Result<()> {
        let now_ts = now.timestamp();
        let mut deltas = Vec::with_capacity(usages.len());
        for usage in usages {
            let app_id = self
                .store
                .upsert_app(&usage.app_key, &usage.display_name, now_ts)
                .context("upsert de app")?;
            let (rx, tx) = self.deltas.delta(&usage.app_key, usage.rx, usage.tx);
            deltas.push(SampleDelta {
                app_id,
                rx_bytes: rx as i64,
                tx_bytes: tx as i64,
            });
        }
        self.store
            .insert_samples(now_ts, &deltas)
            .context("insertando muestras")?;

        if now_ts - self.last_retention_ts >= RETENTION_INTERVAL_SECS {
            self.store
                .run_retention(&self.config, now)
                .context("ejecutando retención")?;
            self.last_retention_ts = now_ts;
        }
        Ok(())
    }
}
