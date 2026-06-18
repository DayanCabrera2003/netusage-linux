//! Programas eBPF (kernel-side) de netusage.
//!
//! Dos programas `cgroup_skb` (ingress y egress) que, por cada paquete IP,
//! suman su longitud al contador del socket al que pertenece el paquete (RX
//! para ingress, TX para egress). Nunca filtran: siempre dejan pasar el
//! paquete.
//!
//! La clave de los contadores es el socket cookie del paquete
//! (`bpf_get_socket_cookie`), de modo que el espacio de usuario puede atribuir
//! el tráfico al ejecutable del proceso dueño del socket. Los contadores son
//! monótonos desde que el programa se engancha y se resetean al reiniciar el
//! demonio (los mapas se recrean). La persistencia corresponde a la Fase 3.

#![no_std]
#![no_main]

mod maps;
mod packet;

use aya_ebpf::{helpers::bpf_get_socket_cookie, macros::cgroup_skb, programs::SkBuffContext};

/// Código de retorno que permite el paso del paquete (1 = permitir; 0 lo
/// bloquearía). Solo medimos, nunca bloqueamos.
const ALLOW: i32 = 1;

/// Devuelve el socket cookie del paquete: un id único y estable del socket que
/// lo origina o recibe. Es la clave con la que el espacio de usuario atribuye el
/// tráfico al ejecutable del proceso dueño del socket.
fn socket_cookie(ctx: &SkBuffContext) -> u64 {
    // El helper toma un puntero de contexto; en `cgroup_skb` es el `__sk_buff`.
    unsafe { bpf_get_socket_cookie(ctx.skb.skb as *mut _) }
}

#[cgroup_skb]
pub fn netusage_ingress(ctx: SkBuffContext) -> i32 {
    if let Some(len) = packet::packet_len(&ctx) {
        maps::add_rx(socket_cookie(&ctx), len);
    }
    ALLOW
}

#[cgroup_skb]
pub fn netusage_egress(ctx: SkBuffContext) -> i32 {
    if let Some(len) = packet::packet_len(&ctx) {
        maps::add_tx(socket_cookie(&ctx), len);
    }
    ALLOW
}

#[cfg(not(test))]
#[panic_handler]
fn panic(_info: &core::panic::PanicInfo) -> ! {
    loop {}
}
