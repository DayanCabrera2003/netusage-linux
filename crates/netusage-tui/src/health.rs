//! Aviso de salud del entorno para la interfaz.
//!
//! Responsabilidad unica: traducir el informe de entorno a un aviso breve para
//! la barra superior de la TUI cuando el sistema no esta en modo completo.
//! En modo completo no hay aviso (la interfaz se muestra limpia).

use netusage_common::preflight::EnvReport;

/// Calcula el aviso de modo degradado consultando el entorno real.
pub fn degraded_note() -> Option<String> {
    note_from(&EnvReport::gather())
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
}
