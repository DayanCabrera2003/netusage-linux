//! Autoprueba de carga eBPF (`--selftest-load`).
//!
//! Responsabilidad unica: probar de extremo a extremo que el objeto eBPF carga
//! y que los tres programas enganchan al cgroup v2 raiz en este sistema, y
//! liberar todo de inmediato. Es la verificacion definitiva de que el entorno
//! soporta la atribucion (mas alla de la estimacion estatica de `EnvReport`),
//! pensada para validacion en CI y en distintas distribuciones.
//!
//! No muestrea ni persiste: carga, engancha, e informa. Al soltarse el handle
//! `Ebpf` los programas y links se desenganchan.

use anyhow::{Context, Result};

use crate::{attach, cgroup, loader, privileges};

/// Ejecuta la autoprueba y devuelve el codigo de salida del proceso.
pub fn run() -> i32 {
    match try_load() {
        Ok(()) => {
            println!("selftest-load: OK (objeto eBPF cargado y enganchado al cgroup raiz)");
            0
        }
        Err(err) => {
            eprintln!("selftest-load: FALLO: {err:#}");
            1
        }
    }
}

/// Carga el objeto, engancha los programas al cgroup raiz y libera.
fn try_load() -> Result<()> {
    // Exigir privilegios primero para dar un mensaje claro en vez de un EPERM
    // opaco desde el verificador del kernel.
    privileges::ensure_minimum().context("privilegios insuficientes para la autoprueba")?;

    let root = cgroup::cgroup_v2_root().context("localizando el cgroup v2 raiz")?;
    let cgroup = std::fs::File::open(&root)
        .with_context(|| format!("abriendo el cgroup raiz {}", root.display()))?;
    let mut bpf = loader::load().context("cargando el objeto eBPF")?;
    attach::attach_all(&mut bpf, &cgroup).context("enganchando los programas al cgroup raiz")?;

    // El handle `bpf` se dropea aqui: programas y links se desenganchan.
    Ok(())
}
