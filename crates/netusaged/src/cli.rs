//! Definicion de la superficie de linea de comandos del demonio.
//!
//! Responsabilidad unica: declarar los argumentos. El comportamiento vive en
//! otros modulos.

use clap::Parser;

/// Demonio de netusage: monitor de uso de datos por aplicacion.
#[derive(Debug, Parser)]
#[command(name = "netusaged", version, about)]
pub struct Cli {
    /// Comprueba el entorno (cgroup v2, BTF, version de kernel, capabilities)
    /// e imprime un informe. Devuelve codigo 0 si el sistema es apto.
    #[arg(long)]
    pub check: bool,
}
