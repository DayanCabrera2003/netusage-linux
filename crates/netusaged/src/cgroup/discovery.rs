//! Descubrimiento estático de los cgroups de aplicación del usuario.
//!
//! Responsabilidad única: dada la raíz de cgroup v2, calcular la ruta de
//! `app.slice` del usuario y enumerar los directorios de scope/servicio de app
//! que cuelgan de ella. No resuelve identidad ni inode; solo rutas.
//!
//! Alcance: se enumeran las `app.slice` de todos los usuarios con sesión activa
//! (`user-<UID>.slice`). Esto es necesario porque bajo `sudo` el UID efectivo
//! del demonio es 0 (root) mientras que las apps viven en la slice del usuario
//! real, y conviene de cara al demonio de sistema de la Fase 4. Ver
//! documentacion/desviaciones/fase-2.md.

use std::io;
use std::path::{Path, PathBuf};

/// Enumera las rutas `app.slice` de todos los usuarios con sesión activa bajo
/// `root/user.slice`.
///
/// Se enumeran todos los `user-<UID>.slice` en vez de un solo UID por dos
/// motivos: cuando el demonio se ejecuta con `sudo`, su UID efectivo es 0
/// (root) y las apps del usuario viven en su propia `user-<UID>.slice`, no en la
/// de root; y un demonio de sistema (Fase 4) debe cubrir a todos los usuarios.
/// Solo se devuelven las `app.slice` que existen (usuarios con sesión gráfica).
pub fn user_app_slices(root: &Path) -> io::Result<Vec<PathBuf>> {
    let user_slice = root.join("user.slice");
    let mut slices = Vec::new();
    if !user_slice.is_dir() {
        return Ok(slices);
    }

    for entry in std::fs::read_dir(&user_slice)? {
        let entry = entry?;
        if !entry.file_type()?.is_dir() {
            continue;
        }
        let name = entry.file_name().to_string_lossy().into_owned();
        let Some(uid) = parse_user_slice_uid(&name) else {
            continue;
        };
        let app_slice = entry
            .path()
            .join(format!("user@{uid}.service"))
            .join("app.slice");
        if app_slice.is_dir() {
            slices.push(app_slice);
        }
    }
    Ok(slices)
}

/// Extrae el UID de un nombre `user-<UID>.slice`; `None` si no encaja.
fn parse_user_slice_uid(name: &str) -> Option<u32> {
    name.strip_prefix("user-")
        .and_then(|rest| rest.strip_suffix(".slice"))
        .and_then(|uid| uid.parse::<u32>().ok())
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

    #[test]
    fn parses_user_slice_uid() {
        assert_eq!(parse_user_slice_uid("user-1000.slice"), Some(1000));
        assert_eq!(parse_user_slice_uid("user-0.slice"), Some(0));
        assert_eq!(parse_user_slice_uid("init.scope"), None);
        assert_eq!(parse_user_slice_uid("user-abc.slice"), None);
        assert_eq!(parse_user_slice_uid("user-.slice"), None);
    }

    #[test]
    fn enumerates_existing_user_app_slices() {
        // Raíz temporal que imita /sys/fs/cgroup con dos usuarios; solo uno
        // tiene app.slice (sesión gráfica).
        let root = std::env::temp_dir().join(format!("netusage-uas-{}", std::process::id()));
        let with_session = root.join("user.slice/user-1000.slice/user@1000.service/app.slice");
        std::fs::create_dir_all(&with_session).unwrap();
        // Usuario sin app.slice: no debe aparecer.
        std::fs::create_dir_all(root.join("user.slice/user-42.slice")).unwrap();
        // Directorio que no es user-*.slice: se ignora.
        std::fs::create_dir_all(root.join("user.slice/noise")).unwrap();

        let got = user_app_slices(&root).unwrap();
        assert_eq!(got, vec![with_session]);

        std::fs::remove_dir_all(&root).ok();
    }

    #[test]
    fn missing_user_slice_yields_empty() {
        let root = Path::new("/definitely/missing/cgroup-root");
        assert!(user_app_slices(root).unwrap().is_empty());
    }
}
