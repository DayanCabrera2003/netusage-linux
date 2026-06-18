//! Carga del objeto eBPF y enganche al cgroup.
//!
//! Responsabilidad única: gestionar el ciclo de vida de los programas eBPF
//! (cargar el objeto, engancharlo a ingress y egress del cgroup). Mientras el
//! `Ebpf` devuelto siga vivo, los programas permanecen enganchados; al
//! dropearlo, se desenganchan automáticamente.

use std::os::fd::AsFd;

use anyhow::{Context, Result};
use aya::programs::{CgroupAttachMode, CgroupSkb, CgroupSkbAttachType};
use aya::Ebpf;

/// Objeto eBPF compilado y embebido en tiempo de compilación. La ruta la fija
/// `build.rs` en la variable de entorno `NETUSAGE_EBPF_OBJ`.
fn ebpf_object() -> &'static [u8] {
    aya::include_bytes_aligned!(env!("NETUSAGE_EBPF_OBJ"))
}

/// Carga el objeto eBPF y engancha los programas ingress y egress al `cgroup`.
///
/// Devuelve el handle `Ebpf`, que el llamador debe mantener vivo mientras quiera
/// seguir contabilizando tráfico.
pub fn load_and_attach<T: AsFd>(cgroup: T) -> Result<Ebpf> {
    let mut bpf = Ebpf::load(ebpf_object()).context("cargando el objeto eBPF")?;

    attach_program(
        &mut bpf,
        "netusage_ingress",
        cgroup.as_fd(),
        CgroupSkbAttachType::Ingress,
    )?;
    attach_program(
        &mut bpf,
        "netusage_egress",
        cgroup.as_fd(),
        CgroupSkbAttachType::Egress,
    )?;

    Ok(bpf)
}

/// Carga y engancha un único programa `cgroup_skb` por su nombre.
fn attach_program(
    bpf: &mut Ebpf,
    name: &str,
    cgroup: impl AsFd,
    attach_type: CgroupSkbAttachType,
) -> Result<()> {
    let program: &mut CgroupSkb = bpf
        .program_mut(name)
        .with_context(|| format!("programa eBPF '{name}' no encontrado en el objeto"))?
        .try_into()
        .with_context(|| format!("'{name}' no es un programa cgroup_skb"))?;

    program.load().with_context(|| {
        format!(
            "cargando '{name}' en el kernel (¿faltan privilegios? se necesita root o \
                 CAP_BPF+CAP_PERFMON+CAP_NET_ADMIN; ver `netusaged --check`)"
        )
    })?;

    program
        .attach(cgroup, attach_type, CgroupAttachMode::Single)
        .with_context(|| format!("enganchando '{name}' al cgroup"))?;

    Ok(())
}
