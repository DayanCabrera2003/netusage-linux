//! Interfaz de terminal (TUI) de netusage.
//!
//! Muestra el consumo de red total y por aplicación, con conmutador de periodo
//! (hoy / semana / mes / mes anterior), leyendo la base del demonio en modo
//! solo lectura.
//!
//! Arquitectura por capas: datos (`data`) y modelo (`model`) sin dependencias
//! de ratatui; estado y reductor (`state`, `update`); widgets de render puros
//! (`ui`); y la orquestación del bucle (`app`, `event`).

mod app;
mod cli;
mod data;
mod error;
mod event;
mod format;
mod health;
mod model;
mod period;
mod release;
mod sort;
mod state;
mod ui;
mod update;
mod vpn;

use clap::Parser;

use crate::cli::Cli;

fn main() {
    let cli = Cli::parse();
    if let Err(err) = app::run(cli) {
        eprintln!("error: {err}");
        std::process::exit(1);
    }
}
