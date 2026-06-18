//! Conversión de contadores absolutos por app en deltas para persistir.
//!
//! Responsabilidad única: dado el contador absoluto monótono que la Fase 2
//! acumula por `app_key` (totales del agregador, que solo crecen durante una
//! ejecución y se resetean al reiniciar el demonio), producir el delta del
//! intervalo, manejando el reset.
//!
//! Semántica de la primera lectura: se toma como **línea base** (delta 0), para
//! no atribuir al arranque tráfico acumulado previo. Reset: si el absoluto nuevo
//! es menor que el anterior (el demonio se reinició o el contador volvió a
//! empezar), el delta es el propio absoluto nuevo (el tráfico desde el reset).

use std::collections::HashMap;

/// Mantiene el último absoluto visto por `app_key` para calcular deltas.
pub struct DeltaTracker {
    last: HashMap<String, (u64, u64)>,
}

impl DeltaTracker {
    /// Crea un tracker vacío.
    pub fn new() -> Self {
        Self {
            last: HashMap::new(),
        }
    }

    /// Devuelve el delta `(rx, tx)` del intervalo para `app_key` dados los
    /// absolutos actuales, y actualiza el último valor visto.
    pub fn delta(&mut self, app_key: &str, rx_abs: u64, tx_abs: u64) -> (u64, u64) {
        // Sin valor previo: línea base (prev = actual -> delta 0).
        let (prev_rx, prev_tx) = self
            .last
            .get(app_key)
            .copied()
            .unwrap_or((rx_abs, tx_abs));
        let rx_delta = if rx_abs >= prev_rx {
            rx_abs - prev_rx
        } else {
            rx_abs // reset: el nuevo absoluto es el tráfico desde el reinicio
        };
        let tx_delta = if tx_abs >= prev_tx {
            tx_abs - prev_tx
        } else {
            tx_abs
        };
        self.last.insert(app_key.to_string(), (rx_abs, tx_abs));
        (rx_delta, tx_delta)
    }
}

impl Default for DeltaTracker {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn monotonic_sequence_gives_increments_with_zero_baseline() {
        let mut t = DeltaTracker::new();
        assert_eq!(t.delta("a", 100, 0), (0, 0)); // primera lectura = línea base
        assert_eq!(t.delta("a", 250, 0), (150, 0));
        assert_eq!(t.delta("a", 400, 0), (150, 0));
    }

    #[test]
    fn reset_is_treated_as_traffic_since_restart() {
        let mut t = DeltaTracker::new();
        assert_eq!(t.delta("a", 400, 0), (0, 0)); // base
        // El absoluto baja (reinicio): el delta es el nuevo absoluto, no negativo.
        assert_eq!(t.delta("a", 50, 0), (50, 0));
    }

    #[test]
    fn unseen_app_first_read_is_baseline() {
        let mut t = DeltaTracker::new();
        assert_eq!(t.delta("nueva", 1234, 5678), (0, 0));
    }
}
