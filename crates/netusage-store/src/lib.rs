//! Capa de persistencia SQLite y queries de agregación temporal de netusage.
//!
//! Fachada del crate: expone `Store` (la conexión y sus operaciones) y los tipos
//! del dominio. Cada responsabilidad vive en su propio módulo (esquema,
//! configuración, periodos, agregación, retención).

mod apps;
mod error;
mod samples;
mod schema;
mod store;

pub use error::{Result, StoreError};
pub use samples::SampleDelta;
pub use store::Store;
