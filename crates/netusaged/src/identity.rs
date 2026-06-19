//! Resolución de la identidad de una aplicación a partir del ejecutable del
//! proceso dueño de un socket.
//!
//! Responsabilidad única: traducir un PID a la identidad de su app. Mejoras
//! sobre el modelo básico (ruta del ejecutable):
//! - **Apps interpretadas** (python/java/node/…): se resuelven al script o
//!   módulo real del `cmdline`, no al intérprete, para no agrupar todo lo de
//!   python bajo "python".
//! - **Nombres legibles**: se mapea el binario a su `Name` de `.desktop`
//!   (estilo escritorio) cuando existe; si no, el nombre del binario.

use std::collections::HashMap;
use std::path::Path;
use std::sync::OnceLock;

/// Identidad resuelta de una aplicación.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AppIdentity {
    pub app_key: String,
    pub display_name: String,
}

/// Resuelve el PID a la identidad de su app.
///
/// Devuelve `None` si el proceso ya no existe o es un hilo de kernel (PID 0).
pub fn resolve_pid(pid: u32) -> Option<AppIdentity> {
    if pid == 0 {
        return None;
    }
    let exe = std::fs::read_link(format!("/proc/{pid}/exe")).ok()?;
    let exe_base = basename(&exe.to_string_lossy());

    // Apps interpretadas: usar el script/módulo del cmdline.
    let mut identity = if is_interpreter(&exe_base) {
        match read_cmdline(pid).and_then(|c| script_from_cmdline(&c)) {
            Some(script) => AppIdentity {
                display_name: strip_script_ext(&basename(&script)),
                app_key: script,
            },
            None => identity_from_exe_path(&exe),
        }
    } else {
        identity_from_exe_path(&exe)
    };

    // Nombre legible desde .desktop, si lo hay.
    if let Some(name) = friendly_from_index(desktop_index(), &identity.display_name) {
        identity.display_name = name;
    }
    Some(identity)
}

/// Construye la identidad a partir de la ruta del ejecutable (lógica pura).
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

/// Intérpretes conocidos cuyo "app real" está en el cmdline.
fn is_interpreter(name: &str) -> bool {
    name.starts_with("python")
        || name == "java"
        || name == "ruby"
        || name == "perl"
        || name == "node"
        || name == "nodejs"
}

/// Extrae el script/módulo/jar de un `cmdline` (NUL-separado).
///
/// Salta `arg0` (el intérprete) y las banderas; reconoce `-jar`/`-cp`/`-m` cuyo
/// valor siguiente es el objetivo. Devuelve `None` si no hay un objetivo claro.
fn script_from_cmdline(cmdline: &[u8]) -> Option<String> {
    let args: Vec<String> = cmdline
        .split(|b| *b == 0)
        .filter(|s| !s.is_empty())
        .map(|s| String::from_utf8_lossy(s).into_owned())
        .collect();
    if args.len() < 2 {
        return None;
    }
    let mut i = 1;
    while i < args.len() {
        let arg = &args[i];
        if matches!(arg.as_str(), "-jar" | "-cp" | "-classpath" | "-m") {
            return args.get(i + 1).cloned();
        }
        if arg.starts_with('-') {
            i += 1;
            continue;
        }
        return Some(arg.clone());
    }
    None
}

/// Quita extensiones de script comunes para el nombre visible.
fn strip_script_ext(name: &str) -> String {
    for ext in [".py", ".js", ".jar", ".rb", ".pl"] {
        if let Some(stem) = name.strip_suffix(ext) {
            return stem.to_string();
        }
    }
    name.to_string()
}

/// Último componente de una ruta.
fn basename(path: &str) -> String {
    path.rsplit('/').next().unwrap_or(path).to_string()
}

/// Busca el nombre legible de un binario en el índice de `.desktop`.
fn friendly_from_index(index: &HashMap<String, String>, exe_base: &str) -> Option<String> {
    index.get(exe_base).cloned()
}

/// Lee el `cmdline` del proceso.
fn read_cmdline(pid: u32) -> Option<Vec<u8>> {
    std::fs::read(format!("/proc/{pid}/cmdline")).ok()
}

/// Índice `binario -> Name` de los `.desktop` del sistema, construido una vez.
fn desktop_index() -> &'static HashMap<String, String> {
    static INDEX: OnceLock<HashMap<String, String>> = OnceLock::new();
    INDEX.get_or_init(build_desktop_index)
}

/// Construye el índice escaneando los directorios de aplicaciones del sistema.
///
/// Como el demonio corre como usuario de servicio, solo se leen los directorios
/// del sistema (no el `~/.local/share/applications` de cada usuario).
fn build_desktop_index() -> HashMap<String, String> {
    const DIRS: [&str; 3] = [
        "/usr/share/applications",
        "/usr/local/share/applications",
        "/var/lib/flatpak/exports/share/applications",
    ];
    let mut index = HashMap::new();
    for dir in DIRS {
        let Ok(entries) = std::fs::read_dir(dir) else {
            continue;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            if path.extension().and_then(|e| e.to_str()) != Some("desktop") {
                continue;
            }
            if let Ok(content) = std::fs::read_to_string(&path) {
                if let Some((bin, name)) = parse_desktop_entry(&content) {
                    // No sobrescribir: gana el primero (orden de DIRS).
                    index.entry(bin).or_insert(name);
                }
            }
        }
    }
    index
}

/// De un `.desktop`, devuelve `(binario, Name)` de su sección `[Desktop Entry]`.
fn parse_desktop_entry(content: &str) -> Option<(String, String)> {
    let mut in_entry = false;
    let mut name: Option<String> = None;
    let mut exec_bin: Option<String> = None;

    for line in content.lines() {
        let line = line.trim();
        if line.starts_with('[') {
            in_entry = line == "[Desktop Entry]";
            continue;
        }
        if !in_entry {
            continue;
        }
        // Solo la clave no localizada (`Name=`, no `Name[es]=`).
        if let Some(rest) = line.strip_prefix("Name=") {
            name.get_or_insert_with(|| rest.trim().to_string());
        } else if let Some(rest) = line.strip_prefix("Exec=") {
            if exec_bin.is_none() {
                exec_bin = exec_binary(rest.trim());
            }
        }
    }
    Some((exec_bin?, name?))
}

/// Del valor `Exec=` extrae el basename del binario, ignorando rutas, field
/// codes (`%U`, `%f`…) y argumentos.
fn exec_binary(exec: &str) -> Option<String> {
    let first = exec.split_whitespace().next()?;
    // Algunos Exec arrancan con `env` o una ruta; tomar el basename del primer
    // token que no sea una asignación VAR=...
    let token = if first.contains('=') {
        exec.split_whitespace().nth(1).unwrap_or(first)
    } else {
        first
    };
    let base = basename(token);
    if base.is_empty() {
        None
    } else {
        Some(base)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detects_interpreters() {
        assert!(is_interpreter("python3.12"));
        assert!(is_interpreter("java"));
        assert!(is_interpreter("node"));
        assert!(!is_interpreter("firefox"));
    }

    #[test]
    fn extracts_python_script_from_cmdline() {
        let cmd = b"python3\0/usr/bin/myapp.py\0--flag\0";
        assert_eq!(
            script_from_cmdline(cmd),
            Some("/usr/bin/myapp.py".to_string())
        );
    }

    #[test]
    fn extracts_java_jar_from_cmdline() {
        let cmd = b"java\0-Xmx512m\0-jar\0/opt/app/app.jar\0";
        assert_eq!(
            script_from_cmdline(cmd),
            Some("/opt/app/app.jar".to_string())
        );
    }

    #[test]
    fn node_app_from_cmdline() {
        let cmd = b"node\0/srv/server.js\0";
        assert_eq!(script_from_cmdline(cmd), Some("/srv/server.js".to_string()));
    }

    #[test]
    fn cmdline_without_target_is_none() {
        assert_eq!(script_from_cmdline(b"python3\0"), None);
        assert_eq!(script_from_cmdline(b"python3\0-q\0"), None);
    }

    #[test]
    fn strips_script_extensions() {
        assert_eq!(strip_script_ext("server.js"), "server");
        assert_eq!(strip_script_ext("app.jar"), "app");
        assert_eq!(strip_script_ext("plain"), "plain");
    }

    #[test]
    fn parses_desktop_entry() {
        let content = "[Desktop Entry]\nType=Application\nName=Firefox\nName[es]=Zorro\nExec=/usr/lib/firefox/firefox %u\n";
        assert_eq!(
            parse_desktop_entry(content),
            Some(("firefox".to_string(), "Firefox".to_string()))
        );
    }

    #[test]
    fn friendly_name_uses_index() {
        let mut index = HashMap::new();
        index.insert("brave-browser".to_string(), "Brave".to_string());
        assert_eq!(
            friendly_from_index(&index, "brave-browser"),
            Some("Brave".to_string())
        );
        assert_eq!(friendly_from_index(&index, "desconocido"), None);
    }

    #[test]
    fn exe_path_identity_is_pure() {
        let id = identity_from_exe_path(Path::new("/usr/lib/firefox/firefox"));
        assert_eq!(id.app_key, "/usr/lib/firefox/firefox");
        assert_eq!(id.display_name, "firefox");
    }

    #[test]
    fn pid_zero_unresolved() {
        assert!(resolve_pid(0).is_none());
    }
}
