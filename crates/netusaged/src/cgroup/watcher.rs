//! Vigilante del árbol de cgroups: detecta nacimiento y muerte de cgroups de
//! aplicación y los notifica como eventos.
//!
//! Responsabilidad única: traducir los eventos de bajo nivel de `inotify`
//! (creación y borrado de directorios) en eventos de ciclo de vida de alto
//! nivel (`CgroupEvent`). No engancha eBPF ni toca el registro; solo emite por
//! un canal para que el hilo principal actúe.
//!
//! Mitigación del race del spec: la identidad y el inode de un cgroup se
//! resuelven aquí, en el momento del nacimiento (`CREATE`), antes de que el
//! cgroup pueda morir y desaparecer su directorio. El evento `Born` ya viaja
//! con esa información resuelta.
//!
//! Como `inotify` no es recursivo, se añade un watch sobre la base y, cada vez
//! que nace un directorio, se añade un watch sobre él para captar a sus hijos.
//!
//! Alternativa evaluada y descartada para el MVP: consumir eventos
//! `cgroup_mkdir` mediante un raw tracepoint eBPF (como pktstat-bpf), que
//! elimina del todo el race pero añade un segundo programa eBPF y más
//! complejidad. Queda como evolución natural si inotify resultara insuficiente.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::mpsc::{self, Receiver};
use std::thread::{self, JoinHandle};

use anyhow::{Context, Result};
use inotify::{EventMask, Inotify, WatchDescriptor, WatchMask};
use netusage_common::counters::CgroupInode;

use super::discovery::is_app_cgroup_name;
use super::identity::{parse_scope, AppIdentity};
use super::inode::cgroup_inode;

/// Evento de ciclo de vida de un cgroup de aplicación.
pub enum CgroupEvent {
    /// Nace un cgroup de app. Trae su identidad e inode ya resueltos para no
    /// perderlos si el cgroup muere antes de que el hilo principal lo procese.
    Born {
        path: PathBuf,
        inode: CgroupInode,
        identity: AppIdentity,
    },
    /// Muere un cgroup. Solo se conoce la ruta; el hilo principal la cruza con
    /// el registro para hallar el inode y drenar sus contadores finales.
    Died { path: PathBuf },
}

/// Máscara de eventos: creación y borrado de directorios.
fn watch_mask() -> WatchMask {
    WatchMask::CREATE | WatchMask::DELETE | WatchMask::ONLYDIR
}

/// Arranca el vigilante en un hilo dedicado y devuelve el extremo receptor del
/// canal de eventos junto al `JoinHandle` del hilo.
///
/// Se vigila `base` (típicamente `app.slice`) y, de forma recursiva, los
/// subdirectorios ya existentes, para captar la creación de scopes dentro de
/// ellos. Los cgroups que ya existían NO se emiten como `Born`: de ese set
/// inicial se encarga el descubrimiento estático del hilo principal. El
/// vigilante solo reporta cambios a partir de su arranque.
pub fn spawn_watcher(base: PathBuf) -> Result<(JoinHandle<()>, Receiver<CgroupEvent>)> {
    let mut inotify = Inotify::init().context("inicializando inotify")?;

    // Mapa de descriptor de watch -> ruta del directorio vigilado, para
    // reconstruir la ruta completa de cada evento (inotify da el nombre
    // relativo al directorio del watch).
    let mut wd_paths: HashMap<WatchDescriptor, PathBuf> = HashMap::new();
    add_watches_recursive(&mut inotify, &base, &mut wd_paths)?;

    let (tx, rx) = mpsc::channel();
    let handle = thread::spawn(move || {
        let mut buffer = [0u8; 4096];
        loop {
            let events = match inotify.read_events_blocking(&mut buffer) {
                Ok(events) => events,
                Err(err) => {
                    tracing::warn!("inotify dejó de leer eventos: {err}");
                    return;
                }
            };

            for event in events {
                // Solo interesan eventos de directorio con nombre.
                let Some(name) = event.name else { continue };
                if !event.mask.contains(EventMask::ISDIR) {
                    continue;
                }
                let Some(parent) = wd_paths.get(&event.wd).cloned() else {
                    continue;
                };
                let path = parent.join(name);

                if event.mask.contains(EventMask::CREATE) {
                    // Vigilar el nuevo directorio para captar a sus hijos.
                    if let Ok(wd) = inotify.watches().add(&path, watch_mask()) {
                        wd_paths.insert(wd, path.clone());
                    }
                    if is_app_cgroup_name(&name.to_string_lossy()) {
                        if let Some(event) = born_event(&path) {
                            if tx.send(event).is_err() {
                                return; // receptor cerrado: terminar el hilo.
                            }
                        }
                    }
                } else if event.mask.contains(EventMask::DELETE)
                    && tx.send(CgroupEvent::Died { path }).is_err()
                {
                    return;
                }
            }
        }
    });

    Ok((handle, rx))
}

/// Construye el evento `Born` resolviendo identidad e inode del cgroup recién
/// nacido. Devuelve `None` si el cgroup no es una app o si no se pudo resolver
/// su inode (p. ej. murió de inmediato).
fn born_event(path: &Path) -> Option<CgroupEvent> {
    let name = path.file_name()?.to_string_lossy();
    let identity = parse_scope(&name);
    if !identity.is_app {
        return None;
    }
    let inode = cgroup_inode(path).ok()?;
    Some(CgroupEvent::Born {
        path: path.to_path_buf(),
        inode,
        identity,
    })
}

/// Añade watches sobre `dir` y todos sus subdirectorios existentes.
fn add_watches_recursive(
    inotify: &mut Inotify,
    dir: &Path,
    wd_paths: &mut HashMap<WatchDescriptor, PathBuf>,
) -> Result<()> {
    if !dir.exists() {
        return Ok(());
    }
    let wd = inotify
        .watches()
        .add(dir, watch_mask())
        .with_context(|| format!("añadiendo watch sobre {}", dir.display()))?;
    wd_paths.insert(wd, dir.to_path_buf());

    for entry in std::fs::read_dir(dir)? {
        let entry = entry?;
        if entry.file_type()?.is_dir() {
            add_watches_recursive(inotify, &entry.path(), wd_paths)?;
        }
    }
    Ok(())
}
