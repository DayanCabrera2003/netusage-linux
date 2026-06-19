//! Tipos de presentación, independientes de ratatui y de la capa de datos.
//!
//! Son una base neutra reutilizable por una futura GUI (fuera del MVP).

use crate::period::Period;

/// Uso de una aplicación en un periodo.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AppUsage {
    pub app_key: String,
    pub display_name: String,
    pub rx_bytes: u64,
    pub tx_bytes: u64,
}

impl AppUsage {
    /// Bytes totales (rx + tx).
    pub fn total(&self) -> u64 {
        self.rx_bytes.saturating_add(self.tx_bytes)
    }
}

/// Resumen de un periodo: total de la máquina y desglose por app.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PeriodSummary {
    pub period: Period,
    pub total_rx: u64,
    pub total_tx: u64,
    pub apps: Vec<AppUsage>,
}

impl PeriodSummary {
    /// Bytes totales del periodo (rx + tx).
    pub fn total(&self) -> u64 {
        self.total_rx.saturating_add(self.total_tx)
    }
}
