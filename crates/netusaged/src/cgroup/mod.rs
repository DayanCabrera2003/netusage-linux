//! Localización del cgroup v2 unificado raíz.
//!
//! Responsabilidad única: devolver la ruta del cgroup v2 al que se enganchan
//! los programas eBPF, validando que el sistema realmente usa cgroup v2.

use std::path::{Path, PathBuf};

use anyhow::{bail, Result};

/// Punto de montaje estándar del cgroup v2 unificado.
const CGROUP_V2_ROOT: &str = "/sys/fs/cgroup";

/// Devuelve la ruta del cgroup v2 raíz.
///
/// Valida la presencia de `cgroup.controllers`, que solo existe en una
/// jerarquía cgroup v2 unificada. Devuelve un error claro si no es el caso.
pub fn cgroup_v2_root() -> Result<PathBuf> {
    let root = Path::new(CGROUP_V2_ROOT);
    if !root.join("cgroup.controllers").is_file() {
        bail!(
            "{CGROUP_V2_ROOT} no es cgroup v2 unificado (falta cgroup.controllers). \
             Ejecuta `netusaged --check` para diagnosticar el entorno."
        );
    }
    Ok(root.to_path_buf())
}
