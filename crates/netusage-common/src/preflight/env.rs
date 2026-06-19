//! Informe de entorno legible por maquina.
//!
//! Responsabilidad unica: condensar las comprobaciones de entorno en un struct
//! de banderas booleanas mas la version del kernel. A diferencia de `Report`
//! (orientado a texto para humanos), `EnvReport` esta pensado para consumo
//! programatico: la salida JSON de `--check --json` y la decision del modo de
//! ejecucion (ver el modulo `degraded`).
//!
//! La bandera `per_app` es una estimacion estatica (kernel + cgroup + BTF), no
//! una prueba de carga real. La verificacion definitiva de que los programas
//! cargan y enganchan es `netusaged --selftest-load`.

use super::{btf, caps, cgroup, kernel};

/// Informe estructurado del entorno.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EnvReport {
    /// Cadena `release` del kernel (p. ej. "6.8.0-31-generic").
    pub kernel: String,
    /// `/sys/fs/cgroup` montado como cgroup v2 unificado.
    pub cgroup_v2: bool,
    /// BTF del kernel presente (requisito de CO-RE).
    pub btf: bool,
    /// Atribucion por aplicacion previsiblemente disponible: requiere cgroup v2,
    /// BTF y kernel >= 5.8 (para `RingBuf` y `cgroup/sock_create`).
    pub per_app: bool,
    /// El proceso puede cargar y enganchar eBPF (root o capabilities).
    pub caps_ok: bool,
}

impl EnvReport {
    /// Recopila el informe consultando el sistema real.
    pub fn gather() -> Self {
        let kernel = kernel::release_string();
        let cgroup_v2 = cgroup::is_v2();
        let btf = btf::present();
        let per_app = cgroup_v2 && btf && kernel::supports_bpf_caps();
        let caps_ok = caps::observe().is_sufficient();
        Self {
            kernel,
            cgroup_v2,
            btf,
            per_app,
            caps_ok,
        }
    }

    /// Serializa el informe como un objeto JSON de una linea.
    ///
    /// Se construye a mano (sin serde) para no arrastrar esa dependencia al
    /// crate comun: el objeto es plano y de campos conocidos.
    pub fn to_json(&self) -> String {
        format!(
            "{{\"kernel\":\"{}\",\"cgroup_v2\":{},\"btf\":{},\"per_app\":{},\"caps_ok\":{}}}",
            escape_json(&self.kernel),
            self.cgroup_v2,
            self.btf,
            self.per_app,
            self.caps_ok
        )
    }
}

/// Escapa los caracteres que JSON no admite crudos dentro de una cadena.
fn escape_json(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for c in s.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            c if (c as u32) < 0x20 => out.push_str(&format!("\\u{:04x}", c as u32)),
            c => out.push(c),
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn report() -> EnvReport {
        EnvReport {
            kernel: "6.8.0-31-generic".to_string(),
            cgroup_v2: true,
            btf: true,
            per_app: true,
            caps_ok: false,
        }
    }

    #[test]
    fn json_has_all_fields() {
        let json = report().to_json();
        assert_eq!(
            json,
            "{\"kernel\":\"6.8.0-31-generic\",\"cgroup_v2\":true,\"btf\":true,\
             \"per_app\":true,\"caps_ok\":false}"
        );
    }

    #[test]
    fn json_escapes_special_chars() {
        assert_eq!(escape_json("a\"b\\c"), "a\\\"b\\\\c");
        assert_eq!(escape_json("line\nbreak"), "line\\nbreak");
    }

    #[test]
    fn gather_returns_nonempty_kernel() {
        // En cualquier Linux real la cadena release no esta vacia.
        let report = EnvReport::gather();
        assert!(!report.kernel.is_empty());
    }
}
