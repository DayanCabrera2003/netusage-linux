//! Carga y enganche de los programas eBPF al cgroup v2 raíz.
//!
//! Responsabilidad única: cargar y enganchar los tres programas
//! (`netusage_ingress`, `netusage_egress`, `netusage_sock_create`) a un único
//! cgroup (la raíz), para ver todo el tráfico de la máquina. Mientras el `Ebpf`
//! siga vivo, los programas y links permanecen; al dropearlo se desenganchan.
//!
//! Modo de enganche: en kernels modernos (>= 5.7) aya engancha vía
//! `bpf_link_create`. Para los links de cgroup el kernel exige `flags == 0`
//! (`cgroup_bpf_link_attach` devuelve EINVAL con cualquier flag), de modo que
//! `CgroupAttachMode::AllowMultiple` falla. Se usa `CgroupAttachMode::Single`,
//! que pasa `flags == 0`: los bpf_links coexisten por naturaleza con los
//! programas que systemd pueda tener en cgroups padre. Ver
//! documentacion/desviaciones/fase-2.md.

use std::os::fd::AsFd;

use anyhow::{Context, Result};
use aya::programs::{CgroupAttachMode, CgroupSkb, CgroupSkbAttachType, CgroupSock};
use aya::Ebpf;

const INGRESS: &str = "netusage_ingress";
const EGRESS: &str = "netusage_egress";
const SOCK_CREATE: &str = "netusage_sock_create";
const SOCK_RELEASE: &str = "netusage_sock_release";

/// Pista a mostrar ante un fallo por falta de privilegios.
const PRIV_HINT: &str = "(¿faltan privilegios? se necesita root o \
     CAP_BPF+CAP_PERFMON+CAP_NET_ADMIN; ver `netusaged --check`)";

/// Carga y engancha los programas al `cgroup` (la raíz de cgroup v2).
pub fn attach_all<T: AsFd>(bpf: &mut Ebpf, cgroup: T) -> Result<()> {
    let fd = cgroup.as_fd();
    attach_skb(bpf, INGRESS, fd, CgroupSkbAttachType::Ingress)?;
    attach_skb(bpf, EGRESS, fd, CgroupSkbAttachType::Egress)?;
    attach_sock(bpf, SOCK_CREATE, fd)?;
    attach_sock(bpf, SOCK_RELEASE, fd)?;
    Ok(())
}

/// Carga y engancha un programa `cgroup_skb` (ingress o egress) al cgroup.
fn attach_skb(
    bpf: &mut Ebpf,
    name: &str,
    cgroup: impl AsFd,
    attach_type: CgroupSkbAttachType,
) -> Result<()> {
    let program: &mut CgroupSkb = bpf
        .program_mut(name)
        .with_context(|| format!("programa eBPF '{name}' no encontrado"))?
        .try_into()
        .with_context(|| format!("'{name}' no es un programa cgroup_skb"))?;
    program
        .load()
        .with_context(|| format!("cargando '{name}' {PRIV_HINT}"))?;
    program
        .attach(cgroup, attach_type, CgroupAttachMode::Single)
        .with_context(|| format!("enganchando '{name}' al cgroup raíz"))?;
    Ok(())
}

/// Carga y engancha un programa `cgroup/sock` (sock_create o sock_release) al
/// cgroup. El tipo de enganche lo fija la sección del programa.
fn attach_sock(bpf: &mut Ebpf, name: &str, cgroup: impl AsFd) -> Result<()> {
    let program: &mut CgroupSock = bpf
        .program_mut(name)
        .with_context(|| format!("programa eBPF '{name}' no encontrado"))?
        .try_into()
        .with_context(|| format!("'{name}' no es un programa cgroup_sock"))?;
    program
        .load()
        .with_context(|| format!("cargando '{name}' {PRIV_HINT}"))?;
    program
        .attach(cgroup, CgroupAttachMode::Single)
        .with_context(|| format!("enganchando '{name}' al cgroup raíz"))?;
    Ok(())
}
