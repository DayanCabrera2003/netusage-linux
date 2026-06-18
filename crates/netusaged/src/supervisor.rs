//! Orquestación del ciclo de vida de la atribución por aplicación.
//!
//! Responsabilidad única: hilar descubrimiento, enganche, vigilancia, registro
//! y drenaje. Es el único componente que posee a la vez el handle `Ebpf` y el
//! registro, de modo que toda mutación de enganches ocurre en este hilo; el
//! vigilante de inotify corre aparte y solo emite eventos.
//!
//! Flujo:
//! 1. Cargar los programas eBPF (una vez).
//! 2. Arrancar el vigilante sobre `app.slice` (antes de descubrir, para no
//!    perder cgroups creados durante el escaneo inicial).
//! 3. Enganchar el set inicial de apps ya abiertas.
//! 4. Bucle: drenar eventos (nacimiento -> enganchar; muerte -> drenar y
//!    desenganchar) y, cada intervalo, leer los mapas y mostrar la lista por
//!    app.

use std::collections::HashMap;
use std::fs::File;
use std::path::Path;
use std::sync::mpsc::RecvTimeoutError;
use std::time::{Duration, Instant};

use anyhow::{Context, Result};
use aya::Ebpf;
use netusage_common::counters::CgroupInode;

use crate::attach::{attach_cgroup, detach_cgroup, load_programs, AttachedLinks};
use crate::cgroup::discovery::{discover_app_cgroups, user_app_slices};
use crate::cgroup::identity::{parse_scope, AppIdentity};
use crate::cgroup::inode::cgroup_inode;
use crate::cgroup::registry::{CgroupEntry, CgroupRegistry};
use crate::cgroup::watcher::{spawn_watcher, CgroupEvent};
use crate::fallback::{aggregate, sort_by_total, AppUsage};
use crate::monitor;

/// Contadores acumulados por app_key de los cgroups ya muertos (drenados), para
/// no perder sus últimos bytes cuando se cierran.
type Finalized = HashMap<String, AppUsage>;

/// Arranca la atribución por aplicación y entra en el bucle de monitorización.
///
/// `bpf` es el objeto ya cargado; `root` es el cgroup v2 raíz; `interval` es el
/// periodo de refresco de la lista.
pub fn run(mut bpf: Ebpf, root: &Path, interval: Duration) -> Result<()> {
    load_programs(&mut bpf)?;

    // Se cubren todas las app.slice de usuario, no la del UID efectivo: bajo
    // `sudo` ese UID es 0 (root) y las apps viven en la slice del usuario real.
    let app_slices = user_app_slices(root)?;
    if app_slices.is_empty() {
        tracing::warn!(
            "no se encontró ninguna app.slice de usuario bajo {}; ¿hay una sesión gráfica activa?",
            root.join("user.slice").display()
        );
    }

    // Vigilar antes de descubrir para no perder nacimientos durante el escaneo.
    let (_watcher, events) = spawn_watcher(app_slices.clone())?;

    let mut registry: CgroupRegistry<AttachedLinks> = CgroupRegistry::new();
    let mut finalized: Finalized = HashMap::new();

    for base in &app_slices {
        for path in discover_app_cgroups(base)? {
            let identity = parse_scope(&file_name(&path));
            if identity.is_app {
                if let Ok(inode) = cgroup_inode(&path) {
                    attach(&mut bpf, &mut registry, &path, inode, identity);
                }
            }
        }
    }
    tracing::info!(
        "atribución activa: {} apps enganchadas en {} slice(s) de usuario (Ctrl-C para salir)",
        registry.len(),
        app_slices.len()
    );

    loop {
        drain_events(&mut bpf, &mut registry, &mut finalized, &events, interval);
        render(&bpf, &registry, &finalized)?;
    }
}

/// Procesa eventos del vigilante hasta agotar el intervalo.
fn drain_events(
    bpf: &mut Ebpf,
    registry: &mut CgroupRegistry<AttachedLinks>,
    finalized: &mut Finalized,
    events: &std::sync::mpsc::Receiver<CgroupEvent>,
    interval: Duration,
) {
    let deadline = Instant::now() + interval;
    loop {
        let remaining = deadline.saturating_duration_since(Instant::now());
        match events.recv_timeout(remaining) {
            Ok(event) => handle_event(bpf, registry, finalized, event),
            Err(RecvTimeoutError::Timeout) => break,
            Err(RecvTimeoutError::Disconnected) => {
                // El vigilante terminó: seguir solo con lo ya enganchado.
                std::thread::sleep(remaining);
                break;
            }
        }
        if Instant::now() >= deadline {
            break;
        }
    }
}

/// Aplica un evento de ciclo de vida.
fn handle_event(
    bpf: &mut Ebpf,
    registry: &mut CgroupRegistry<AttachedLinks>,
    finalized: &mut Finalized,
    event: CgroupEvent,
) {
    match event {
        CgroupEvent::Born {
            path,
            inode,
            identity,
        } => {
            // Dedupe frente al escaneo inicial: si ya está, no reenganchar.
            if !registry.contains(inode) {
                attach(bpf, registry, &path, inode, identity);
            }
        }
        CgroupEvent::Died { path } => {
            // El evento solo trae la ruta; cruzarla con el registro por inode.
            let inode = registry.iter().find(|e| e.path == path).map(|e| e.inode);
            if let Some(inode) = inode {
                drain_and_detach(bpf, registry, finalized, inode);
            }
        }
    }
}

/// Engancha los programas al cgroup y registra su entrada.
fn attach(
    bpf: &mut Ebpf,
    registry: &mut CgroupRegistry<AttachedLinks>,
    path: &Path,
    inode: CgroupInode,
    identity: AppIdentity,
) {
    let file = match File::open(path) {
        Ok(file) => file,
        Err(err) => {
            // El cgroup pudo morir entre la resolución y el enganche: su tráfico
            // caerá en el cubo de fallback. No es un fallo del demonio.
            tracing::debug!("no se pudo abrir {}: {err}", path.display());
            return;
        }
    };
    match attach_cgroup(bpf, &file) {
        Ok(links) => {
            let app_name = identity.display_name.clone();
            registry.insert(CgroupEntry::new(inode, path.to_path_buf(), identity, links));
            tracing::info!("app enganchada: {app_name} (inode {inode})");
        }
        Err(err) => {
            tracing::warn!("no se pudo enganchar {}: {err:#}", path.display());
        }
    }
}

/// Drena los contadores finales de un cgroup que muere, los acumula por app y
/// desengancha sus programas.
fn drain_and_detach(
    bpf: &mut Ebpf,
    registry: &mut CgroupRegistry<AttachedLinks>,
    finalized: &mut Finalized,
    inode: CgroupInode,
) {
    // Leer una última vez antes de eliminar las entradas del mapa.
    let (rx, tx) = monitor::read_inode_counters(bpf, inode).unwrap_or((0, 0));

    let Some(entry) = registry.remove_by_inode(inode) else {
        return;
    };

    // Acumular los bytes finales en el total persistente de la app.
    let acc = finalized
        .entry(entry.identity.app_key.clone())
        .or_insert_with(|| AppUsage {
            app_key: entry.identity.app_key.clone(),
            display_name: entry.identity.display_name.clone(),
            rx: 0,
            tx: 0,
        });
    acc.rx = acc.rx.saturating_add(rx);
    acc.tx = acc.tx.saturating_add(tx);

    let app_name = entry.identity.display_name.clone();
    if let Err(err) = detach_cgroup(bpf, entry.links) {
        tracing::warn!("error desenganchando {app_name} (inode {inode}): {err:#}");
    }
    let _ = monitor::remove_inode_entries(bpf, inode);
    tracing::info!("app cerrada: {app_name} (inode {inode}), drenados rx={rx} tx={tx}");
}

/// Lee los mapas, agrega por app (con fallback) y muestra la lista.
fn render(
    bpf: &Ebpf,
    registry: &CgroupRegistry<AttachedLinks>,
    finalized: &Finalized,
) -> Result<()> {
    let samples = monitor::read_samples(bpf).context("leyendo los mapas de contadores")?;
    let mut usages = aggregate(samples, |inode| {
        registry
            .get_identity(inode)
            .map(|id| (id.app_key.clone(), id.display_name.clone()))
    });
    merge_finalized(&mut usages, finalized);
    sort_by_total(&mut usages);
    monitor::print_app_list(&usages);
    Ok(())
}

/// Suma los contadores de apps ya cerradas a la lista en vivo.
fn merge_finalized(usages: &mut Vec<AppUsage>, finalized: &Finalized) {
    for (key, acc) in finalized {
        if let Some(existing) = usages.iter_mut().find(|u| &u.app_key == key) {
            existing.rx = existing.rx.saturating_add(acc.rx);
            existing.tx = existing.tx.saturating_add(acc.tx);
        } else {
            usages.push(acc.clone());
        }
    }
}

/// Nombre del último componente de una ruta de cgroup (el nombre del scope).
fn file_name(path: &Path) -> String {
    path.file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_default()
}
