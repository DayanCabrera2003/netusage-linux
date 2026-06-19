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
mod configure;
mod counters;
mod degraded;
mod identity;
mod ipc_server;
mod loader;
mod monitor;
mod privileges;
mod report;
mod resolver;
mod sampler;
mod selftest;
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
        std::process::exit(check::run(cli.json));
    }

    if cli.selftest_load {
        std::process::exit(selftest::run());
    }

    let result = match cli.command {
        Some(Command::Run { interval_secs, db }) => run(interval_secs, db),
        Some(Command::Report { period, db }) => report::run(period, &db),
        Some(Command::Config { action }) => config_command(action),
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

/// Despacha el subcomando `config`.
fn config_command(action: crate::cli::ConfigAction) -> Result<()> {
    use crate::cli::ConfigAction;
    match action {
        ConfigAction::Show { db } => configure::show(&db),
        ConfigAction::Set {
            db,
            timezone,
            cycle_start_day,
            week_start,
            sample_interval_secs,
            fine_retention_days,
            daily_retention_days,
        } => configure::set(
            &db,
            timezone,
            cycle_start_day,
            week_start,
            sample_interval_secs,
            fine_retention_days,
            daily_retention_days,
        ),
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

    // Evaluar el entorno y decidir el modo de ejecución antes de cargar eBPF,
    // para fallar con un mensaje claro en vez de un panic en el loader.
    let env = netusage_common::preflight::EnvReport::gather();
    let decision = degraded::decide(&env);
    match decision.mode {
        degraded::RunMode::Disabled => {
            anyhow::bail!("entorno no apto para netusaged: {}", decision.reason);
        }
        degraded::RunMode::NoPerApp => {
            tracing::warn!("modo degradado: {}", decision.reason);
        }
        degraded::RunMode::Full => {
            tracing::info!("{}", decision.reason);
        }
    }

    let root = cgroup::cgroup_v2_root()?;
    let bpf = loader::load()?;

    // Si se pasó `--db`, persistir además de mostrar en vivo, y exponer el
    // socket IPC de solo lectura.
    let sampler = match db {
        Some(path) => {
            let store = Store::open(&path)
                .with_context(|| format!("abriendo la base de datos {}", path.display()))?;
            // Primer arranque: fijar la zona horaria del sistema (en vez de UTC).
            configure::ensure_first_run_config(&store)?;
            let config = store.load_config().context("cargando la configuración")?;
            tracing::info!("persistiendo muestras en {}", path.display());

            // El socket IPC es opcional: si su directorio no existe (p. ej. sin
            // la unit systemd), se avisa y el demonio sigue.
            let socket = std::path::Path::new(netusage_ipc::protocol::DEFAULT_SOCKET_PATH);
            match ipc_server::spawn(path.clone(), socket) {
                Ok(()) => tracing::info!("servidor IPC escuchando en {}", socket.display()),
                Err(err) => tracing::warn!("servidor IPC no disponible: {err:#}"),
            }

            Some(Sampler::new(store, config))
        }
        None => None,
    };

    supervisor::run(bpf, &root, Duration::from_secs(interval_secs), sampler)
}
