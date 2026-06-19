//! Argumentos de línea de comandos de la TUI.

use std::path::PathBuf;

use clap::{Parser, ValueEnum};

use crate::period::Period;

/// Interfaz de terminal de netusage.
#[derive(Debug, Parser)]
#[command(name = "netusage-tui", version, about)]
pub struct Cli {
    /// Periodo inicial a mostrar.
    #[arg(long, value_enum, default_value_t = CliPeriod::Today)]
    pub period: CliPeriod,

    /// Cada cuántos segundos se refrescan los datos.
    #[arg(long, default_value_t = 2)]
    pub refresh_secs: u64,

    /// Ruta de la base de datos del demonio (solo lectura).
    #[arg(long, default_value = "/var/lib/netusage/netusage.db")]
    pub db: PathBuf,
}

/// Periodo seleccionable por CLI.
#[derive(Debug, Clone, Copy, ValueEnum)]
pub enum CliPeriod {
    Today,
    Week,
    Month,
    LastMonth,
}

impl CliPeriod {
    /// Convierte al `Period` interno.
    pub fn to_period(self) -> Period {
        match self {
            CliPeriod::Today => Period::Today,
            CliPeriod::Week => Period::Week,
            CliPeriod::Month => Period::Month,
            CliPeriod::LastMonth => Period::LastMonth,
        }
    }
}
