//! Politica de modo de ejecucion segun el entorno detectado.
//!
//! Responsabilidad unica: traducir el informe de entorno (`EnvReport`) a una
//! decision de como debe arrancar el demonio, con un mensaje explicativo. Es
//! logica pura (sin efectos), para poder testear cada caso.
//!
//! Modos:
//! - `Full`: atribucion por aplicacion disponible (kernel >= 5.8 + cgroup v2 +
//!   BTF). Es el modo normal.
//! - `NoPerApp`: el kernel soporta `cgroup_skb` (conteo total) pero no la
//!   atribucion por aplicacion (sin `RingBuf`/`cgroup/sock_create`); se mide el
//!   total del sistema, no el desglose por app.
//! - `Disabled`: faltan requisitos duros (cgroup v2 unificado o BTF); no se
//!   puede cargar el monitor.

use netusage_common::preflight::EnvReport;

/// Modo de ejecucion resultante.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RunMode {
    /// Todo disponible: atribucion por aplicacion.
    Full,
    /// Solo conteo total del sistema, sin desglose por aplicacion.
    NoPerApp,
    /// No se puede ejecutar: falta un requisito duro.
    Disabled,
}

/// Decision de arranque: modo y motivo legible.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Decision {
    pub mode: RunMode,
    pub reason: String,
}

impl Decision {
    fn new(mode: RunMode, reason: impl Into<String>) -> Self {
        Self {
            mode,
            reason: reason.into(),
        }
    }
}

/// Decide el modo de ejecucion a partir del informe de entorno.
pub fn decide(env: &EnvReport) -> Decision {
    if !env.cgroup_v2 {
        return Decision::new(
            RunMode::Disabled,
            "se requiere cgroup v2 unificado en /sys/fs/cgroup; arranca con \
             systemd.unified_cgroup_hierarchy=1",
        );
    }
    if !env.btf {
        return Decision::new(
            RunMode::Disabled,
            "falta el BTF del kernel (/sys/kernel/btf/vmlinux); se requiere \
             CONFIG_DEBUG_INFO_BTF=y",
        );
    }
    if env.per_app {
        Decision::new(
            RunMode::Full,
            "entorno completo: atribucion de trafico por aplicacion disponible",
        )
    } else {
        Decision::new(
            RunMode::NoPerApp,
            "kernel < 5.8: se medira el consumo total del sistema, sin desglose \
             por aplicacion (sin RingBuf ni cgroup/sock_create)",
        )
    }
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
    fn full_when_everything_available() {
        assert_eq!(decide(&env(true, true, true)).mode, RunMode::Full);
    }

    #[test]
    fn no_per_app_when_old_kernel() {
        assert_eq!(decide(&env(true, true, false)).mode, RunMode::NoPerApp);
    }

    #[test]
    fn disabled_without_cgroup_v2() {
        assert_eq!(decide(&env(false, true, true)).mode, RunMode::Disabled);
    }

    #[test]
    fn disabled_without_btf() {
        assert_eq!(decide(&env(true, false, true)).mode, RunMode::Disabled);
    }
}
