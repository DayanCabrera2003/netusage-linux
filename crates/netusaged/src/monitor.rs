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
use aya::maps::Array;
use aya::Ebpf;
use netusage_common::counters::{COUNTER_RX, COUNTER_TX};

/// Lee el mapa de contadores cada `interval` e imprime el total y el delta.
///
/// El bucle se ejecuta hasta que el proceso recibe una señal de terminación
/// (Ctrl-C); en ese momento el kernel desengancha los programas al cerrarse los
/// descriptores.
pub fn run_monitor(bpf: &Ebpf, interval: Duration) -> Result<()> {
    let traffic: Array<_, u64> =
        Array::try_from(bpf.map("TRAFFIC").context("mapa eBPF 'TRAFFIC' no encontrado")?)
            .context("el mapa 'TRAFFIC' no es un Array<u64>")?;

    let mut prev_rx = 0u64;
    let mut prev_tx = 0u64;

    loop {
        let rx = traffic.get(&COUNTER_RX, 0).context("leyendo contador RX")?;
        let tx = traffic.get(&COUNTER_TX, 0).context("leyendo contador TX")?;

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
