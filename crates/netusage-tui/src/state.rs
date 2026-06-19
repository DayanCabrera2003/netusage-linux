//! Estado mutable de la interfaz.
//!
//! Lógica de transición pura (sin terminal), testeable.

use crate::model::{AppUsage, PeriodSummary};
use crate::period::Period;

/// Estado de la conexión/carga de datos.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ConnState {
    /// Esperando los datos del periodo.
    Loading,
    /// Datos cargados y listos.
    Ready,
    /// El demonio o la base no están disponibles, con el motivo.
    Disconnected(String),
}

/// Estado de la aplicación TUI.
#[derive(Debug, Clone)]
pub struct AppState {
    pub period: Period,
    pub summary: Option<PeriodSummary>,
    pub selected: usize,
    pub connection: ConnState,
    pub should_quit: bool,
    pub show_detail: bool,
    /// Aviso de modo degradado del entorno, si lo hay (barra superior).
    pub degraded_note: Option<String>,
}

impl AppState {
    /// Crea el estado inicial sobre `period`, en estado de carga.
    pub fn new(period: Period) -> Self {
        Self {
            period,
            summary: None,
            selected: 0,
            connection: ConnState::Loading,
            should_quit: false,
            show_detail: false,
            degraded_note: None,
        }
    }

    /// Número de apps del resumen actual.
    pub fn app_count(&self) -> usize {
        self.summary.as_ref().map(|s| s.apps.len()).unwrap_or(0)
    }

    /// Mueve la selección a la siguiente app, sin salirse del rango.
    pub fn select_next(&mut self) {
        let count = self.app_count();
        if count > 0 && self.selected + 1 < count {
            self.selected += 1;
        }
    }

    /// Mueve la selección a la app anterior, sin bajar de cero.
    pub fn select_prev(&mut self) {
        if self.selected > 0 {
            self.selected -= 1;
        }
    }

    /// Guarda un resumen recién cargado, acotando la selección al nuevo rango.
    pub fn set_summary(&mut self, summary: PeriodSummary) {
        let count = summary.apps.len();
        if count == 0 {
            self.selected = 0;
        } else if self.selected >= count {
            self.selected = count - 1;
        }
        self.summary = Some(summary);
    }

    /// App actualmente seleccionada, si la hay.
    pub fn selected_app(&self) -> Option<&AppUsage> {
        self.summary.as_ref()?.apps.get(self.selected)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::AppUsage;

    fn summary(n: usize) -> PeriodSummary {
        let apps = (0..n)
            .map(|i| AppUsage {
                app_key: format!("/a{i}"),
                display_name: format!("a{i}"),
                rx_bytes: (n - i) as u64 * 10,
                tx_bytes: 0,
            })
            .collect();
        PeriodSummary {
            period: Period::Today,
            total_rx: 0,
            total_tx: 0,
            apps,
        }
    }

    #[test]
    fn selection_is_bounded() {
        let mut state = AppState::new(Period::Today);
        state.set_summary(summary(3));
        state.select_prev(); // ya en 0, no baja
        assert_eq!(state.selected, 0);
        state.select_next();
        state.select_next();
        state.select_next(); // tope en 2
        assert_eq!(state.selected, 2);
    }

    #[test]
    fn set_summary_clamps_selection() {
        let mut state = AppState::new(Period::Today);
        state.set_summary(summary(5));
        state.selected = 4;
        state.set_summary(summary(2)); // ahora solo 2 apps
        assert_eq!(state.selected, 1);
    }

    #[test]
    fn empty_summary_has_no_selected_app() {
        let mut state = AppState::new(Period::Today);
        state.set_summary(summary(0));
        assert!(state.selected_app().is_none());
        assert_eq!(state.app_count(), 0);
    }
}
