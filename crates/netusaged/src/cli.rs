//! Definición de la superficie de línea de comandos del demonio.
//!
//! Responsabilidad única: declarar los argumentos. El comportamiento vive en
//! otros módulos.

use clap::{Parser, Subcommand};

/// Demonio de netusage: monitor de uso de datos por aplicación.
#[derive(Debug, Parser)]
#[command(name = "netusaged", version, about)]
pub struct Cli {
    /// Comprueba el entorno (cgroup v2, BTF, versión de kernel, capabilities)
    /// e imprime un informe. Devuelve código 0 si el sistema es apto.
    #[arg(long)]
    pub check: bool,

    #[command(subcommand)]
    pub command: Option<Command>,
}

/// Subcomandos del demonio.
#[derive(Debug, Subcommand)]
pub enum Command {
    /// Engancha los programas eBPF al cgroup raíz y monitoriza en vivo el
    /// tráfico total (rx/tx) de la máquina.
    Run {
        /// Intervalo de muestreo en segundos.
        #[arg(long, default_value_t = 2)]
        interval_secs: u64,
    },
}
