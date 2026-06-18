//! Tipo de error del crate de persistencia.
//!
//! Responsabilidad única: definir `StoreError` y el alias `Result` que usa toda
//! la capa de almacenamiento.

use thiserror::Error;

/// Errores de la capa de persistencia.
#[derive(Debug, Error)]
pub enum StoreError {
    /// Error subyacente de SQLite (apertura, query, transacción).
    #[error("error de SQLite: {0}")]
    Sqlite(#[from] rusqlite::Error),

    /// Error (de)serializando la configuración a/desde JSON.
    #[error("error de configuración (JSON): {0}")]
    Config(#[from] serde_json::Error),

    /// La zona horaria configurada no es un nombre IANA válido.
    #[error("zona horaria desconocida: {0}")]
    UnknownTimezone(String),

    /// Una migración del esquema falló o la versión es incoherente.
    #[error("error de migración del esquema: {0}")]
    Migration(String),
}

/// Alias de `Result` con el error del crate.
pub type Result<T> = std::result::Result<T, StoreError>;
