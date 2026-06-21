//! Cache en disco del resultado del chequeo de release.
//!
//! Responsabilidad unica: recordar entre ejecuciones cual fue la ultima release
//! vista y cuando se consulto. Sirve para dos cosas: no llamar a la API de
//! GitHub en cada arranque (su limite de peticiones sin autenticar es bajo) y
//! poder avisar tambien sin red, reutilizando el ultimo dato conocido. Todo es
//! best-effort: si el fichero no se puede leer o escribir, se ignora.

use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

/// Validez del dato cacheado: 24 h. Dentro de esa ventana no se vuelve a llamar
/// a GitHub.
const TTL_SECS: u64 = 24 * 3600;

/// Dato persistido entre ejecuciones.
pub struct Cached {
    /// Ultimo tag de release visto.
    pub tag: String,
    /// Momento (epoch en segundos) en que se consulto.
    pub checked_at: u64,
}

impl Cached {
    /// Indica si el dato sigue dentro de la ventana de validez respecto a `now`.
    pub fn is_fresh(&self, now: u64) -> bool {
        now.saturating_sub(self.checked_at) < TTL_SECS
    }
}

/// Instante actual en segundos epoch (0 si el reloj es anterior a la epoch).
pub fn now_secs() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

/// Ruta del fichero de cache: `$XDG_CACHE_HOME/netusage/release-check.json`, o
/// `~/.cache/netusage/release-check.json` si la variable no esta. `None` si no
/// se puede determinar el directorio.
fn cache_path() -> Option<PathBuf> {
    let base = std::env::var_os("XDG_CACHE_HOME")
        .map(PathBuf::from)
        .filter(|p| p.is_absolute())
        .or_else(|| std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".cache")))?;
    Some(base.join("netusage").join("release-check.json"))
}

/// Lee el dato cacheado, si existe y es parseable.
pub fn read() -> Option<Cached> {
    let body = std::fs::read_to_string(cache_path()?).ok()?;
    let value: serde_json::Value = serde_json::from_str(&body).ok()?;
    let tag = value.get("tag")?.as_str()?.to_string();
    let checked_at = value.get("checked_at")?.as_u64()?;
    if tag.is_empty() {
        return None;
    }
    Some(Cached { tag, checked_at })
}

/// Guarda el dato (best-effort): crea el directorio si hace falta e ignora
/// cualquier error de escritura.
pub fn write(tag: &str, checked_at: u64) {
    let Some(path) = cache_path() else {
        return;
    };
    if let Some(dir) = path.parent() {
        let _ = std::fs::create_dir_all(dir);
    }
    let json = serde_json::json!({ "tag": tag, "checked_at": checked_at }).to_string();
    let _ = std::fs::write(path, json);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fresh_within_ttl_and_stale_after() {
        let c = Cached {
            tag: "v0.1.0".to_string(),
            checked_at: 1_000,
        };
        assert!(c.is_fresh(1_000));
        assert!(c.is_fresh(1_000 + TTL_SECS - 1));
        assert!(!c.is_fresh(1_000 + TTL_SECS));
        assert!(!c.is_fresh(1_000 + TTL_SECS + 10));
    }

    #[test]
    fn future_checked_at_is_treated_as_fresh() {
        // saturating_sub evita el desbordamiento si el reloj retrocede.
        let c = Cached {
            tag: "v0.1.0".to_string(),
            checked_at: 5_000,
        };
        assert!(c.is_fresh(1_000));
    }
}
