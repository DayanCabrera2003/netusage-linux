//! Comprobador de privilegios y capabilities para eBPF.
//!
//! Responsabilidad unica: determinar si el proceso podra cargar y enganchar
//! programas eBPF, ya sea por ser root o por tener el conjunto de capabilities
//! `CAP_BPF` + `CAP_PERFMON` + `CAP_NET_ADMIN` (kernel >= 5.8).
//!
//! Este comprobador no es un `Fail` duro: `--check` puede ejecutarse sin
//! privilegios solo para diagnosticar, por eso la ausencia de privilegios es
//! un `Warn`.

use super::{CheckResult, CheckStatus};

const CHECK_NAME: &str = "privilegios para eBPF";

/// Conjunto de privilegios observado, para clasificar de forma pura.
#[derive(Debug, Clone, Copy)]
pub struct Privileges {
    pub is_root: bool,
    pub has_bpf: bool,
    pub has_perfmon: bool,
    pub has_net_admin: bool,
}

impl Privileges {
    fn has_full_caps(&self) -> bool {
        self.has_bpf && self.has_perfmon && self.has_net_admin
    }
}

/// Clasifica los privilegios observados.
pub fn classify(p: Privileges) -> CheckResult {
    if p.is_root {
        CheckResult::new(
            CHECK_NAME,
            CheckStatus::Ok,
            "proceso ejecutado como root".to_string(),
        )
    } else if p.has_full_caps() {
        CheckResult::new(
            CHECK_NAME,
            CheckStatus::Ok,
            "presentes CAP_BPF, CAP_PERFMON y CAP_NET_ADMIN".to_string(),
        )
    } else {
        CheckResult::new(
            CHECK_NAME,
            CheckStatus::Warn,
            "se necesita root o CAP_BPF+CAP_PERFMON+CAP_NET_ADMIN para cargar y \
             enganchar eBPF (se otorgaran via systemd en la Fase 4)"
                .to_string(),
        )
    }
}

/// Ejecuta el comprobador contra el proceso real.
pub fn check() -> CheckResult {
    use caps::{CapSet, Capability};

    let is_root = rustix::process::geteuid().is_root();
    let has = |cap| caps::has_cap(None, CapSet::Effective, cap).unwrap_or(false);

    classify(Privileges {
        is_root,
        has_bpf: has(Capability::CAP_BPF),
        has_perfmon: has(Capability::CAP_PERFMON),
        has_net_admin: has(Capability::CAP_NET_ADMIN),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn privs(is_root: bool, bpf: bool, perfmon: bool, net_admin: bool) -> Privileges {
        Privileges {
            is_root,
            has_bpf: bpf,
            has_perfmon: perfmon,
            has_net_admin: net_admin,
        }
    }

    #[test]
    fn root_is_ok() {
        assert_eq!(
            classify(privs(true, false, false, false)).status,
            CheckStatus::Ok
        );
    }

    #[test]
    fn full_caps_is_ok() {
        assert_eq!(
            classify(privs(false, true, true, true)).status,
            CheckStatus::Ok
        );
    }

    #[test]
    fn partial_caps_is_warn() {
        assert_eq!(
            classify(privs(false, true, true, false)).status,
            CheckStatus::Warn
        );
    }

    #[test]
    fn no_privileges_is_warn() {
        assert_eq!(
            classify(privs(false, false, false, false)).status,
            CheckStatus::Warn
        );
    }
}
