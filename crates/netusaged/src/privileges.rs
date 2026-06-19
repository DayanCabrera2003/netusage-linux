//! Modelo de privilegios del demonio: capabilities en vez de root pleno.
//!
//! Responsabilidad única: decidir y verificar con qué privilegios corre el
//! demonio. En kernels >= 5.8 basta el trío `CAP_BPF` + `CAP_PERFMON` +
//! `CAP_NET_ADMIN` (concedido por la unit systemd); en kernels anteriores esas
//! capabilities no existen y se requiere root (o `CAP_SYS_ADMIN`).
//!
//! `ensure_minimum` falla con un mensaje accionable si no se cumple el mínimo,
//! para no llegar a intentar cargar eBPF y obtener un error opaco.

use anyhow::{bail, Result};
use caps::{CapSet, Capability};

/// Modo de privilegios efectivo con el que arranca el demonio.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PrivilegeMode {
    /// Conjunto acotado de capabilities (kernel >= 5.8).
    Capabilities,
    /// Root pleno (fallback en kernel < 5.8).
    Root,
}

impl PrivilegeMode {
    /// Descripción legible para el log de arranque.
    pub fn describe(self) -> &'static str {
        match self {
            PrivilegeMode::Capabilities => "capabilities (CAP_BPF, CAP_PERFMON, CAP_NET_ADMIN)",
            PrivilegeMode::Root => "root (fallback de kernel < 5.8)",
        }
    }
}

/// Verifica que el proceso tenga el privilegio mínimo para cargar eBPF y
/// devuelve el modo detectado, o un error accionable.
pub fn ensure_minimum() -> Result<PrivilegeMode> {
    let is_root = rustix::process::geteuid().is_root();

    if kernel_supports_bpf_caps() {
        if has(Capability::CAP_BPF)
            && has(Capability::CAP_PERFMON)
            && has(Capability::CAP_NET_ADMIN)
        {
            return Ok(PrivilegeMode::Capabilities);
        }
        if is_root {
            return Ok(PrivilegeMode::Root);
        }
        let missing = missing_caps();
        bail!(
            "faltan capabilities para cargar eBPF: {missing}. Concédelas con la unit \
             systemd (AmbientCapabilities=CAP_BPF CAP_PERFMON CAP_NET_ADMIN) o ejecuta \
             como root."
        );
    }

    // Kernel < 5.8: CAP_BPF/CAP_PERFMON no existen; se necesita root o CAP_SYS_ADMIN.
    if is_root || (has(Capability::CAP_SYS_ADMIN) && has(Capability::CAP_NET_ADMIN)) {
        return Ok(PrivilegeMode::Root);
    }
    bail!(
        "kernel < 5.8: se necesita root (o CAP_SYS_ADMIN + CAP_NET_ADMIN) para cargar eBPF; \
         este kernel no soporta el modelo de capabilities CAP_BPF/CAP_PERFMON."
    );
}

/// Indica si el proceso tiene `cap` en su conjunto efectivo.
fn has(cap: Capability) -> bool {
    caps::has_cap(None, CapSet::Effective, cap).unwrap_or(false)
}

/// Lista legible de las capabilities del trío que faltan.
fn missing_caps() -> String {
    let mut missing = Vec::new();
    if !has(Capability::CAP_BPF) {
        missing.push("CAP_BPF");
    }
    if !has(Capability::CAP_PERFMON) {
        missing.push("CAP_PERFMON");
    }
    if !has(Capability::CAP_NET_ADMIN) {
        missing.push("CAP_NET_ADMIN");
    }
    missing.join(", ")
}

/// Indica si el kernel soporta el modelo de capabilities para eBPF (>= 5.8).
fn kernel_supports_bpf_caps() -> bool {
    kernel_version()
        .map(|(maj, min)| (maj, min) >= (5, 8))
        .unwrap_or(false)
}

/// Lee la versión `(mayor, menor)` del kernel de `/proc/sys/kernel/osrelease`.
fn kernel_version() -> Option<(u32, u32)> {
    let release = std::fs::read_to_string("/proc/sys/kernel/osrelease").ok()?;
    let mut parts = release.trim().split(['.', '-']);
    let major = parts.next()?.parse().ok()?;
    let minor = parts.next()?.parse().ok()?;
    Some((major, minor))
}
