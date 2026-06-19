//! Reductor: aplica un mensaje al estado de la aplicación.
//!
//! Centraliza todas las transiciones para que sean testeables sin terminal.

use crate::model::PeriodSummary;
use crate::state::{AppState, ConnState};

/// Mensaje que provoca una transición de estado.
#[derive(Debug, Clone)]
pub enum Message {
    /// Salir de la aplicación.
    Quit,
    /// Pasar al siguiente periodo.
    NextPeriod,
    /// Pasar al periodo anterior.
    PrevPeriod,
    /// Bajar en la lista de apps.
    SelectNext,
    /// Subir en la lista de apps.
    SelectPrev,
    /// Alternar la vista de detalle de la app seleccionada.
    ToggleDetail,
    /// Cerrar la vista de detalle.
    CloseDetail,
    /// Forzar un refresco de datos.
    Refresh,
    /// Tick de polling.
    Tick,
    /// Datos del periodo cargados.
    DataLoaded(PeriodSummary),
    /// La carga de datos falló, con el motivo.
    DataFailed(String),
}

/// Aplica `msg` a `state`.
///
/// Cambiar de periodo o forzar refresco pone la conexión en `Loading` (el bucle
/// disparará un `fetch`). `DataLoaded`/`DataFailed` resuelven ese estado.
pub fn update(state: &mut AppState, msg: Message) {
    match msg {
        Message::Quit => state.should_quit = true,
        Message::NextPeriod => {
            state.period = state.period.next();
            state.connection = ConnState::Loading;
        }
        Message::PrevPeriod => {
            state.period = state.period.prev();
            state.connection = ConnState::Loading;
        }
        Message::SelectNext => state.select_next(),
        Message::SelectPrev => state.select_prev(),
        Message::ToggleDetail => state.show_detail = !state.show_detail,
        Message::CloseDetail => state.show_detail = false,
        Message::Refresh => state.connection = ConnState::Loading,
        // El tick no cambia el estado por sí mismo; el bucle decide si refresca.
        Message::Tick => {}
        Message::DataLoaded(summary) => {
            state.set_summary(summary);
            state.connection = ConnState::Ready;
        }
        Message::DataFailed(reason) => {
            state.connection = ConnState::Disconnected(reason);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::PeriodSummary;
    use crate::period::Period;

    fn empty_summary(period: Period) -> PeriodSummary {
        PeriodSummary {
            period,
            total_rx: 0,
            total_tx: 0,
            apps: vec![],
        }
    }

    #[test]
    fn next_period_cycles_and_sets_loading() {
        let mut state = AppState::new(Period::Today);
        update(&mut state, Message::NextPeriod);
        assert_eq!(state.period, Period::Week);
        assert_eq!(state.connection, ConnState::Loading);
    }

    #[test]
    fn data_loaded_sets_ready() {
        let mut state = AppState::new(Period::Today);
        update(&mut state, Message::DataLoaded(empty_summary(Period::Today)));
        assert_eq!(state.connection, ConnState::Ready);
        assert!(state.summary.is_some());
    }

    #[test]
    fn data_failed_sets_disconnected() {
        let mut state = AppState::new(Period::Today);
        update(&mut state, Message::DataFailed("sin base".into()));
        assert_eq!(
            state.connection,
            ConnState::Disconnected("sin base".to_string())
        );
    }

    #[test]
    fn quit_marks_should_quit() {
        let mut state = AppState::new(Period::Today);
        update(&mut state, Message::Quit);
        assert!(state.should_quit);
    }
}
