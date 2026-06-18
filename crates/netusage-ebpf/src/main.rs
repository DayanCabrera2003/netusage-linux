//! Programas eBPF (kernel-side) de netusage.
//!
//! Fase 0: programa `cgroup_skb` trivial que solo valida la cadena de
//! compilacion y carga. No mide nada todavia: siempre deja pasar el paquete.
//! La medicion de bytes por cgroup se implementa en la Fase 1.

#![no_std]
#![no_main]

mod maps;

use aya_ebpf::{macros::cgroup_skb, programs::SkBuffContext};

/// Codigo de retorno que permite el paso del paquete (no se filtra nada).
const ALLOW: i32 = 1;

#[cgroup_skb]
pub fn netusage_ingress(_ctx: SkBuffContext) -> i32 {
    ALLOW
}

#[cgroup_skb]
pub fn netusage_egress(_ctx: SkBuffContext) -> i32 {
    ALLOW
}

#[cfg(not(test))]
#[panic_handler]
fn panic(_info: &core::panic::PanicInfo) -> ! {
    loop {}
}
