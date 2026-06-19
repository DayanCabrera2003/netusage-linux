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
use netusage_common::counters::SOCK_BIRTH_MAP_NAME;

use crate::aggregator::{read_counters, Aggregator, CounterSample};
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

        std::thread::sleep(interval);
    }
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
