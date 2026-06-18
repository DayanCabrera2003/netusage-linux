//! Comprobador de BTF (BPF Type Format).
//!
//! Responsabilidad unica: verificar que el kernel expone BTF en
//! `/sys/kernel/btf/vmlinux`, requisito de CO-RE.

use std::path::Path;

use super::{CheckResult, CheckStatus};

/// Ruta estandar del BTF del kernel.
const BTF_PATH: &str = "/sys/kernel/btf/vmlinux";

const CHECK_NAME: &str = "BTF del kernel";

/// Clasifica la disponibilidad de BTF a partir de una ruta.
///
/// Recibe la ruta como parametro para poder testear los dos caminos (presente
/// y ausente) sin depender del sistema real.
pub fn classify_path(path: &Path) -> CheckResult {
    if path.is_file() {
        CheckResult::new(
            CHECK_NAME,
            CheckStatus::Ok,
            format!("{} presente", path.display()),
        )
    } else {
        CheckResult::new(
            CHECK_NAME,
            CheckStatus::Fail,
            format!(
                "{} ausente; CO-RE requiere BTF (CONFIG_DEBUG_INFO_BTF=y)",
                path.display()
            ),
        )
    }
}

/// Ejecuta el comprobador contra el sistema real.
pub fn check() -> CheckResult {
    classify_path(Path::new(BTF_PATH))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    #[test]
    fn existing_file_is_ok() {
        let mut tmp = tempfile_in_target();
        writeln!(tmp.file, "btf").unwrap();
        let r = classify_path(&tmp.path);
        assert_eq!(r.status, CheckStatus::Ok);
    }

    #[test]
    fn missing_file_is_fail() {
        let missing = std::env::temp_dir().join("netusage-no-existe-btf-xyz");
        let _ = std::fs::remove_file(&missing);
        let r = classify_path(&missing);
        assert_eq!(r.status, CheckStatus::Fail);
    }

    /// Fichero temporal minimo sin dependencias externas: se crea en el
    /// directorio temporal del sistema y se borra al soltarse.
    struct TmpFile {
        file: std::fs::File,
        path: std::path::PathBuf,
    }

    impl Drop for TmpFile {
        fn drop(&mut self) {
            let _ = std::fs::remove_file(&self.path);
        }
    }

    fn tempfile_in_target() -> TmpFile {
        let path = std::env::temp_dir().join(format!("netusage-btf-test-{}", std::process::id()));
        let file = std::fs::File::create(&path).unwrap();
        TmpFile { file, path }
    }
}
