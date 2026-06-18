//! Punto de entrada del demonio `netusaged`.
//!
//! Responsabilidad unica: inicializar el logging, parsear la CLI y despachar
//! al subcomando correspondiente.

mod cli;

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
        // La logica real de los preflight checks se conecta en un commit
        // posterior de la Fase 0 (modulo `check`).
        println!("netusaged --check: comprobacion de entorno no implementada todavia");
        std::process::exit(0);
    }

    println!("netusaged: sin accion. Usa --check para diagnosticar el entorno.");
}
