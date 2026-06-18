//! Programas eBPF (kernel-side) de netusage.
//!
//! Dos programas `cgroup_skb` (ingress y egress) que, por cada paquete IP,
//! suman su longitud al contador correspondiente (RX para ingress, TX para
//! egress). Nunca filtran: siempre dejan pasar el paquete.
//!
//! El contador es monótono desde que el programa se engancha y se resetea al
//! reiniciar el demonio (el mapa se recrea). La persistencia y el manejo de
//! deltas/resets corresponden a la Fase 3.

#![no_std]
#![no_main]

mod maps;
mod packet;

use aya_ebpf::{macros::cgroup_skb, programs::SkBuffContext};

/// Código de retorno que permite el paso del paquete (1 = permitir; 0 lo
/// bloquearía). Solo medimos, nunca bloqueamos.
const ALLOW: i32 = 1;

#[cgroup_skb]
pub fn netusage_ingress(ctx: SkBuffContext) -> i32 {
    if let Some(len) = packet::packet_len(&ctx) {
        maps::add_rx(len);
    }
    ALLOW
}

#[cgroup_skb]
pub fn netusage_egress(ctx: SkBuffContext) -> i32 {
    if let Some(len) = packet::packet_len(&ctx) {
        maps::add_tx(len);
    }
    ALLOW
}

#[cfg(not(test))]
#[panic_handler]
fn panic(_info: &core::panic::PanicInfo) -> ! {
    loop {}
}
