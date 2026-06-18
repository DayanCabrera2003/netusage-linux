//! Comprobador de cgroup v2 unificado.
//!
//! Responsabilidad unica: verificar que `/sys/fs/cgroup` esta montado como
//! cgroup v2 unificado (cgroup2fs), requisito para enganchar programas
//! `cgroup_skb`.

use super::{CheckResult, CheckStatus};

/// Magic number del superbloque de cgroup v2 (`CGROUP2_SUPER_MAGIC`).
pub const CGROUP2_SUPER_MAGIC: u64 = 0x6367_7270;

/// Ruta estandar del punto de montaje de cgroup.
const CGROUP_MOUNT: &str = "/sys/fs/cgroup";

const CHECK_NAME: &str = "cgroup v2 unificado";

/// Clasifica el magic del sistema de ficheros montado en `/sys/fs/cgroup`.
///
/// Funcion pura para poder testearla con valores inyectados sin depender del
/// sistema de ficheros real.
pub fn classify_magic(f_type: u64) -> CheckResult {
    if f_type == CGROUP2_SUPER_MAGIC {
        CheckResult::new(
            CHECK_NAME,
            CheckStatus::Ok,
            format!("{CGROUP_MOUNT} es cgroup2fs (cgroup v2 unificado)"),
        )
    } else {
        CheckResult::new(
            CHECK_NAME,
            CheckStatus::Fail,
            format!(
                "{CGROUP_MOUNT} no es cgroup v2 unificado (magic 0x{f_type:x}); \
                 se requiere cgroup v2 (cgroup2fs). Arranca con \
                 systemd.unified_cgroup_hierarchy=1"
            ),
        )
    }
}

/// Ejecuta el comprobador contra el sistema real.
pub fn check() -> CheckResult {
    match rustix::fs::statfs(CGROUP_MOUNT) {
        Ok(stat) => classify_magic(stat.f_type as u64),
        Err(err) => CheckResult::new(
            CHECK_NAME,
            CheckStatus::Fail,
            format!("no se pudo consultar {CGROUP_MOUNT}: {err}"),
        ),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cgroup2_magic_is_ok() {
        let r = classify_magic(CGROUP2_SUPER_MAGIC);
        assert_eq!(r.status, CheckStatus::Ok);
    }

    #[test]
    fn tmpfs_magic_is_fail() {
        // 0x01021994 es TMPFS_MAGIC: representa un montaje que no es cgroup v2.
        let r = classify_magic(0x0102_1994);
        assert_eq!(r.status, CheckStatus::Fail);
    }

    #[test]
    fn cgroup_v1_magic_is_fail() {
        // 0x27e0eb es CGROUP_SUPER_MAGIC (cgroup v1).
        let r = classify_magic(0x0027_e0eb);
        assert_eq!(r.status, CheckStatus::Fail);
    }
}
