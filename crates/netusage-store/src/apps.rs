//! Alta y resolución de aplicaciones (tabla `apps`).
//!
//! Responsabilidad única: traducir `app_key` (ruta del ejecutable) a `app_id`,
//! creando la fila si no existe, con una caché en memoria para evitar el
//! round-trip a SQLite en cada muestra.

use rusqlite::params;

use crate::error::Result;
use crate::store::Store;

impl Store {
    /// Devuelve el `app_id` de `app_key`, creándolo si no existe.
    ///
    /// `now` (epoch segundos UTC) solo se usa como `first_seen` al crear la fila;
    /// en upserts posteriores no se toca. El `display_name` se actualiza en la
    /// fila cuando se inserta o cuando hay conflicto y no estaba cacheado.
    pub fn upsert_app(&mut self, app_key: &str, display_name: &str, now: i64) -> Result<i64> {
        if let Some(&id) = self.app_cache.get(app_key) {
            return Ok(id);
        }
        let id: i64 = self.conn.query_row(
            "INSERT INTO apps(app_key, display_name, first_seen) VALUES (?1, ?2, ?3)
             ON CONFLICT(app_key) DO UPDATE SET display_name = excluded.display_name
             RETURNING id",
            params![app_key, display_name, now],
            |row| row.get(0),
        )?;
        self.app_cache.insert(app_key.to_string(), id);
        Ok(id)
    }
}

#[cfg(test)]
mod tests {
    use crate::store::Store;

    #[test]
    fn upsert_is_stable_and_keeps_first_seen() {
        let mut store = Store::open_in_memory().unwrap();
        let id1 = store.upsert_app("/usr/lib/firefox/firefox", "firefox", 1000).unwrap();
        // Forzar el camino de BD en la segunda llamada vaciando la caché.
        store.app_cache.clear();
        let id2 = store
            .upsert_app("/usr/lib/firefox/firefox", "firefox", 9999)
            .unwrap();
        assert_eq!(id1, id2);

        let first_seen: i64 = store
            .conn
            .query_row(
                "SELECT first_seen FROM apps WHERE id = ?1",
                [id1],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(first_seen, 1000, "first_seen no debe cambiar en upserts");
    }

    #[test]
    fn distinct_keys_get_distinct_ids() {
        let mut store = Store::open_in_memory().unwrap();
        let a = store.upsert_app("/usr/bin/a", "a", 0).unwrap();
        let b = store.upsert_app("/usr/bin/b", "b", 0).unwrap();
        assert_ne!(a, b);
    }
}
