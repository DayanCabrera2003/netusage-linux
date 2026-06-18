//! Mapas eBPF donde se acumulan los bytes de tráfico por cgroup.
//!
//! Responsabilidad única: declarar los mapas de contadores indexados por cgroup
//! id y exponer helpers para sumar bytes a la entrada de un cgroup concreto en
//! las direcciones de recibidos (RX) y enviados (TX).
//!
//! A diferencia de la Fase 1 (un único `Array<u64>` con el total de la
//! máquina), aquí la clave es el cgroup id del paquete, de modo que cada
//! aplicación (cada scope de cgroup) tiene su propia entrada y el espacio de
//! usuario puede atribuir el tráfico a la app correspondiente.

use aya_ebpf::{macros::map, maps::HashMap};
use netusage_common::counters::{CgroupInode, TRAFFIC_MAP_CAPACITY};

/// Bandera de `insert`: 0 equivale a `BPF_ANY` (crear o sobrescribir).
const BPF_ANY: u64 = 0;

/// Bytes recibidos (ingress) acumulados por cgroup id.
#[map(name = "RX_BYTES")]
static RX_BYTES: HashMap<CgroupInode, u64> = HashMap::with_max_entries(TRAFFIC_MAP_CAPACITY, 0);

/// Bytes enviados (egress) acumulados por cgroup id.
#[map(name = "TX_BYTES")]
static TX_BYTES: HashMap<CgroupInode, u64> = HashMap::with_max_entries(TRAFFIC_MAP_CAPACITY, 0);

/// Acumula `bytes` en la entrada de recibidos (ingress) del cgroup `cgroup_id`.
pub fn add_rx(cgroup_id: CgroupInode, bytes: u64) {
    add(&RX_BYTES, cgroup_id, bytes);
}

/// Acumula `bytes` en la entrada de enviados (egress) del cgroup `cgroup_id`.
pub fn add_tx(cgroup_id: CgroupInode, bytes: u64) {
    add(&TX_BYTES, cgroup_id, bytes);
}

/// Suma `bytes` a la entrada `cgroup_id` del mapa `counters`.
///
/// Si la entrada ya existe, se incrementa en sitio (no atómico, aceptable para
/// un contador de medición igual que en Fase 1). Si no existe, se inserta con
/// el valor inicial. La inserción puede fallar si el mapa está lleno; en ese
/// caso se descarta la cuenta de este paquete (situación rara con la capacidad
/// configurada; el espacio de usuario lo notaría como tráfico no contabilizado).
fn add(counters: &HashMap<CgroupInode, u64>, cgroup_id: CgroupInode, bytes: u64) {
    if let Some(ptr) = counters.get_ptr_mut(&cgroup_id) {
        unsafe {
            *ptr += bytes;
        }
    } else {
        // Primera vez que vemos este cgroup: crear su entrada.
        let _ = counters.insert(&cgroup_id, &bytes, BPF_ANY);
    }
}
