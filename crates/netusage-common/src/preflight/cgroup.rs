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

/// Disposicion de la jerarquia de cgroups del sistema.
///
/// Determina con que tipo de cgroup arranco el sistema, lo que condiciona si la
/// atribucion `cgroup_skb` es posible (requiere v2 unificado).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CgroupLayout {
    /// cgroup v2 unificado montado en `/sys/fs/cgroup`. Es el caso soportado.
    Unified,
    /// Modo hibrido: cgroup2 montado (p. ej. en `/sys/fs/cgroup/unified`) junto
    /// con controladores cgroup v1. Funciona parcialmente; no recomendado.
    Hybrid,
    /// Solo cgroup v1 (jerarquias legadas). No soportado para `cgroup_skb`.
    LegacyV1,
    /// No se detecto ninguna jerarquia de cgroups reconocible.
    Unknown,
}

impl CgroupLayout {
    /// Etiqueta corta para mensajes.
    pub fn label(self) -> &'static str {
        match self {
            CgroupLayout::Unified => "cgroup v2 unificado",
            CgroupLayout::Hybrid => "cgroup hibrido (v1+v2)",
            CgroupLayout::LegacyV1 => "cgroup v1 legado",
            CgroupLayout::Unknown => "cgroup desconocido",
        }
    }
}

/// Deduce la disposicion de cgroups a partir del contenido de `/proc/mounts`.
///
/// Funcion pura: recibe el texto de mounts para testear cada caso sin tocar el
/// sistema real. Cada linea de mounts tiene el formato
/// `dispositivo punto_montaje tipo opciones ...`; el tipo (campo 3) distingue
/// `cgroup2` (v2) de `cgroup` (v1).
pub fn parse_layout(mounts: &str) -> CgroupLayout {
    let mut v2_at_root = false;
    let mut v2_anywhere = false;
    let mut v1_present = false;

    for line in mounts.lines() {
        let mut fields = line.split_whitespace();
        let _device = fields.next();
        let mount_point = fields.next().unwrap_or("");
        let fs_type = fields.next().unwrap_or("");

        match fs_type {
            "cgroup2" => {
                v2_anywhere = true;
                if mount_point == CGROUP_MOUNT {
                    v2_at_root = true;
                }
            }
            "cgroup" => v1_present = true,
            _ => {}
        }
    }

    match (v2_at_root, v2_anywhere, v1_present) {
        // v2 en la raiz sin controladores v1: unificado puro.
        (true, _, false) => CgroupLayout::Unified,
        // Hay v1 y ademas algun cgroup2 montado: hibrido.
        (_, true, true) => CgroupLayout::Hybrid,
        // v2 en la raiz pero conviven controladores v1: hibrido.
        (true, _, true) => CgroupLayout::Hybrid,
        // Solo v1.
        (false, false, true) => CgroupLayout::LegacyV1,
        // Nada reconocible.
        _ => CgroupLayout::Unknown,
    }
}

/// Detecta la disposicion de cgroups del sistema leyendo `/proc/mounts`.
pub fn detect_layout() -> CgroupLayout {
    match std::fs::read_to_string("/proc/mounts") {
        Ok(mounts) => parse_layout(&mounts),
        Err(_) => CgroupLayout::Unknown,
    }
}

/// Indica si `/sys/fs/cgroup` esta montado como cgroup v2 unificado
/// (deteccion booleana directa para el informe de entorno).
pub fn is_v2() -> bool {
    rustix::fs::statfs(CGROUP_MOUNT)
        .map(|stat| stat.f_type as u64 == CGROUP2_SUPER_MAGIC)
        .unwrap_or(false)
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

    #[test]
    fn layout_unified_when_only_cgroup2_at_root() {
        let mounts = "\
sysfs /sys sysfs rw 0 0
cgroup2 /sys/fs/cgroup cgroup2 rw,nsdelegate 0 0
";
        assert_eq!(parse_layout(mounts), CgroupLayout::Unified);
    }

    #[test]
    fn layout_hybrid_when_v2_and_v1_coexist() {
        let mounts = "\
tmpfs /sys/fs/cgroup tmpfs ro 0 0
cgroup2 /sys/fs/cgroup/unified cgroup2 rw 0 0
cgroup /sys/fs/cgroup/cpu cgroup rw,cpu 0 0
";
        assert_eq!(parse_layout(mounts), CgroupLayout::Hybrid);
    }

    #[test]
    fn layout_legacy_when_only_cgroup_v1() {
        let mounts = "\
tmpfs /sys/fs/cgroup tmpfs ro 0 0
cgroup /sys/fs/cgroup/cpu cgroup rw,cpu 0 0
cgroup /sys/fs/cgroup/memory cgroup rw,memory 0 0
";
        assert_eq!(parse_layout(mounts), CgroupLayout::LegacyV1);
    }

    #[test]
    fn layout_unknown_when_no_cgroups() {
        let mounts = "sysfs /sys sysfs rw 0 0\nproc /proc proc rw 0 0\n";
        assert_eq!(parse_layout(mounts), CgroupLayout::Unknown);
    }
}
