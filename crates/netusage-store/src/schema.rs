//! Esquema SQLite y migraciones versionadas.
//!
//! Responsabilidad única: crear y evolucionar el esquema. La versión del
//! esquema se guarda en `PRAGMA user_version`; al abrir, se aplican en orden las
//! migraciones pendientes dentro de una transacción.
//!
//! Unidades: todos los timestamps son epoch en **segundos UTC**. `samples`
//! guarda **deltas** de bytes (no contadores absolutos), ya desreseteados por el
//! demonio. `daily` son agregados por día local (medianoche local expresada en
//! epoch UTC). Las tablas de series usan `WITHOUT ROWID` porque su clave
//! primaria compuesta ya es el identificador natural y ahorra el rowid implícito.

use rusqlite::Connection;

use crate::error::{Result, StoreError};

/// Migración 1: esquema base.
const MIGRATION_1: &str = r#"
CREATE TABLE apps (
    id           INTEGER PRIMARY KEY,
    app_key      TEXT NOT NULL UNIQUE,
    display_name TEXT NOT NULL,
    first_seen   INTEGER NOT NULL
);

CREATE TABLE samples (
    app_id   INTEGER NOT NULL REFERENCES apps(id) ON DELETE CASCADE,
    ts       INTEGER NOT NULL,
    rx_bytes INTEGER NOT NULL,
    tx_bytes INTEGER NOT NULL,
    PRIMARY KEY (app_id, ts)
) WITHOUT ROWID;

CREATE INDEX idx_samples_ts ON samples(ts);

CREATE TABLE daily (
    app_id   INTEGER NOT NULL REFERENCES apps(id) ON DELETE CASCADE,
    day      INTEGER NOT NULL,
    rx_bytes INTEGER NOT NULL,
    tx_bytes INTEGER NOT NULL,
    PRIMARY KEY (app_id, day)
) WITHOUT ROWID;

CREATE INDEX idx_daily_day ON daily(day);

CREATE TABLE config (
    key   TEXT PRIMARY KEY,
    value TEXT NOT NULL
);
"#;

/// Lista ordenada de migraciones. El índice + 1 es la versión que deja el
/// esquema; `MIGRATIONS.len()` es la versión objetivo.
const MIGRATIONS: &[&str] = &[MIGRATION_1];

/// Aplica las migraciones pendientes hasta dejar el esquema en la última versión.
///
/// Idempotente: si la base ya está en la última versión, no hace nada.
pub fn migrate(conn: &mut Connection) -> Result<()> {
    let current: i64 = conn.query_row("PRAGMA user_version", [], |row| row.get(0))?;
    let target = MIGRATIONS.len() as i64;
    if current > target {
        return Err(StoreError::Migration(format!(
            "la base está en la versión {current}, posterior a la conocida {target}"
        )));
    }
    if current == target {
        return Ok(());
    }

    let tx = conn.transaction()?;
    for sql in MIGRATIONS.iter().skip(current as usize) {
        tx.execute_batch(sql)?;
    }
    tx.pragma_update(None, "user_version", target)?;
    tx.commit()?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn migrate_creates_schema_and_sets_version() {
        let mut conn = Connection::open_in_memory().unwrap();
        migrate(&mut conn).unwrap();

        let version: i64 = conn
            .query_row("PRAGMA user_version", [], |r| r.get(0))
            .unwrap();
        assert_eq!(version, 1);

        let tables: Vec<String> = {
            let mut stmt = conn
                .prepare("SELECT name FROM sqlite_master WHERE type='table' ORDER BY name")
                .unwrap();
            let rows = stmt.query_map([], |r| r.get::<_, String>(0)).unwrap();
            rows.map(|r| r.unwrap()).collect()
        };
        assert!(tables.contains(&"apps".to_string()));
        assert!(tables.contains(&"samples".to_string()));
        assert!(tables.contains(&"daily".to_string()));
        assert!(tables.contains(&"config".to_string()));
    }

    #[test]
    fn migrate_is_idempotent() {
        let mut conn = Connection::open_in_memory().unwrap();
        migrate(&mut conn).unwrap();
        // Segunda llamada: no debe fallar ni duplicar tablas.
        migrate(&mut conn).unwrap();
        let version: i64 = conn
            .query_row("PRAGMA user_version", [], |r| r.get(0))
            .unwrap();
        assert_eq!(version, 1);
    }
}
