//! Aviso de salud del entorno para la interfaz.
//!
//! Responsabilidad unica: traducir el informe de entorno a un aviso breve para
//! la barra superior de la TUI cuando el sistema no esta en modo completo.
//! En modo completo no hay aviso (la interfaz se muestra limpia).

use std::path::Path;

use netusage_common::preflight::EnvReport;

/// Calcula el aviso de modo degradado consultando el entorno real.
pub fn degraded_note() -> Option<String> {
    note_from(&EnvReport::gather())
}

/// Construye un mensaje accionable cuando no se puede leer la base del demonio.
///
/// Traduce el fallo crudo de SQLite (que el usuario no sabe interpretar) a la
/// causa probable y al comando que la resuelve, priorizando:
/// 1. entorno no apto (el demonio no puede monitorizar en este sistema),
/// 2. base ausente (lo normal: el demonio no esta corriendo),
/// 3. base presente pero ilegible (permisos u otro error).
pub fn connection_hint(db_path: &Path, err: &str, degraded: Option<&str>) -> String {
    if let Some(note) = degraded {
        return format!(
            "{note}.\nEl demonio no puede monitorizar en este sistema. Diagnostica con:\n    netusaged --check"
        );
    }
    if !db_path.exists() {
        return format!(
            "No se encontro la base de datos en {}.\nEl demonio no esta corriendo o aun no la ha creado. Inicialo con:\n    sudo systemctl enable --now netusaged",
            db_path.display()
        );
    }
    format!(
        "No se pudo leer la base en {}:\n    {err}\nComprueba que el servicio esta activo:\n    systemctl status netusaged",
        db_path.display()
    )
}

/// Logica pura: deriva el aviso a partir de un informe de entorno.
fn note_from(env: &EnvReport) -> Option<String> {
    if !env.cgroup_v2 || !env.btf {
        return Some(
            "Entorno no apto: el demonio no puede monitorizar (falta cgroup v2 o BTF)".to_string(),
        );
    }
    if !env.per_app {
        return Some(
            "Modo degradado: solo consumo total del sistema, sin desglose por aplicacion"
                .to_string(),
        );
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    fn env(cgroup_v2: bool, btf: bool, per_app: bool) -> EnvReport {
        EnvReport {
            kernel: "test".to_string(),
            cgroup_v2,
            btf,
            per_app,
            caps_ok: true,
        }
    }

    #[test]
    fn full_environment_has_no_note() {
        assert!(note_from(&env(true, true, true)).is_none());
    }

    #[test]
    fn old_kernel_warns_no_per_app() {
        let note = note_from(&env(true, true, false)).unwrap();
        assert!(note.contains("degradado"));
    }

    #[test]
    fn missing_requirement_warns_not_apt() {
        assert!(note_from(&env(false, true, true))
            .unwrap()
            .contains("no apto"));
    }

    #[test]
    fn hint_for_missing_db_suggests_starting_the_daemon() {
        let path = Path::new("/definitivamente/no/existe/netusage.db");
        let hint = connection_hint(path, "error de SQLite: unable to open database file", None);
        assert!(hint.contains("No se encontro la base"), "{hint}");
        assert!(hint.contains("systemctl enable --now netusaged"), "{hint}");
    }

    #[test]
    fn hint_prioritizes_degraded_environment() {
        let path = Path::new("/definitivamente/no/existe/netusage.db");
        let hint = connection_hint(path, "x", Some("Entorno no apto: falta cgroup v2 o BTF"));
        assert!(hint.contains("Entorno no apto"), "{hint}");
        assert!(hint.contains("netusaged --check"), "{hint}");
    }

    #[test]
    fn hint_for_existing_but_unreadable_db_points_to_status() {
        let path = std::env::temp_dir().join(format!("netusage-hint-{}.db", std::process::id()));
        std::fs::write(&path, b"no soy una base valida").unwrap();
        let hint = connection_hint(&path, "error de datos: archivo corrupto", None);
        assert!(hint.contains("systemctl status netusaged"), "{hint}");
        std::fs::remove_file(&path).ok();
    }
}
