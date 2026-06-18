//! Punto de entrada del demonio `netusaged`.
//!
//! Responsabilidad unica: inicializar el logging, parsear la CLI y despachar
//! al subcomando correspondiente.

mod cgroup;
mod check;
mod cli;
mod loader;
mod monitor;

use clap::Parser;

use crate::cli::Cli;

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

    println!("netusaged: sin accion. Usa --check para diagnosticar el entorno.");
}
