//! Punto de entrada del demonio `netusaged`.
//!
//! Responsabilidad única: inicializar el logging, parsear la CLI y despachar
//! al subcomando correspondiente. La lógica de cada acción vive en su módulo.

mod attach;
mod cgroup;
mod check;
mod cli;
mod fallback;
mod identity;
mod loader;
mod monitor;
mod resolver;
mod supervisor;

use std::time::Duration;

use anyhow::Result;
use clap::Parser;

use crate::cli::{Cli, Command};

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
        Some(Command::Run { interval_secs }) => run(interval_secs),
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
fn run(interval_secs: u64) -> Result<()> {
    let root = cgroup::cgroup_v2_root()?;
    let bpf = loader::load()?;
    supervisor::run(bpf, &root, Duration::from_secs(interval_secs))
}
