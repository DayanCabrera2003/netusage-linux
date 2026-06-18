//! Capa de persistencia SQLite y queries de agregación temporal de netusage.
//!
//! Fachada del crate: expone `Store` (la conexión y sus operaciones) y los tipos
//! del dominio. Cada responsabilidad vive en su propio módulo (esquema,
//! configuración, periodos, agregación, retención).

mod error;
mod schema;

pub use error::{Result, StoreError};
