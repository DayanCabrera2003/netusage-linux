//! Lectura de los mapas de contadores por cgroup y presentación de la lista por
//! aplicación.
//!
//! Responsabilidad única: traducir los mapas eBPF (inode -> bytes) en muestras
//! para el agregador y presentar la lista resultante por app con formato
//! humano. La orquestación del ciclo de vida (enganche, vigilancia, drenaje)
//! vive en `supervisor`; aquí solo se lee y se muestra.
//!
//! Los contadores son monótonos desde que se enganchan los programas y se
//! resetean al reiniciar el demonio (los mapas se recrean). La persistencia y
//! el manejo de deltas/resets corresponden a la Fase 3.

use std::collections::HashMap;

use anyhow::{Context, Result};
use aya::maps::HashMap as BpfHashMap;
use aya::maps::MapData;
use aya::Ebpf;
use netusage_common::counters::{CgroupInode, RX_MAP_NAME, TX_MAP_NAME};

use crate::fallback::{AppUsage, CgroupSample, SYSTEM_OTHER_KEY};

/// Lee ambos mapas y devuelve una muestra (rx, tx) por cada cgroup id presente
/// en cualquiera de los dos.
pub fn read_samples(bpf: &Ebpf) -> Result<Vec<CgroupSample>> {
    let rx = read_map(bpf, RX_MAP_NAME)?;
    let tx = read_map(bpf, TX_MAP_NAME)?;

    // Unir las claves de ambos mapas: un cgroup puede tener solo rx o solo tx.
    let mut inodes: Vec<CgroupInode> = rx.keys().chain(tx.keys()).copied().collect();
    inodes.sort_unstable();
    inodes.dedup();

    Ok(inodes
        .into_iter()
        .map(|inode| CgroupSample {
            inode,
            rx: rx.get(&inode).copied().unwrap_or(0),
            tx: tx.get(&inode).copied().unwrap_or(0),
        })
        .collect())
}

/// Lee los contadores (rx, tx) de un único cgroup id. Se usa para el drenaje
/// final de un cgroup que muere, antes de eliminar sus entradas.
pub fn read_inode_counters(bpf: &Ebpf, inode: CgroupInode) -> Result<(u64, u64)> {
    let rx = open_map(bpf, RX_MAP_NAME)?.get(&inode, 0).unwrap_or(0);
    let tx = open_map(bpf, TX_MAP_NAME)?.get(&inode, 0).unwrap_or(0);
    Ok((rx, tx))
}

/// Elimina las entradas de un cgroup id en ambos mapas, una vez drenadas.
///
/// Evita que el tráfico final de un cgroup muerto se siga contando como vivo o
/// recaiga en el cubo de fallback en ciclos posteriores.
pub fn remove_inode_entries(bpf: &mut Ebpf, inode: CgroupInode) -> Result<()> {
    for name in [RX_MAP_NAME, TX_MAP_NAME] {
        let mut map: BpfHashMap<&mut MapData, CgroupInode, u64> = BpfHashMap::try_from(
            bpf.map_mut(name)
                .with_context(|| format!("mapa eBPF '{name}' no encontrado"))?,
        )
        .with_context(|| format!("el mapa '{name}' no es un HashMap<u64, u64>"))?;
        // Si la entrada no existe (cgroup sin tráfico), `remove` falla: se ignora.
        let _ = map.remove(&inode);
    }
    Ok(())
}

/// Imprime la lista por aplicación: `display_name | rx | tx`, ya ordenada por el
/// agregador (mayor consumo arriba).
pub fn print_app_list(usages: &[AppUsage]) {
    println!("--- uso por aplicación ---");
    if usages.is_empty() {
        println!("(sin tráfico atribuido todavía)");
        return;
    }
    for usage in usages {
        // Marcar visualmente el cubo de fallback.
        let marker = if usage.app_key == SYSTEM_OTHER_KEY {
            "*"
        } else {
            " "
        };
        println!(
            "{marker} {:<28} rx={:>12}  tx={:>12}",
            usage.display_name,
            human_bytes(usage.rx),
            human_bytes(usage.tx),
        );
    }
}

/// Abre un mapa de contadores como `HashMap` de solo lectura.
fn open_map<'a>(bpf: &'a Ebpf, name: &str) -> Result<BpfHashMap<&'a MapData, CgroupInode, u64>> {
    BpfHashMap::try_from(
        bpf.map(name)
            .with_context(|| format!("mapa eBPF '{name}' no encontrado"))?,
    )
    .with_context(|| format!("el mapa '{name}' no es un HashMap<u64, u64>"))
}

/// Lee todas las entradas de un mapa a un `HashMap` de usuario.
///
/// Las entradas ilegibles (p. ej. una clave eliminada por el kernel entre la
/// enumeración y la lectura) se ignoran: no deben tumbar el muestreo.
fn read_map(bpf: &Ebpf, name: &str) -> Result<HashMap<CgroupInode, u64>> {
    let map = open_map(bpf, name)?;
    Ok(map.iter().filter_map(|res| res.ok()).collect())
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
