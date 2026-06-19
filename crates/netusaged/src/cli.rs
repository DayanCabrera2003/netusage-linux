//! Definición de la superficie de línea de comandos del demonio.
//!
//! Responsabilidad única: declarar los argumentos. El comportamiento vive en
//! otros módulos.

use std::path::PathBuf;

use clap::{Parser, Subcommand, ValueEnum};

/// Demonio de netusage: monitor de uso de datos por aplicación.
#[derive(Debug, Parser)]
#[command(name = "netusaged", version, about)]
pub struct Cli {
    /// Comprueba el entorno (cgroup v2, BTF, versión de kernel, capabilities)
    /// e imprime un informe. Devuelve código 0 si el sistema es apto.
    #[arg(long)]
    pub check: bool,

    /// Junto con `--check`, emite el informe de entorno en formato JSON de una
    /// línea (para consumo programático en lugar del informe legible).
    #[arg(long)]
    pub json: bool,

    #[command(subcommand)]
    pub command: Option<Command>,
}

/// Subcomandos del demonio.
#[derive(Debug, Subcommand)]
pub enum Command {
    /// Engancha los programas eBPF al cgroup raíz y atribuye el tráfico por
    /// aplicación en vivo. Con `--db`, además persiste las muestras.
    Run {
        /// Intervalo de muestreo en segundos.
        #[arg(long, default_value_t = 2)]
        interval_secs: u64,

        /// Ruta de la base de datos SQLite donde persistir las muestras. Si se
        /// omite, solo se muestra en vivo sin persistir.
        #[arg(long)]
        db: Option<PathBuf>,
    },

    /// Consulta el consumo persistido por periodo.
    Report {
        /// Periodo a consultar. Si se omite, se imprimen los cuatro.
        #[arg(long, value_enum)]
        period: Option<ReportPeriod>,

        /// Ruta de la base de datos SQLite a consultar.
        #[arg(long)]
        db: PathBuf,
    },

    /// Muestra o cambia la configuración (zona horaria, ciclo de facturación…).
    Config {
        #[command(subcommand)]
        action: ConfigAction,
    },
}

/// Acción del subcomando `config`.
#[derive(Debug, Subcommand)]
pub enum ConfigAction {
    /// Imprime la configuración actual.
    Show {
        #[arg(long)]
        db: PathBuf,
    },
    /// Cambia uno o más parámetros (solo los indicados).
    Set {
        #[arg(long)]
        db: PathBuf,
        /// Zona horaria IANA (p. ej. Europe/Madrid).
        #[arg(long)]
        timezone: Option<String>,
        /// Día del mes (1..=28) de inicio del ciclo de facturación.
        #[arg(long)]
        cycle_start_day: Option<u8>,
        /// Día de inicio de la semana.
        #[arg(long, value_enum)]
        week_start: Option<WeekStartArg>,
        /// Intervalo de muestreo en segundos.
        #[arg(long)]
        sample_interval_secs: Option<u64>,
        /// Días de retención de las muestras finas.
        #[arg(long)]
        fine_retention_days: Option<u32>,
        /// Días de retención de los agregados diarios.
        #[arg(long)]
        daily_retention_days: Option<u32>,
    },
}

/// Inicio de semana por CLI.
#[derive(Debug, Clone, Copy, ValueEnum)]
pub enum WeekStartArg {
    Monday,
    Sunday,
}

/// Periodos consultables por el subcomando `report`.
#[derive(Debug, Clone, Copy, ValueEnum)]
pub enum ReportPeriod {
    Today,
    Week,
    Month,
    LastMonth,
}
