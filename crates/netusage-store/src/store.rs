//! La struct `Store`: conexión SQLite, pragmas y migraciones.
//!
//! Responsabilidad única: gestionar la conexión y su configuración. Las queries
//! de negocio (apps, samples, agregación, retención) viven en sus módulos y se
//! implementan como métodos de `Store` en esos archivos.

use std::path::Path;

use rusqlite::Connection;

use crate::error::Result;
use crate::schema;

/// Conexión a la base de datos de netusage, ya migrada y configurada.
pub struct Store {
    pub(crate) conn: Connection,
}

impl Store {
    /// Abre (o crea) la base de datos en `path`.
    pub fn open<P: AsRef<Path>>(path: P) -> Result<Store> {
        Self::init(Connection::open(path)?)
    }

    /// Abre una base de datos en memoria (para tests y uso efímero).
    pub fn open_in_memory() -> Result<Store> {
        Self::init(Connection::open_in_memory()?)
    }

    /// Configura los pragmas y aplica las migraciones sobre una conexión recién
    /// abierta.
    fn init(mut conn: Connection) -> Result<Store> {
        // WAL mejora la concurrencia lectura/escritura; synchronous=NORMAL es un
        // buen equilibrio durabilidad/rendimiento con WAL; foreign_keys activa el
        // borrado en cascada de samples/daily al borrar una app.
        conn.execute_batch(
            "PRAGMA journal_mode=WAL; PRAGMA synchronous=NORMAL; PRAGMA foreign_keys=ON;",
        )?;
        schema::migrate(&mut conn)?;
        Ok(Store { conn })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn opens_in_memory_with_foreign_keys_on() {
        let store = Store::open_in_memory().unwrap();
        let fk: i64 = store
            .conn
            .query_row("PRAGMA foreign_keys", [], |r| r.get(0))
            .unwrap();
        assert_eq!(fk, 1);
    }
}
