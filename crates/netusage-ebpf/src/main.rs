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

use aya_ebpf::{
    helpers::{bpf_get_current_pid_tgid, bpf_get_socket_cookie},
    macros::{cgroup_skb, cgroup_sock},
    programs::{SkBuffContext, SockContext},
};

/// Código de retorno que permite el paso (1 = permitir; 0 bloquearía). Solo
/// medimos, nunca bloqueamos ni rechazamos la creación de sockets.
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

/// Corre en el contexto del proceso que crea un socket. Publica `(cookie, pid)`
/// en el ringbuf para que el espacio de usuario resuelva el ejecutable dueño.
/// El `pid` es el tgid (PID de proceso, el que usa `/proc/<pid>`): los 32 bits
/// altos de `bpf_get_current_pid_tgid`.
#[cgroup_sock(sock_create)]
pub fn netusage_sock_create(ctx: SockContext) -> i32 {
    let cookie = unsafe { bpf_get_socket_cookie(ctx.sock as *mut _) };
    let pid = (unsafe { bpf_get_current_pid_tgid() } >> 32) as u32;
    maps::emit_sock_birth(cookie, pid);
    ALLOW
}

/// Corre cuando un socket se cierra. Publica su cookie para que el espacio de
/// usuario finalice sus bytes y libere la entrada del mapa con exactitud.
#[cgroup_sock(sock_release)]
pub fn netusage_sock_release(ctx: SockContext) -> i32 {
    let cookie = unsafe { bpf_get_socket_cookie(ctx.sock as *mut _) };
    maps::emit_sock_death(cookie);
    ALLOW
}

#[cfg(not(test))]
#[panic_handler]
fn panic(_info: &core::panic::PanicInfo) -> ! {
    loop {}
}
