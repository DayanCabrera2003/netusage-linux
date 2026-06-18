//! Resolución de la identidad de una aplicación a partir del ejecutable del
//! proceso dueño de un socket.
//!
//! Responsabilidad única: traducir un PID a la identidad de su app leyendo el
//! ejecutable (`/proc/<pid>/exe`). La identidad es por binario: la `app_key` es
//! la ruta del ejecutable (estable, sirve de clave de persistencia en la Fase
//! 3) y el `display_name` es su nombre legible.
//!
//! Decisión documentada: identidad por ejecutable. Las apps interpretadas
//! (python/java/electron genéricos) se agrupan por el intérprete; mejora futura:
//! usar el cmdline para distinguirlas.

use std::path::Path;

/// Identidad resuelta de una aplicación.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AppIdentity {
    pub app_key: String,
    pub display_name: String,
}

/// Resuelve el PID al ejecutable de su proceso y de ahí a su identidad.
///
/// Devuelve `None` si el proceso ya no existe o no se puede leer su ejecutable
/// (proceso efímero, hilo de kernel, PID 0): el llamador manda ese tráfico al
/// cubo de fallback.
pub fn resolve_pid(pid: u32) -> Option<AppIdentity> {
    if pid == 0 {
        return None;
    }
    let exe = std::fs::read_link(format!("/proc/{pid}/exe")).ok()?;
    Some(identity_from_exe_path(&exe))
}

/// Construye la identidad a partir de la ruta del ejecutable (lógica pura).
///
/// `app_key` es la ruta tal cual; `display_name` es el último componente
/// (basename). Para `kernel`-threads o rutas raras, degrada a la propia ruta sin
/// hacer panic.
pub fn identity_from_exe_path(exe: &Path) -> AppIdentity {
    let app_key = exe.to_string_lossy().into_owned();
    let display_name = exe
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .filter(|n| !n.is_empty())
        .unwrap_or_else(|| app_key.clone());
    AppIdentity {
        app_key,
        display_name,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;

    #[test]
    fn brave_exe_resolves_to_brave() {
        let id = identity_from_exe_path(Path::new("/opt/brave.com/brave/brave"));
        assert_eq!(id.app_key, "/opt/brave.com/brave/brave");
        assert_eq!(id.display_name, "brave");
    }

    #[test]
    fn firefox_exe_resolves_to_firefox() {
        let id = identity_from_exe_path(Path::new("/usr/lib/firefox/firefox"));
        assert_eq!(id.display_name, "firefox");
    }

    #[test]
    fn interpreter_is_grouped_by_binary() {
        // Las apps interpretadas se agrupan por el intérprete (decisión asumida).
        let id = identity_from_exe_path(Path::new("/usr/bin/python3.12"));
        assert_eq!(id.display_name, "python3.12");
    }

    #[test]
    fn path_with_spaces_and_root_never_panic() {
        let id = identity_from_exe_path(Path::new("/opt/My App/my app"));
        assert_eq!(id.display_name, "my app");
        // Ruta sin componente final: degrada a la propia ruta, sin panic.
        let id_root = identity_from_exe_path(Path::new("/"));
        assert_eq!(id_root.display_name, "/");
    }

    #[test]
    fn pid_zero_is_unresolved() {
        assert!(resolve_pid(0).is_none());
    }

    #[test]
    fn nonexistent_pid_is_unresolved() {
        // Un PID altísimo casi seguro no existe; nunca debe hacer panic.
        assert!(resolve_pid(u32::MAX).is_none());
    }
}
