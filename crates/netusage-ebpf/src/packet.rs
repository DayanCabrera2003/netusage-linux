//! Extracción de la longitud del paquete para contabilizar bytes.
//!
//! Definición de "byte" (Fase 1): se cuenta la longitud total del paquete a
//! nivel IP (`SkBuffContext::len`). En el hook `cgroup_skb` el paquete empieza
//! en la cabecera IP (capa 3), de modo que esa longitud incluye la cabecera IP
//! y el payload de transporte, pero no la cabecera Ethernet de capa de enlace.
//! Se elige así para que el total sea comparable al modelo de Android (medición
//! a nivel transporte/IP), con la limitación conocida de que no cuadra al byte
//! con la factura del ISP (overhead de enlace, retransmisiones).
//!
//! Loopback: en Fase 1 NO se filtra (se cuenta). Su tratamiento se reevaluará
//! en la Fase 2.

use aya_ebpf::programs::SkBuffContext;

/// Versión IP en el primer nibble de la cabecera (IPv4 = 4, IPv6 = 6).
const IP_VERSION_4: u8 = 4;
const IP_VERSION_6: u8 = 6;

/// Devuelve la longitud del paquete si es IPv4 o IPv6; `None` para otros
/// protocolos (ARP, etc.), que el llamador ignora.
pub fn packet_len(ctx: &SkBuffContext) -> Option<u64> {
    // El primer byte de la cabecera IP contiene la versión en el nibble alto.
    let first_byte: u8 = ctx.load(0).ok()?;
    let version = first_byte >> 4;

    // TODO Fase 2: evaluar descarte de loopback por ifindex.
    match version {
        IP_VERSION_4 | IP_VERSION_6 => Some(u64::from(ctx.len())),
        _ => None,
    }
}
