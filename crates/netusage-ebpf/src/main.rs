//! Programas eBPF (kernel-side) de netusage.
//!
//! Dos programas `cgroup_skb` (ingress y egress) que, por cada paquete IP,
//! suman su longitud al contador del cgroup al que pertenece el paquete (RX
//! para ingress, TX para egress). Nunca filtran: siempre dejan pasar el
//! paquete.
//!
//! La clave de los contadores es el cgroup id del paquete (igual al inode del
//! directorio del cgroup), de modo que el tráfico queda atribuido por
//! aplicación. Los contadores son monótonos desde que el programa se engancha y
//! se resetean al reiniciar el demonio (los mapas se recrean). La persistencia
//! y el manejo de deltas/resets corresponden a la Fase 3.

#![no_std]
#![no_main]

mod maps;
mod packet;

use aya_ebpf::{helpers::bpf_skb_cgroup_id, macros::cgroup_skb, programs::SkBuffContext};

/// Código de retorno que permite el paso del paquete (1 = permitir; 0 lo
/// bloquearía). Solo medimos, nunca bloqueamos.
const ALLOW: i32 = 1;

/// Devuelve el cgroup id del paquete: el cgroup al que pertenece el socket que
/// lo origina o recibe. Coincide con el inode del directorio del cgroup en
/// `/sys/fs/cgroup`, que es la clave con la que el espacio de usuario atribuye
/// el tráfico a una aplicación.
fn cgroup_id(ctx: &SkBuffContext) -> u64 {
    // El helper es válido en un programa cgroup_skb; el puntero `skb` proviene
    // del contexto que el kernel garantiza correcto durante la ejecución.
    unsafe { bpf_skb_cgroup_id(ctx.skb.skb) }
}

#[cgroup_skb]
pub fn netusage_ingress(ctx: SkBuffContext) -> i32 {
    if let Some(len) = packet::packet_len(&ctx) {
        maps::add_rx(cgroup_id(&ctx), len);
    }
    ALLOW
}

#[cgroup_skb]
pub fn netusage_egress(ctx: SkBuffContext) -> i32 {
    if let Some(len) = packet::packet_len(&ctx) {
        maps::add_tx(cgroup_id(&ctx), len);
    }
    ALLOW
}

#[cfg(not(test))]
#[panic_handler]
fn panic(_info: &core::panic::PanicInfo) -> ! {
    loop {}
}
