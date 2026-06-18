//! Registro en memoria de los cgroups enganchados y su identidad.
//!
//! Responsabilidad única: mantener, por cada cgroup de app vivo, su inode
//! (clave del mapa eBPF), su ruta, su identidad ya resuelta y sus links eBPF.
//!
//! Clave de diseño: la identidad se resuelve y se cachea al insertar (cuando el
//! cgroup nace), no al eliminar. Tras la muerte del cgroup, su directorio y su
//! nombre desaparecen de `/sys/fs/cgroup` y ya no podrían parsearse; por eso la
//! identidad debe capturarse en el nacimiento.
//!
//! El registro no lleva locking: solo el hilo principal del demonio lo muta
//! (inserta al enganchar, elimina al desenganchar) y lo lee (al agregar la
//! lista por app). El vigilante de inotify corre en otro hilo pero no toca el
//! registro: se limita a emitir eventos de ciclo de vida. Esto evita un
//! `Mutex` y la posibilidad de contención (ver desviaciones respecto al plan,
//! que proponía `Arc<Mutex>`).
//!
//! El tipo de los links es un parámetro genérico `L` para poder probar la
//! estructura de datos sin cargar eBPF (en producción `L = AttachedLinks`).

use std::collections::HashMap;
use std::collections::hash_map::Values;
use std::path::PathBuf;

use netusage_common::counters::CgroupInode;

use super::identity::AppIdentity;

/// Entrada de un cgroup enganchado.
pub struct CgroupEntry<L> {
    pub inode: CgroupInode,
    pub path: PathBuf,
    pub identity: AppIdentity,
    pub links: L,
}

impl<L> CgroupEntry<L> {
    /// Crea una entrada con su identidad ya resuelta y sus links.
    pub fn new(inode: CgroupInode, path: PathBuf, identity: AppIdentity, links: L) -> Self {
        Self {
            inode,
            path,
            identity,
            links,
        }
    }
}

/// Registro de cgroups vivos indexado por inode.
pub struct CgroupRegistry<L> {
    by_inode: HashMap<CgroupInode, CgroupEntry<L>>,
}

impl<L> CgroupRegistry<L> {
    /// Crea un registro vacío.
    pub fn new() -> Self {
        Self {
            by_inode: HashMap::new(),
        }
    }

    /// Inserta (o reemplaza) la entrada de un cgroup. Devuelve la entrada
    /// previa si la había para ese inode.
    pub fn insert(&mut self, entry: CgroupEntry<L>) -> Option<CgroupEntry<L>> {
        self.by_inode.insert(entry.inode, entry)
    }

    /// Elimina y devuelve la entrada de un inode, si existía.
    pub fn remove_by_inode(&mut self, inode: CgroupInode) -> Option<CgroupEntry<L>> {
        self.by_inode.remove(&inode)
    }

    /// Indica si hay una entrada registrada para `inode`.
    pub fn contains(&self, inode: CgroupInode) -> bool {
        self.by_inode.contains_key(&inode)
    }

    /// Devuelve la identidad cacheada de un inode, si está registrado.
    pub fn get_identity(&self, inode: CgroupInode) -> Option<&AppIdentity> {
        self.by_inode.get(&inode).map(|e| &e.identity)
    }

    /// Itera sobre todas las entradas vivas.
    pub fn iter(&self) -> Values<'_, CgroupInode, CgroupEntry<L>> {
        self.by_inode.values()
    }

    /// Número de cgroups registrados.
    pub fn len(&self) -> usize {
        self.by_inode.len()
    }

    /// Indica si el registro está vacío.
    pub fn is_empty(&self) -> bool {
        self.by_inode.is_empty()
    }
}

impl<L> Default for CgroupRegistry<L> {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn app_identity(key: &str) -> AppIdentity {
        // Reutiliza el parser real para construir una identidad de prueba.
        super::super::identity::parse_scope(&format!("app-gnome-{key}-1.scope"))
    }

    fn entry(inode: CgroupInode, key: &str) -> CgroupEntry<()> {
        // `L = ()`: links ficticios, no se necesita eBPF para probar la
        // estructura de datos.
        CgroupEntry::new(
            inode,
            PathBuf::from(format!("/sys/fs/cgroup/app-{key}.scope")),
            app_identity(key),
            (),
        )
    }

    #[test]
    fn insert_get_identity_and_remove() {
        let mut reg: CgroupRegistry<()> = CgroupRegistry::new();
        assert!(reg.is_empty());

        reg.insert(entry(42, "firefox"));
        assert_eq!(reg.len(), 1);
        assert!(reg.contains(42));
        assert_eq!(reg.get_identity(42).unwrap().display_name, "firefox");

        let removed = reg.remove_by_inode(42).unwrap();
        assert_eq!(removed.inode, 42);
        assert!(!reg.contains(42));
        assert!(reg.is_empty());
    }

    #[test]
    fn unknown_inode_has_no_identity() {
        let mut reg: CgroupRegistry<()> = CgroupRegistry::new();
        assert!(reg.get_identity(999).is_none());
        assert!(reg.remove_by_inode(999).is_none());
    }
}
