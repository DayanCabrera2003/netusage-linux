//! Carga del objeto eBPF en el kernel.
//!
//! Responsabilidad única: embeber el objeto eBPF compilado y cargarlo,
//! devolviendo el handle `Ebpf`. El enganche de los programas a cada cgroup de
//! aplicación lo gestiona el módulo `attach` (carga de programas) y el
//! `supervisor` (enganche/desenganche por cgroup). Mientras el `Ebpf` devuelto
//! siga vivo, los programas y mapas permanecen en el kernel; al dropearlo se
//! liberan.

use anyhow::{Context, Result};
use aya::Ebpf;

/// Objeto eBPF compilado y embebido en tiempo de compilación. La ruta la fija
/// `build.rs` en la variable de entorno `NETUSAGE_EBPF_OBJ`.
fn ebpf_object() -> &'static [u8] {
    aya::include_bytes_aligned!(env!("NETUSAGE_EBPF_OBJ"))
}

/// Carga el objeto eBPF y devuelve el handle.
///
/// No engancha ningún programa: eso corresponde a `attach::load_programs` (una
/// vez) y `attach::attach_cgroup` (por cada cgroup de app).
pub fn load() -> Result<Ebpf> {
    Ebpf::load(ebpf_object()).context(
        "cargando el objeto eBPF (¿faltan privilegios? se necesita root o \
         CAP_BPF+CAP_PERFMON+CAP_NET_ADMIN; ver `netusaged --check`)",
    )
}
