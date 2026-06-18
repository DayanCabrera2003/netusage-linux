//! Punto de entrada del demonio `netusaged`.
//!
//! Responsabilidad única: inicializar el logging, parsear la CLI y despachar
//! al subcomando correspondiente. La lógica de cada acción vive en su módulo.

mod cgroup;
mod check;
mod cli;
mod loader;
mod monitor;

use std::time::Duration;

use anyhow::{Context, Result};
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

/// Engancha los programas eBPF al cgroup raíz y monitoriza el tráfico total.
fn run(interval_secs: u64) -> Result<()> {
    let cgroup_path = cgroup::cgroup_v2_root()?;
    let cgroup = std::fs::File::open(&cgroup_path)
        .with_context(|| format!("abriendo el cgroup {}", cgroup_path.display()))?;

    // El handle eBPF debe vivir durante toda la monitorización: al dropearse se
    // desenganchan los programas.
    let bpf = loader::load_and_attach(&cgroup)?;
    tracing::info!(
        "monitorizando el tráfico total en {} (Ctrl-C para salir)",
        cgroup_path.display()
    );

    monitor::run_monitor(&bpf, Duration::from_secs(interval_secs))
}
