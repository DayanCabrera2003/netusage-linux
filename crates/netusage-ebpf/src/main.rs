//! Programas eBPF (kernel-side) de netusage.
//!
//! Esqueleto de la Fase 0: solo valida que la cadena de compilacion al target
//! BPF funciona. El programa trivial se anade en el commit siguiente.

#![no_std]
#![no_main]

#[cfg(not(test))]
#[panic_handler]
fn panic(_info: &core::panic::PanicInfo) -> ! {
    loop {}
}
