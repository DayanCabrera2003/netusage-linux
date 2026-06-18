//! Muestreo del mapa de contadores y presentación en vivo.
//!
//! Responsabilidad única: leer periódicamente el mapa eBPF de contadores e
//! imprimir el total rx/tx (y el incremento respecto a la lectura anterior)
//! con formato humano.
//!
//! Los contadores son monótonos desde que se enganchan los programas y se
//! resetean al reiniciar el demonio (el mapa se recrea). La persistencia y el
//! manejo de deltas/resets corresponden a la Fase 3.

use std::time::Duration;

use anyhow::{Context, Result};
use aya::maps::HashMap as BpfHashMap;
use aya::Ebpf;
use netusage_common::counters::{CgroupInode, RX_MAP_NAME, TX_MAP_NAME};

/// Lee los mapas de contadores por cgroup cada `interval` e imprime el total de
/// la máquina (suma de todas las entradas) y el delta respecto a la lectura
/// anterior.
///
/// Esta vista de total es un puente: los contadores ya están indexados por
/// cgroup (Fase 2), pero la atribución por aplicación con su lista en vivo se
/// arma en commits posteriores. Sumar todas las entradas reproduce el total de
/// máquina de la Fase 1 sobre el nuevo modelo de datos.
///
/// El bucle se ejecuta hasta que el proceso recibe una señal de terminación
/// (Ctrl-C); en ese momento el kernel desengancha los programas al cerrarse los
/// descriptores.
pub fn run_monitor(bpf: &Ebpf, interval: Duration) -> Result<()> {
    let rx_map: BpfHashMap<_, CgroupInode, u64> = BpfHashMap::try_from(
        bpf.map(RX_MAP_NAME)
            .with_context(|| format!("mapa eBPF '{RX_MAP_NAME}' no encontrado"))?,
    )
    .with_context(|| format!("el mapa '{RX_MAP_NAME}' no es un HashMap<u64, u64>"))?;
    let tx_map: BpfHashMap<_, CgroupInode, u64> = BpfHashMap::try_from(
        bpf.map(TX_MAP_NAME)
            .with_context(|| format!("mapa eBPF '{TX_MAP_NAME}' no encontrado"))?,
    )
    .with_context(|| format!("el mapa '{TX_MAP_NAME}' no es un HashMap<u64, u64>"))?;

    let mut prev_rx = 0u64;
    let mut prev_tx = 0u64;

    loop {
        let rx = sum_map(&rx_map);
        let tx = sum_map(&tx_map);

        println!(
            "rx={} (Δ {}) tx={} (Δ {})",
            human_bytes(rx),
            human_bytes(rx.saturating_sub(prev_rx)),
            human_bytes(tx),
            human_bytes(tx.saturating_sub(prev_tx)),
        );

        prev_rx = rx;
        prev_tx = tx;
        std::thread::sleep(interval);
    }
}

/// Suma los valores de todas las entradas de un mapa de contadores por cgroup.
///
/// Las entradas ilegibles (p. ej. una clave eliminada por otro hilo entre la
/// enumeración y la lectura) se ignoran: no deben tumbar el muestreo.
fn sum_map(map: &BpfHashMap<&aya::maps::MapData, CgroupInode, u64>) -> u64 {
    map.iter()
        .filter_map(|res| res.ok())
        .map(|(_inode, bytes)| bytes)
        .sum()
}

/// Formatea una cantidad de bytes en unidades binarias legibles.
///
/// Por debajo de 1 KiB se muestra el valor exacto en bytes; a partir de ahí se
/// usan dos decimales (KiB/MiB/GiB/TiB).
pub fn human_bytes(bytes: u64) -> String {
    const UNITS: [&str; 5] = ["B", "KiB", "MiB", "GiB", "TiB"];

    if bytes < 1024 {
        return format!("{bytes} B");
    }

    let mut value = bytes as f64;
    let mut unit = 0;
    while value >= 1024.0 && unit < UNITS.len() - 1 {
        value /= 1024.0;
        unit += 1;
    }

    format!("{value:.2} {}", UNITS[unit])
}

#[cfg(test)]
mod tests {
    use super::human_bytes;

    #[test]
    fn formats_bytes_in_binary_units() {
        assert_eq!(human_bytes(0), "0 B");
        assert_eq!(human_bytes(1023), "1023 B");
        assert_eq!(human_bytes(1024), "1.00 KiB");
        assert_eq!(human_bytes(1_048_576), "1.00 MiB");
        assert_eq!(human_bytes(1_073_741_824), "1.00 GiB");
    }
}
