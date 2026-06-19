//! Tipos del protocolo IPC: peticiones, respuestas, periodos y versión.
//!
//! Todas las operaciones son de **solo lectura**: el `enum Request` no define
//! ninguna escritura, de modo que una UI conectada nunca puede modificar datos.

use serde::{Deserialize, Serialize};

/// Versión del protocolo. El cliente y el servidor deben coincidir.
pub const PROTOCOL_VERSION: u32 = 1;

/// Ruta por defecto del socket Unix del demonio.
pub const DEFAULT_SOCKET_PATH: &str = "/run/netusage/netusaged.sock";

/// Periodo consultable (alineado con las queries de la Fase 3).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Period {
    Today,
    ThisWeek,
    ThisMonth,
    LastMonth,
}

/// Uso de una aplicación en un periodo (cara de wire del IPC).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AppUsage {
    pub app_key: String,
    pub display_name: String,
    pub rx_bytes: i64,
    pub tx_bytes: i64,
}

/// Petición del cliente. Solo lectura.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum Request {
    /// Comprobación de vida y de versión de protocolo.
    Ping {
        /// Versión que habla el cliente.
        version: u32,
    },
    /// Total rx/tx del periodo.
    TotalsForPeriod { period: Period },
    /// Uso por aplicación del periodo.
    PerAppForPeriod { period: Period },
}

/// Respuesta del servidor.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum Response {
    /// Respuesta a `Ping`, con la versión del servidor.
    Pong { version: u32 },
    /// Total del periodo.
    Totals { rx_bytes: i64, tx_bytes: i64 },
    /// Lista por aplicación del periodo.
    PerApp { entries: Vec<AppUsage> },
    /// Error del lado servidor, con un mensaje legible.
    Error { message: String },
}
