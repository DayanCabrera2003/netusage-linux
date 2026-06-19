//! Tipo de error de la TUI.
//!
//! Nota: la capa de datos lee la base SQLite en modo solo lectura (camino
//! primario de la Fase 4), por eso el error de datos envuelve `StoreError` en
//! vez de un error de IPC. Los errores de terminal de crossterm/ratatui son
//! `std::io::Error` y caen en `Io`. Ver desviaciones.

use thiserror::Error;

/// Errores de la interfaz de terminal.
#[derive(Debug, Error)]
pub enum TuiError {
    /// Error al leer los datos del store (base de solo lectura).
    #[error("error de datos: {0}")]
    Data(#[from] netusage_store::StoreError),

    /// Error de E/S de terminal o de entrada/salida en general.
    #[error("error de E/S: {0}")]
    Io(#[from] std::io::Error),
}

/// Alias de `Result` con el error de la TUI.
pub type Result<T> = std::result::Result<T, TuiError>;
