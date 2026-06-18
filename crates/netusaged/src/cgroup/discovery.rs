//! Descubrimiento estático de los cgroups de aplicación del usuario.
//!
//! Responsabilidad única: dada la raíz de cgroup v2, calcular la ruta de
//! `app.slice` del usuario y enumerar los directorios de scope/servicio de app
//! que cuelgan de ella. No resuelve identidad ni inode; solo rutas.
//!
//! Decisión de alcance (MVP de Fase 2): se atribuye el tráfico de las apps del
//! usuario que ejecuta el demonio (su UID efectivo). Un demonio de sistema que
//! cubriera a todos los usuarios tendría que iterar cada `user-<UID>.slice`;
//! eso queda preparado en la forma de las funciones (la base es un parámetro)
//! pero no se activa aquí. Ver documentacion/desviaciones/fase-2.md.

use std::io;
use std::path::{Path, PathBuf};

/// Devuelve la ruta de `app.slice` del usuario `uid` bajo la raíz `root` de
/// cgroup v2.
///
/// La jerarquía de systemd para sesiones de usuario es:
/// `root/user.slice/user-<UID>.slice/user@<UID>.service/app.slice`.
pub fn app_slice_path(root: &Path, uid: u32) -> PathBuf {
    root.join("user.slice")
        .join(format!("user-{uid}.slice"))
        .join(format!("user@{uid}.service"))
        .join("app.slice")
}

/// Enumera recursivamente los cgroups de aplicación bajo `base` (normalmente
/// `app.slice`).
///
/// Recolecta los directorios cuyo nombre termina en `.scope` o `.service`: los
/// scopes son el caso principal de las apps de escritorio (GNOME/KDE), y
/// algunos entornos modelan ciertas apps como `.service`. La función es pura y
/// testeable: opera sobre la ruta `base` que se le pasa, sin asumir nada del
/// sistema más allá de poder leer directorios.
///
/// Si `base` no existe (p. ej. no hay sesión gráfica para ese UID), devuelve
/// una lista vacía en vez de error: es una condición normal, no un fallo.
pub fn discover_app_cgroups(base: &Path) -> io::Result<Vec<PathBuf>> {
    let mut found = Vec::new();
    if !base.exists() {
        return Ok(found);
    }
    collect_app_dirs(base, &mut found)?;
    Ok(found)
}

/// Recorre `dir` en profundidad acumulando en `out` los subdirectorios que son
/// scopes o servicios de aplicación.
fn collect_app_dirs(dir: &Path, out: &mut Vec<PathBuf>) -> io::Result<()> {
    for entry in std::fs::read_dir(dir)? {
        let entry = entry?;
        // Solo nos interesan directorios: cada cgroup es un directorio.
        if !entry.file_type()?.is_dir() {
            continue;
        }
        let path = entry.path();
        if is_app_cgroup_name(&entry.file_name().to_string_lossy()) {
            out.push(path.clone());
        }
        // Los scopes de app pueden anidar (p. ej. un `.scope` con hijos), así
        // que se sigue descendiendo en todos los directorios.
        collect_app_dirs(&path, out)?;
    }
    Ok(())
}

/// Indica si el nombre de un directorio de cgroup corresponde a un scope o
/// servicio de aplicación.
pub fn is_app_cgroup_name(name: &str) -> bool {
    name.ends_with(".scope") || name.ends_with(".service")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn builds_app_slice_path_for_uid() {
        let p = app_slice_path(Path::new("/sys/fs/cgroup"), 1000);
        assert_eq!(
            p,
            Path::new(
                "/sys/fs/cgroup/user.slice/user-1000.slice/user@1000.service/app.slice"
            )
        );
    }

    #[test]
    fn missing_base_yields_empty_list() {
        let base = Path::new("/definitely/does/not/exist/app.slice");
        assert!(discover_app_cgroups(base).unwrap().is_empty());
    }

    #[test]
    fn collects_scopes_and_services_recursively() {
        // Árbol temporal que imita app.slice con scopes y servicios anidados.
        let tmp = std::env::temp_dir().join(format!("netusage-disc-{}", std::process::id()));
        let scope = tmp.join("app-gnome-firefox-2838.scope");
        let nested = tmp.join("app-foo.service").join("app-bar-1.scope");
        std::fs::create_dir_all(&scope).unwrap();
        std::fs::create_dir_all(&nested).unwrap();
        // Un directorio que no es app no debe recolectarse.
        std::fs::create_dir_all(tmp.join("noise")).unwrap();

        let mut got = discover_app_cgroups(&tmp).unwrap();
        got.sort();

        assert!(got.contains(&scope));
        assert!(got.contains(&tmp.join("app-foo.service")));
        assert!(got.contains(&nested));
        assert!(!got.iter().any(|p| p.ends_with("noise")));

        std::fs::remove_dir_all(&tmp).ok();
    }
}
