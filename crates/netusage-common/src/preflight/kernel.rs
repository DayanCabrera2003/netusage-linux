//! Comprobador de version de kernel.
//!
//! Responsabilidad unica: leer la version del kernel y compararla con los
//! minimos requeridos por el proyecto.
//!
//! Minimos:
//! - `BPF_PROG_TYPE_CGROUP_SKB` requiere kernel >= 4.10.
//! - Las capabilities BPF (`CAP_BPF`/`CAP_PERFMON`) requieren kernel >= 5.8;
//!   por debajo se podra cargar eBPF pero solo como root.

use super::{CheckResult, CheckStatus};

const CHECK_NAME: &str = "version de kernel";

/// Parsea la cadena `release` de `uname` (p. ej. `7.0.11-100.fc43.x86_64`)
/// quedandose con `major.minor`. Ignora sufijos de distro.
pub fn parse_release(release: &str) -> Option<(u32, u32)> {
    let mut nums = release
        .split(|c: char| !c.is_ascii_digit())
        .filter(|s| !s.is_empty());
    let major = nums.next()?.parse().ok()?;
    let minor = nums.next()?.parse().ok()?;
    Some((major, minor))
}

/// Clasifica una version `major.minor` contra los minimos.
pub fn classify(major: u32, minor: u32) -> CheckResult {
    let supports_cgroup_skb = (major, minor) >= (4, 10);
    let supports_bpf_caps = (major, minor) >= (5, 8);

    if supports_bpf_caps {
        CheckResult::new(
            CHECK_NAME,
            CheckStatus::Ok,
            format!("kernel {major}.{minor} soporta cgroup_skb y capabilities BPF"),
        )
    } else if supports_cgroup_skb {
        CheckResult::new(
            CHECK_NAME,
            CheckStatus::Warn,
            format!(
                "kernel {major}.{minor} soporta cgroup_skb pero no capabilities \
                 BPF (<5.8); requerira ejecutarse como root"
            ),
        )
    } else {
        CheckResult::new(
            CHECK_NAME,
            CheckStatus::Fail,
            format!("kernel {major}.{minor} no soporta BPF_PROG_TYPE_CGROUP_SKB (<4.10)"),
        )
    }
}

/// Cadena `release` del kernel en ejecucion (p. ej. `6.8.0-31-generic`).
pub fn release_string() -> String {
    rustix::system::uname()
        .release()
        .to_string_lossy()
        .into_owned()
}

/// Indica si el kernel en ejecucion soporta las capabilities BPF (>= 5.8),
/// requisito para la atribucion por aplicacion sin ejecutarse como root y para
/// el mapa `RingBuf` que usa la deteccion de nacimiento de sockets.
pub fn supports_bpf_caps() -> bool {
    parse_release(&release_string())
        .map(|(major, minor)| (major, minor) >= (5, 8))
        .unwrap_or(false)
}

/// Ejecuta el comprobador contra el sistema real.
pub fn check() -> CheckResult {
    let release = release_string();
    match parse_release(&release) {
        Some((major, minor)) => classify(major, minor),
        None => CheckResult::new(
            CHECK_NAME,
            CheckStatus::Warn,
            format!("no se pudo interpretar la version del kernel: {release}"),
        ),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_release_with_distro_suffix() {
        assert_eq!(parse_release("7.0.11-100.fc43.x86_64"), Some((7, 0)));
        assert_eq!(parse_release("6.1.0-13-amd64"), Some((6, 1)));
        assert_eq!(parse_release("5.15.0"), Some((5, 15)));
    }

    #[test]
    fn classify_minimums() {
        assert_eq!(classify(4, 9).status, CheckStatus::Fail);
        assert_eq!(classify(4, 10).status, CheckStatus::Warn);
        assert_eq!(classify(5, 7).status, CheckStatus::Warn);
        assert_eq!(classify(5, 8).status, CheckStatus::Ok);
        assert_eq!(classify(7, 0).status, CheckStatus::Ok);
    }
}
