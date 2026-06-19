//! Punto de entrada del demonio `netusaged`.
//!
//! Responsabilidad única: inicializar el logging, parsear la CLI y despachar
//! al subcomando correspondiente. La lógica de cada acción vive en su módulo.

mod aggregator;
mod attach;
mod backfill;
mod cgroup;
mod check;
mod cli;
mod counters;
mod identity;
mod loader;
mod monitor;
mod privileges;
mod report;
mod resolver;
mod sampler;
mod supervisor;

use std::path::PathBuf;
use std::time::Duration;

use anyhow::{Context, Result};
use clap::Parser;
use netusage_store::Store;

use crate::cli::{Cli, Command};
use crate::sampler::Sampler;

fn main() {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info")),
        )
        .init();

    let cli = Cli::parse();

    if cli.check {
        std::process::exit(check::run());
    }

    let result = match cli.command {
        Some(Command::Run { interval_secs, db }) => run(interval_secs, db),
        Some(Command::Report { period, db }) => report::run(period, &db),
        None => {
            println!(
                "netusaged: sin acción. Usa --check para diagnosticar o `run` para monitorizar."
            );
            Ok(())
        }
    };

    if let Err(err) = result {
        eprintln!("error: {err:#}");
        std::process::exit(1);
    }
}

/// Carga el objeto eBPF y arranca la atribución de tráfico por aplicación.
///
/// El handle eBPF se transfiere al `supervisor`, que lo mantiene vivo durante
/// toda la monitorización: al dropearse se desenganchan los programas y se
/// liberan los mapas.
fn run(interval_secs: u64, db: Option<PathBuf>) -> Result<()> {
    // Verificar el privilegio mínimo y reportarlo antes de tocar eBPF.
    let mode = privileges::ensure_minimum()?;
    tracing::info!("netusaged corriendo con {}", mode.describe());

    let root = cgroup::cgroup_v2_root()?;
    let bpf = loader::load()?;

    // Si se pasó `--db`, persistir además de mostrar en vivo.
    let sampler = match db {
        Some(path) => {
            let store = Store::open(&path)
                .with_context(|| format!("abriendo la base de datos {}", path.display()))?;
            let config = store.load_config().context("cargando la configuración")?;
            tracing::info!("persistiendo muestras en {}", path.display());
            Some(Sampler::new(store, config))
        }
        None => None,
    };

    supervisor::run(bpf, &root, Duration::from_secs(interval_secs), sampler)
}
