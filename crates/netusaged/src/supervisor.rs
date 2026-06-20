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
use netusage_common::counters::{SocketCookie, SOCK_BIRTH_MAP_NAME};

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
    // Cookies que han aparecido en al menos un muestreo eBPF. Solo estas se
    // podan del cookie_map cuando desaparecen de los contadores (socket
    // cerrado). Las cookies del backfill o de sock_create que aún no han
    // generado tráfico se conservan hasta que se confirmen.
    let mut seen_in_ebpf: HashSet<SocketCookie> = HashSet::new();
    loop {
        let samples = read_counters(&bpf).context("leyendo los mapas de contadores")?;

        let usages = {
            let map = cookie_map.lock().unwrap();
            aggregator.sample(&samples, |cookie| {
                map.get(&cookie)
                    .map(|id| (id.app_key.clone(), id.display_name.clone()))
            })
        };

        prune_cookie_map(&cookie_map, &samples, &mut seen_in_ebpf);
        monitor::print_app_list(&usages);

        if let Some(sampler) = sampler.as_mut() {
            if let Err(err) = sampler.tick(&usages, chrono::Utc::now()) {
                tracing::warn!("persistencia de muestras falló: {err:#}");
            }
        }

        std::thread::sleep(interval);
    }
}

/// Elimina del mapa compartido las cookies que ya no aparecen en los
/// contadores (sockets desalojados por el LRU o cerrados), pero solo si ya
/// habían aparecido en algún muestreo anterior (`seen_in_ebpf`).
///
/// Las cookies precargadas por el backfill o registradas por `sock_create`
/// que aún no han generado tráfico medible no se podan, porque el socket
/// puede estar simplemente inactivo: si se podasen y luego generasen tráfico,
/// ese tráfico caería en "Sistema / Otros" sin atribución.
fn prune_cookie_map(
    cookie_map: &CookieMap,
    samples: &[CounterSample],
    seen_in_ebpf: &mut HashSet<SocketCookie>,
) {
    let live: HashSet<SocketCookie> = samples.iter().map(|&(c, _, _)| c).collect();
    // Marcar como confirmadas las cookies activas en este muestreo.
    seen_in_ebpf.extend(live.iter().copied());
    // Podar solo las cookies confirmadas que ya no están en la muestra.
    let mut map = cookie_map.lock().unwrap();
    map.retain(|cookie, _| !seen_in_ebpf.contains(cookie) || live.contains(cookie));
    // Limpiar seen_in_ebpf para las cookies que acaban de podarse, para que
    // el set no crezca sin límite a lo largo de sesiones largas.
    seen_in_ebpf.retain(|c| map.contains_key(c));
}
