//! Aviso de nueva release para la interfaz.
//!
//! Responsabilidad unica: en segundo plano (sin bloquear el arranque de la TUI),
//! averiguar si hay una version mas nueva publicada en GitHub y, si la hay,
//! entregar el texto del banner por un canal. Usa una cache con TTL para no
//! llamar a la API en cada arranque y para poder avisar tambien sin red. El
//! chequeo es desactivable (opt-out por flag de CLI o variable de entorno).
//!
//! Vive solo en la TUI: el demonio corre con un sandbox systemd sin familias de
//! direcciones de red (`AF_INET`), asi que no puede hacer la peticion HTTP.

mod cache;
mod github;
mod version;

use std::sync::mpsc::{self, Receiver};
use std::thread;

/// Pagina de releases que se muestra en el banner para que el usuario actualice.
const RELEASES_URL: &str = "https://github.com/DayanCabrera2003/netusage-linux/releases/latest";

/// Variable de entorno que desactiva el chequeo (cualquier valor no vacio).
const OPT_OUT_ENV: &str = "NETUSAGE_NO_UPDATE_CHECK";

/// Indica si el chequeo esta desactivado, por el flag de CLI o por la variable
/// de entorno `NETUSAGE_NO_UPDATE_CHECK`.
pub fn is_disabled(cli_flag: bool) -> bool {
    cli_flag
        || std::env::var_os(OPT_OUT_ENV)
            .map(|v| !v.is_empty())
            .unwrap_or(false)
}

/// Lanza el chequeo en segundo plano y devuelve el extremo receptor por el que
/// llegara, como mucho una vez, el texto del banner si hay una version nueva.
///
/// Devuelve `None` si el chequeo esta desactivado: en ese caso no se crea ningun
/// hilo ni se hace ninguna peticion de red.
pub fn spawn_check(disabled: bool) -> Option<Receiver<String>> {
    if disabled {
        return None;
    }
    let (tx, rx) = mpsc::channel();
    thread::spawn(move || {
        if let Some(banner) = check() {
            // El receptor puede haberse cerrado (TUI ya cerrada): ignorar error.
            let _ = tx.send(banner);
        }
    });
    Some(rx)
}

/// Resuelve el ultimo tag (cache fresca o GitHub) y, si es mas nuevo que el
/// actual, construye el texto del banner.
fn check() -> Option<String> {
    let latest = resolve_latest_tag()?;
    version::is_newer(&latest, version::CURRENT).then(|| banner_text(&latest))
}

/// Texto del banner para una version `latest` mas nueva que la compilada.
fn banner_text(latest: &str) -> String {
    format!(
        "Nueva version {latest} disponible (tienes v{}) - {RELEASES_URL}",
        version::CURRENT
    )
}

/// Devuelve el ultimo tag conocido: usa la cache si esta fresca; si no, llama a
/// GitHub y actualiza la cache. Sin red, cae al ultimo dato cacheado (aunque
/// este vencido) para poder avisar igualmente.
fn resolve_latest_tag() -> Option<String> {
    let now = cache::now_secs();
    let cached = cache::read();
    if let Some(c) = &cached {
        if c.is_fresh(now) {
            return Some(c.tag.clone());
        }
    }
    match github::latest_tag() {
        Some(tag) => {
            cache::write(&tag, now);
            Some(tag)
        }
        None => cached.map(|c| c.tag),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cli_flag_disables_check() {
        assert!(is_disabled(true));
    }

    #[test]
    fn banner_mentions_both_versions_and_the_url() {
        let text = banner_text("v0.2.0");
        assert!(text.contains("v0.2.0"));
        assert!(text.contains(&format!("v{}", version::CURRENT)));
        assert!(text.contains(RELEASES_URL));
    }
}
