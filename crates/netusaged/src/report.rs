//! Subcomando `report`: consulta del consumo persistido por periodo.
//!
//! Responsabilidad única: abrir el `Store`, resolver los periodos pedidos y
//! presentar total y desglose por app con bytes formateados.

use std::path::Path;

use anyhow::{Context, Result};
use chrono::Utc;
use netusage_store::Store;

use crate::cli::ReportPeriod;
use crate::monitor::human_bytes;

/// Ejecuta el reporte. Si `period` es `None`, imprime los cuatro periodos.
pub fn run(period: Option<ReportPeriod>, db: &Path) -> Result<()> {
    let store =
        Store::open(db).with_context(|| format!("abriendo la base de datos {}", db.display()))?;
    let now = Utc::now();

    let periods: &[ReportPeriod] = match &period {
        Some(p) => std::slice::from_ref(p),
        None => &[
            ReportPeriod::Today,
            ReportPeriod::Week,
            ReportPeriod::Month,
            ReportPeriod::LastMonth,
        ],
    };
    for period in periods {
        print_period(&store, *period, now)?;
    }
    Ok(())
}

/// Imprime el total y el desglose por app de un periodo.
fn print_period(
    store: &Store,
    report_period: ReportPeriod,
    now: chrono::DateTime<Utc>,
) -> Result<()> {
    let label = match report_period {
        ReportPeriod::Today => "Hoy",
        ReportPeriod::Week => "Esta semana",
        ReportPeriod::Month => "Este mes",
        ReportPeriod::LastMonth => "Mes anterior",
    };
    let period = report_period.to_store();

    let total = store.usage_total(period, now)?;
    let apps = store.usage_by_app(period, now)?;

    println!("=== {label} ===");
    println!(
        "  {:<28} rx={:>12}  tx={:>12}",
        "TOTAL",
        human_bytes(total.rx_bytes as u64),
        human_bytes(total.tx_bytes as u64),
    );
    if apps.is_empty() {
        println!("  (sin datos en el periodo)");
    }
    for app in apps {
        println!(
            "  {:<28} rx={:>12}  tx={:>12}",
            app.display_name,
            human_bytes(app.rx_bytes as u64),
            human_bytes(app.tx_bytes as u64),
        );
    }
    Ok(())
}
