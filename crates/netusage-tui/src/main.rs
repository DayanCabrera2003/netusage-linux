//! Interfaz de terminal (TUI) de netusage.
//!
//! Muestra el consumo de red total y por aplicación, con conmutador de periodo
//! (hoy / semana / mes / mes anterior), leyendo la base del demonio en modo
//! solo lectura.
//!
//! Arquitectura por capas: datos (`data`) y modelo (`model`) sin dependencias
//! de ratatui; estado y reductor (`state`, `update`); widgets de render puros
//! (`ui`); y la orquestación del bucle (`app`, `event`).

// Durante la construcción incremental hay módulos aún no cableados; el `app`
// final (commit de cableado) los usa todos y entonces se retira este allow.
#![allow(dead_code)]

mod data;
mod error;
mod format;
mod model;
mod period;
mod sort;
mod state;
mod update;

fn main() {
    println!("netusage-tui (placeholder)");
}
