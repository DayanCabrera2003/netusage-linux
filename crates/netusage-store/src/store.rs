//! La struct `Store`: conexión SQLite, pragmas y migraciones.
//!
//! Responsabilidad única: gestionar la conexión y su configuración. Las queries
//! de negocio (apps, samples, agregación, retención) viven en sus módulos y se
//! implementan como métodos de `Store` en esos archivos.

use std::collections::HashMap;
use std::path::Path;

use rusqlite::Connection;

use crate::error::Result;
use crate::schema;

/// Conexión a la base de datos de netusage, ya migrada y configurada.
pub struct Store {
    pub(crate) conn: Connection,
    /// Caché `app_key -> app_id` para no resolver la fila en cada muestra.
    pub(crate) app_cache: HashMap<String, i64>,
}

impl Store {
    /// Abre (o crea) la base de datos en `path`.
    pub fn open<P: AsRef<Path>>(path: P) -> Result<Store> {
        Self::init(Connection::open(path)?)
    }

    /// Abre la base de datos en `path` en modo **solo lectura**.
    ///
    /// Es el camino que usa la interfaz sin privilegios: la conexión no puede
    /// ejecutar `INSERT`/`UPDATE`/DDL, así que la UI no puede modificar datos
    /// aunque quisiera. No aplica migraciones (la base ya existe, creada por el
    /// demonio); reutiliza las mismas queries de agregación de la Fase 3.
    pub fn open_readonly<P: AsRef<Path>>(path: P) -> Result<Store> {
        use rusqlite::OpenFlags;
        let conn = Connection::open_with_flags(
            path,
            OpenFlags::SQLITE_OPEN_READ_ONLY | OpenFlags::SQLITE_OPEN_NO_MUTEX,
        )?;
        Ok(Store {
            conn,
            app_cache: HashMap::new(),
        })
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
        Ok(Store {
            conn,
            app_cache: HashMap::new(),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn readonly_can_query_but_not_write() {
        // Sembrar una base en disco, cerrarla y reabrirla en solo lectura.
        let path = std::env::temp_dir().join(format!("netusage-ro-{}.db", std::process::id()));
        {
            let mut store = Store::open(&path).unwrap();
            let id = store.upsert_app("/app", "app", 0).unwrap();
            store
                .insert_samples(
                    100,
                    &[crate::samples::SampleDelta {
                        app_id: id,
                        rx_bytes: 5,
                        tx_bytes: 1,
                    }],
                )
                .unwrap();
        }

        let ro = Store::open_readonly(&path).unwrap();
        // Una query de lectura funciona.
        let count: i64 = ro
            .conn
            .query_row("SELECT count(*) FROM samples", [], |r| r.get(0))
            .unwrap();
        assert_eq!(count, 1);
        // Una escritura falla (base de solo lectura).
        let write = ro.conn.execute(
            "INSERT INTO apps(app_key, display_name, first_seen) VALUES ('x','x',0)",
            [],
        );
        assert!(
            write.is_err(),
            "no debe poder escribir en modo solo lectura"
        );

        std::fs::remove_file(&path).ok();
    }

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
