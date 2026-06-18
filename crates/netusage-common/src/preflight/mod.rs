//! Comprobaciones de entorno (preflight) que determinan si el sistema es apto
//! para cargar y ejecutar el monitor eBPF.
//!
//! Cada comprobador vive en su propio modulo con una unica responsabilidad.
//! Este modulo solo define los tipos comunes (estado, resultado e informe) y
//! la agregacion del veredicto final.

pub mod cgroup;

/// Estado de una comprobacion individual.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CheckStatus {
    /// La comprobacion se cumple.
    Ok,
    /// La comprobacion no es ideal pero no impide funcionar.
    Warn,
    /// La comprobacion falla y bloquea el funcionamiento.
    Fail,
}

impl CheckStatus {
    /// Etiqueta corta para mostrar en el informe.
    pub fn label(self) -> &'static str {
        match self {
            CheckStatus::Ok => "OK",
            CheckStatus::Warn => "WARN",
            CheckStatus::Fail => "FAIL",
        }
    }
}

/// Resultado de una comprobacion: nombre, estado y detalle explicativo.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CheckResult {
    pub name: &'static str,
    pub status: CheckStatus,
    pub detail: String,
}

impl CheckResult {
    pub fn new(name: &'static str, status: CheckStatus, detail: impl Into<String>) -> Self {
        Self {
            name,
            status,
            detail: detail.into(),
        }
    }
}

/// Informe agregado de todas las comprobaciones.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Report {
    pub results: Vec<CheckResult>,
}

impl Report {
    pub fn new(results: Vec<CheckResult>) -> Self {
        Self { results }
    }

    /// El sistema es apto si ninguna comprobacion termino en `Fail`.
    /// Los `Warn` se reportan pero no bloquean.
    pub fn is_apt(&self) -> bool {
        !self
            .results
            .iter()
            .any(|r| r.status == CheckStatus::Fail)
    }

    /// Formatea el informe como texto plano, una linea por comprobacion.
    pub fn format_plain(&self) -> String {
        let mut out = String::new();
        for r in &self.results {
            out.push_str(&format!("[{}] {}: {}\n", r.status.label(), r.name, r.detail));
        }
        out
    }
}
