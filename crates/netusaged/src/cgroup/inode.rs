//! Traducción de la ruta de un cgroup al inode que el mapa eBPF usa como clave.
//!
//! Responsabilidad única: dado el directorio de un cgroup en `/sys/fs/cgroup`,
//! devolver su número de inode.
//!
//! Equivalencia clave de la atribución: el `st_ino` del directorio de un cgroup
//! en `/sys/fs/cgroup` es exactamente el cgroup id que devuelve el helper
//! `bpf_skb_cgroup_id` en el kernel. Por eso este inode sirve de puente entre el
//! árbol de cgroups del espacio de usuario y las entradas del mapa eBPF.

use std::io;
use std::path::Path;

use netusage_common::counters::CgroupInode;
use rustix::fs::{statx, AtFlags, StatxFlags, CWD};

/// Devuelve el inode del directorio de cgroup en `path`.
///
/// Usa `statx` pidiendo solo el campo de inode. Resuelve enlaces simbólicos de
/// forma normal (los cgroups no son enlaces, pero no forzamos `NOFOLLOW` para
/// no fallar en montajes atípicos).
pub fn cgroup_inode(path: &Path) -> io::Result<CgroupInode> {
    let stat = statx(CWD, path, AtFlags::empty(), StatxFlags::INO)?;
    Ok(stat.stx_ino)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn matches_std_metadata_inode() {
        // Cruza el resultado con el inode que reporta std para un directorio
        // real cualquiera (no necesita ser un cgroup: la fuente del inode es la
        // misma syscall subyacente).
        use std::os::unix::fs::MetadataExt;

        let dir = std::env::temp_dir();
        let via_statx = cgroup_inode(&dir).unwrap();
        let via_std = std::fs::metadata(&dir).unwrap().ino();
        assert_eq!(via_statx, via_std);
    }

    #[test]
    fn missing_path_is_error() {
        let res = cgroup_inode(Path::new("/definitely/missing/cgroup"));
        assert!(res.is_err());
    }
}
