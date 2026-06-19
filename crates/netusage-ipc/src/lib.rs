//! Protocolo IPC demonio (`netusaged`) <-> interfaz.
//!
//! Define los tipos de petición/respuesta de solo lectura, la versión del
//! protocolo, el framing por longitud + `postcard`, y el cliente/servidor de
//! socket Unix. No contiene lógica de negocio ni acceso a SQLite ni a eBPF.

pub mod client;
pub mod codec;
pub mod error;
pub mod protocol;

pub use client::Client;
pub use error::IpcError;
pub use protocol::{
    AppUsage, Period, Request, Response, DEFAULT_SOCKET_PATH, PROTOCOL_VERSION,
};
