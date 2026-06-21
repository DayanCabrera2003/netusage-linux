//! Capa de datos: consulta el consumo persistido y lo mapea al modelo de
//! presentación.
//!
//! Frontera con la Fase 4: se usa el camino primario (SQLite en modo solo
//! lectura, `Store::open_readonly`) en vez del socket IPC. Es síncrono y sin
//! privilegios (la base es `0644`). Ver desviaciones; si se quisiera el socket,
//! solo cambiaría este archivo.

use std::path::PathBuf;

use chrono::Utc;
use netusage_store::{Period as StorePeriod, Store};

use crate::error::Result;
use crate::model::{AppUsage, PeriodSummary};
use crate::period::Period;
use crate::sort::sort_by_usage;

/// Fuente de datos de la TUI: la base del demonio abierta en solo lectura.
pub struct DataSource {
    db_path: PathBuf,
}

impl DataSource {
    /// Crea la fuente apuntando a la base en `db_path`.
    pub fn new(db_path: PathBuf) -> Self {
        Self { db_path }
    }

    /// Ruta de la base que consulta esta fuente (para diagnosticar fallos).
    pub fn db_path(&self) -> &std::path::Path {
        &self.db_path
    }

    /// Consulta el resumen del `period`: total y desglose por app, ya ordenado.
    pub fn fetch(&self, period: Period) -> Result<PeriodSummary> {
        let store = Store::open_readonly(&self.db_path)?;
        let now = Utc::now();
        let store_period = map_period(period);

        let total = store.usage_total(store_period, now)?;
        let mut apps: Vec<AppUsage> = store
            .usage_by_app(store_period, now)?
            .into_iter()
            .map(|app| AppUsage {
                app_key: app.app_key,
                display_name: app.display_name,
                rx_bytes: app.rx_bytes.max(0) as u64,
                tx_bytes: app.tx_bytes.max(0) as u64,
            })
            .collect();
        sort_by_usage(&mut apps);

        Ok(PeriodSummary {
            period,
            total_rx: total.rx_bytes.max(0) as u64,
            total_tx: total.tx_bytes.max(0) as u64,
            apps,
        })
    }
}

/// Traduce el periodo de la TUI al del store.
fn map_period(period: Period) -> StorePeriod {
    match period {
        Period::Today => StorePeriod::Today,
        Period::Week => StorePeriod::ThisWeek,
        Period::Month => StorePeriod::ThisMonth,
        Period::LastMonth => StorePeriod::LastMonth,
    }
}
