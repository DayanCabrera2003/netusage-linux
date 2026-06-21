//! Subcomando `clear`: borrado manual del consumo de un periodo.
//!
//! Responsabilidad unica: abrir la base en lectura-escritura, mostrar un
//! preview, confirmar (salvo `--yes`) y delegar el borrado en el store. No
//! carga eBPF ni exige capabilities (igual que `report`/`config`).

use std::io::{self, Write};
use std::path::Path;

use anyhow::{Context, Result};
use chrono::Utc;
use netusage_store::Store;

use crate::cli::ReportPeriod;
use crate::monitor::human_bytes;

/// Ejecuta el borrado del periodo. Con `yes`, no pide confirmacion.
pub fn run(period: ReportPeriod, db: &Path, yes: bool) -> Result<()> {
    let mut store =
        Store::open(db).with_context(|| format!("abriendo la base de datos {}", db.display()))?;
    let now = Utc::now();
    let store_period = period.to_store();

    // Preview informativo: el demonio puede insertar nuevas muestras en el rango
    // entre esta consulta y el borrado, asi que es orientativo.
    let total = store
        .usage_total(store_period, now)
        .context("consultando el periodo")?;
    println!(
        "Se borrara el consumo de {}: rx={}, tx={}.",
        period_label(period),
        human_bytes(total.rx_bytes as u64),
        human_bytes(total.tx_bytes as u64),
    );

    if !yes && !confirm()? {
        println!("Cancelado.");
        return Ok(());
    }

    let (n_samples, n_daily) = store
        .delete_period(store_period, now)
        .context("no se pudo borrar (¿base ocupada por el demonio? reintenta)")?;
    println!("Borrado: {n_samples} muestras finas y {n_daily} agregados diarios.");
    Ok(())
}

/// Etiqueta legible del periodo.
fn period_label(p: ReportPeriod) -> &'static str {
    match p {
        ReportPeriod::Today => "hoy",
        ReportPeriod::Week => "esta semana",
        ReportPeriod::Month => "este mes",
        ReportPeriod::LastMonth => "el mes anterior",
    }
}

/// Pide confirmacion por stdin. Verdadero solo ante s/si/y/yes.
fn confirm() -> Result<bool> {
    print!("Borrar? [s/N]: ");
    io::stdout().flush().ok();
    let mut line = String::new();
    io::stdin().read_line(&mut line)?;
    Ok(matches!(
        line.trim().to_lowercase().as_str(),
        "s" | "si" | "sí" | "y" | "yes"
    ))
}
