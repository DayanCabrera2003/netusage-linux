//! Enganche y desenganche dinámico de los programas `cgroup_skb` a un cgroup
//! concreto.
//!
//! Responsabilidad única: gestionar los links eBPF de un cgroup. Los dos
//! programas (`netusage_ingress`, `netusage_egress`) se cargan una sola vez en
//! el kernel (`load_programs`) y luego se enganchan a tantos cgroups como haga
//! falta (`attach_cgroup`), guardando los identificadores de link para poder
//! desengancharlos después (`detach_cgroup`).
//!
//! Se usa `CgroupAttachMode::AllowMultiple` (no `Single`): el escritorio y
//! systemd ya pueden tener programas BPF enganchados en cgroups padre, y este
//! modo permite coexistir sin desplazarlos. Nota: el plan lo llamaba `Multi`;
//! en aya 0.13.x el variant es `AllowMultiple` (ver desviaciones).

use anyhow::{Context, Result};
use aya::programs::cgroup_skb::CgroupSkbLinkId;
use aya::programs::{CgroupAttachMode, CgroupSkb, CgroupSkbAttachType};
use aya::Ebpf;
use std::os::fd::AsFd;

/// Nombre del programa de entrada (ingress) en el objeto eBPF.
const INGRESS: &str = "netusage_ingress";
/// Nombre del programa de salida (egress) en el objeto eBPF.
const EGRESS: &str = "netusage_egress";

/// Links eBPF de un cgroup enganchado: uno por dirección.
///
/// Se guardan para poder desenganchar exactamente estos links cuando el cgroup
/// muere, sin afectar a los de otros cgroups.
pub struct AttachedLinks {
    ingress: CgroupSkbLinkId,
    egress: CgroupSkbLinkId,
}

/// Carga en el kernel los dos programas `cgroup_skb`.
///
/// Debe llamarse una sola vez tras cargar el objeto eBPF y antes del primer
/// `attach_cgroup`. Cargar un programa dos veces es un error en aya, por eso la
/// carga se separa del enganche.
pub fn load_programs(bpf: &mut Ebpf) -> Result<()> {
    load_one(bpf, INGRESS)?;
    load_one(bpf, EGRESS)?;
    Ok(())
}

/// Carga un único programa `cgroup_skb` por su nombre.
fn load_one(bpf: &mut Ebpf, name: &str) -> Result<()> {
    let program: &mut CgroupSkb = program_mut(bpf, name)?;
    program.load().with_context(|| {
        format!(
            "cargando '{name}' en el kernel (¿faltan privilegios? se necesita root o \
             CAP_BPF+CAP_PERFMON+CAP_NET_ADMIN; ver `netusaged --check`)"
        )
    })?;
    Ok(())
}

/// Engancha los programas ingress y egress (ya cargados) al cgroup cuyo
/// descriptor es `cgroup_fd`, devolviendo los links para un desenganche
/// posterior.
pub fn attach_cgroup<T: AsFd>(bpf: &mut Ebpf, cgroup_fd: T) -> Result<AttachedLinks> {
    let fd = cgroup_fd.as_fd();
    let ingress = attach_one(bpf, INGRESS, fd, CgroupSkbAttachType::Ingress)?;
    let egress = attach_one(bpf, EGRESS, fd, CgroupSkbAttachType::Egress)?;
    Ok(AttachedLinks { ingress, egress })
}

/// Engancha un único programa al cgroup y devuelve su id de link.
fn attach_one(
    bpf: &mut Ebpf,
    name: &str,
    cgroup_fd: impl AsFd,
    attach_type: CgroupSkbAttachType,
) -> Result<CgroupSkbLinkId> {
    let program: &mut CgroupSkb = program_mut(bpf, name)?;
    program
        .attach(cgroup_fd, attach_type, CgroupAttachMode::AllowMultiple)
        .with_context(|| format!("enganchando '{name}' al cgroup"))
}

/// Desengancha los links de un cgroup previamente enganchado con
/// `attach_cgroup`.
///
/// Desengancha ambas direcciones aunque una falle, para no dejar links
/// colgados; devuelve el primer error encontrado.
pub fn detach_cgroup(bpf: &mut Ebpf, links: AttachedLinks) -> Result<()> {
    let ingress = detach_one(bpf, INGRESS, links.ingress);
    let egress = detach_one(bpf, EGRESS, links.egress);
    ingress.and(egress)
}

/// Desengancha un único link por su id.
fn detach_one(bpf: &mut Ebpf, name: &str, link: CgroupSkbLinkId) -> Result<()> {
    let program: &mut CgroupSkb = program_mut(bpf, name)?;
    program
        .detach(link)
        .with_context(|| format!("desenganchando '{name}' del cgroup"))
}

/// Obtiene una referencia mutable al programa `cgroup_skb` `name` del objeto.
fn program_mut<'a>(bpf: &'a mut Ebpf, name: &str) -> Result<&'a mut CgroupSkb> {
    bpf.program_mut(name)
        .with_context(|| format!("programa eBPF '{name}' no encontrado en el objeto"))?
        .try_into()
        .with_context(|| format!("'{name}' no es un programa cgroup_skb"))
}
