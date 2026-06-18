//! Mapa eBPF donde se acumulan los bytes de tráfico.
//!
//! Responsabilidad única: declarar el mapa de contadores y exponer helpers
//! para sumar bytes en las entradas de recibidos (RX) y enviados (TX).

use aya_ebpf::{macros::map, maps::Array};
use netusage_common::counters::{COUNTER_RX, COUNTER_TX, TRAFFIC_MAP_ENTRIES};

/// Mapa con dos entradas (RX y TX) que acumulan el total de bytes de la
/// máquina. El espacio de usuario lo lee por las mismas claves.
#[map]
static TRAFFIC: Array<u64> = Array::with_max_entries(TRAFFIC_MAP_ENTRIES, 0);

/// Acumula `bytes` en la entrada de recibidos (ingress).
pub fn add_rx(bytes: u64) {
    add(COUNTER_RX, bytes);
}

/// Acumula `bytes` en la entrada de enviados (egress).
pub fn add_tx(bytes: u64) {
    add(COUNTER_TX, bytes);
}

/// Suma `bytes` a la entrada `key` del mapa.
///
/// La suma no es atómica. Para un contador de medición es aceptable (igual que
/// el ejemplo HBM del kernel): puede perderse alguna cuenta por carreras entre
/// CPUs, pero no afecta de forma significativa al total. Si se observara
/// pérdida apreciable, evaluar una operación atómica.
fn add(key: u32, bytes: u64) {
    if let Some(ptr) = TRAFFIC.get_ptr_mut(key) {
        unsafe {
            *ptr += bytes;
        }
    }
}
