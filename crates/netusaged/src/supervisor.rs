//! Orquestación de la atribución por aplicación.
//!
//! Responsabilidad única: enganchar los programas al cgroup raíz, lanzar el
//! hilo resolver sobre el ringbuf, y en bucle leer los contadores por socket,
//! agregarlos por app y mostrar la lista.
//!
//! Flujo:
//! 1. Enganchar `cgroup_skb` (x2) y `cgroup/sock_create` a la raíz.
//! 2. Extraer el ringbuf `SOCK_BIRTH` y lanzar el resolver (`cookie -> app`).
//! 3. Bucle cada `interval`: leer los mapas, agregar por deltas (fallback para
//!    cookies sin app), podar el mapa de cookies muertas y mostrar la lista.

use std::collections::{HashMap, HashSet};
use std::path::Path;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use anyhow::{Context, Result};
use aya::maps::{MapData, RingBuf};
use aya::Ebpf;
use netusage_common::counters::{SocketCookie, SOCK_BIRTH_MAP_NAME, SOCK_DEATH_MAP_NAME};

use crate::aggregator::{read_counters, remove_cookie, Aggregator, CounterSample};
use crate::attach::attach_all;
use crate::monitor;
use crate::resolver::{spawn_resolver, CookieMap};

/// Engancha los programas y entra en el bucle de monitorización por aplicación.
///
/// Si `sampler` es `Some`, además de mostrar en vivo persiste las muestras cada
/// ciclo.
pub fn run(
    mut bpf: Ebpf,
    root: &Path,
    interval: Duration,
    mut sampler: Option<crate::sampler::Sampler>,
) -> Result<()> {
    let cgroup = std::fs::File::open(root)
        .with_context(|| format!("abriendo el cgroup raíz {}", root.display()))?;
    attach_all(&mut bpf, &cgroup)?;

    // Extraer el ringbuf del objeto (lo posee el hilo resolver) y arrancarlo.
    let ring: RingBuf<MapData> = RingBuf::try_from(
        bpf.take_map(SOCK_BIRTH_MAP_NAME)
            .with_context(|| format!("mapa eBPF '{SOCK_BIRTH_MAP_NAME}' no encontrado"))?,
    )
    .with_context(|| format!("el mapa '{SOCK_BIRTH_MAP_NAME}' no es un ring buffer"))?;
    // Ringbuf de muertes de socket, drenado en este hilo para finalizar bytes y
    // liberar entradas del mapa con exactitud.
    let mut death_ring: RingBuf<MapData> = RingBuf::try_from(
        bpf.take_map(SOCK_DEATH_MAP_NAME)
            .with_context(|| format!("mapa eBPF '{SOCK_DEATH_MAP_NAME}' no encontrado"))?,
    )
    .with_context(|| format!("el mapa '{SOCK_DEATH_MAP_NAME}' no es un ring buffer"))?;
    let cookie_map: CookieMap = Arc::new(Mutex::new(HashMap::new()));
    let _resolver = spawn_resolver(ring, Arc::clone(&cookie_map));

    // Precargar los sockets ya abiertos antes del arranque (su tráfico, si no,
    // caería en "Sistema / Otros"; típico del túnel de un VPN o conexiones
    // persistentes del navegador).
    match crate::backfill::backfill(&cookie_map) {
        Ok(n) => tracing::info!("backfill: {n} sockets preexistentes correlacionados"),
        Err(err) => tracing::warn!("backfill de sockets preexistentes falló: {err:#}"),
    }

    tracing::info!(
        "atribución por aplicación activa en {} (Ctrl-C para salir)",
        root.display()
    );

    let mut aggregator = Aggregator::new();
    loop {
        // Drenar las muertes de socket ocurridas desde el ciclo anterior. Sus
        // entradas siguen en el mapa, así que el muestreo de abajo todavía
        // contabiliza sus bytes finales; se eliminan después.
        let dead = drain_deaths(&mut death_ring);

        let samples = read_counters(&bpf).context("leyendo los mapas de contadores")?;

        let usages = {
            let map = cookie_map.lock().unwrap();
            aggregator.sample(&samples, |cookie| {
                map.get(&cookie)
                    .map(|id| (id.app_key.clone(), id.display_name.clone()))
            })
        };

        prune_cookie_map(&cookie_map, &samples);
        monitor::print_app_list(&usages);

        if let Some(sampler) = sampler.as_mut() {
            if let Err(err) = sampler.tick(&usages, chrono::Utc::now()) {
                tracing::warn!("persistencia de muestras falló: {err:#}");
            }
        }

        // Ya contabilizados: liberar los cookies muertos del mapa y del estado.
        for cookie in dead {
            aggregator.forget(cookie);
            cookie_map.lock().unwrap().remove(&cookie);
            remove_cookie(&mut bpf, cookie);
        }

        std::thread::sleep(interval);
    }
}

/// Drena el ringbuf de muertes y devuelve los cookies de los sockets cerrados.
fn drain_deaths(death_ring: &mut RingBuf<MapData>) -> Vec<SocketCookie> {
    let mut dead = Vec::new();
    while let Some(item) = death_ring.next() {
        if item.len() >= 8 {
            if let Ok(bytes) = <[u8; 8]>::try_from(&item[0..8]) {
                dead.push(u64::from_ne_bytes(bytes));
            }
        }
    }
    dead
}

/// Elimina del mapa compartido las cookies que ya no aparecen en los contadores
/// (sockets desalojados por el LRU), para que no crezca sin límite.
fn prune_cookie_map(cookie_map: &CookieMap, samples: &[CounterSample]) {
    let live: HashSet<_> = samples.iter().map(|&(cookie, _, _)| cookie).collect();
    cookie_map
        .lock()
        .unwrap()
        .retain(|cookie, _| live.contains(cookie));
}
