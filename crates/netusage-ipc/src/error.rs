//! Tipo de error del crate IPC.

use thiserror::Error;

/// Errores del protocolo y el transporte IPC.
#[derive(Debug, Error)]
pub enum IpcError {
    /// Error de E/S del socket o del stream.
    #[error("error de E/S: {0}")]
    Io(#[from] std::io::Error),

    /// Error de (de)serialización con `postcard`.
    #[error("error de (de)serialización: {0}")]
    Serde(#[from] postcard::Error),

    /// Mensaje mal formado o que viola el protocolo (p. ej. longitud excesiva).
    #[error("error de protocolo: {0}")]
    Protocol(String),

    /// El cliente y el servidor hablan versiones distintas del protocolo.
    #[error("versión de protocolo incompatible: esperada {expected}, recibida {got}")]
    VersionMismatch { expected: u32, got: u32 },
}
