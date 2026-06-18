//! Subsistema de cgroups: del árbol de `/sys/fs/cgroup` a la identidad de cada
//! aplicación.
//!
//! Responsabilidad única de este módulo (fachada): localizar el cgroup v2 raíz
//! y reexportar las piezas del subsistema (descubrimiento, inode, identidad)
//! para el resto del demonio. Cada pieza vive en su propio submódulo con una
//! única responsabilidad:
//!
//! - `discovery`: enumera los cgroups de aplicación bajo `app.slice`.
//! - `inode`: traduce la ruta de un cgroup al inode que el mapa eBPF usa como
//!   clave.
//! - `identity`: convierte el nombre de un scope de systemd en la identidad
//!   legible de la app.

use std::path::{Path, PathBuf};

use anyhow::{bail, Result};

pub mod discovery;
pub mod identity;
pub mod inode;

/// Punto de montaje estándar del cgroup v2 unificado.
pub const CGROUP_V2_ROOT: &str = "/sys/fs/cgroup";

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
