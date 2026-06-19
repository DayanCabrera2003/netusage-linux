//! Subcomando `config` y autodetección de zona horaria.
//!
//! Responsabilidad única: leer/escribir la `StoreConfig` desde la CLI y, en el
//! primer arranque (base sin configuración), fijar la zona horaria del sistema
//! para que "hoy/semana/mes" se calculen en hora local y no en UTC.

use std::path::Path;

use anyhow::{Context, Result};
use netusage_store::{Store, StoreConfig, WeekStart};

use crate::cli::WeekStartArg;

/// Detecta el nombre IANA de la zona horaria del sistema.
///
/// `/etc/localtime` suele ser un enlace a `.../zoneinfo/<Area/Ciudad>`; si no,
/// se prueba `/etc/timezone` (Debian). Devuelve `None` si no se puede deducir.
pub fn detect_local_timezone() -> Option<String> {
    if let Ok(target) = std::fs::read_link("/etc/localtime") {
        let s = target.to_string_lossy();
        if let Some(idx) = s.find("zoneinfo/") {
            let tz = s[idx + "zoneinfo/".len()..].trim_matches('/').to_string();
            if !tz.is_empty() {
                return Some(tz);
            }
        }
    }
    std::fs::read_to_string("/etc/timezone")
        .ok()
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
}

/// En el primer arranque (sin config guardada) escribe una config con la zona
/// horaria del sistema. Si ya hay config, no toca nada.
pub fn ensure_first_run_config(store: &Store) -> Result<()> {
    if store.config_exists()? {
        return Ok(());
    }
    let default = StoreConfig::default();
    // Intentar con la zona detectada; `save_config` valida el nombre IANA. Si la
    // detección falla o no es válida, se guarda el default (UTC).
    if let Some(tz) = detect_local_timezone() {
        let candidate = StoreConfig {
            timezone: tz,
            ..default.clone()
        };
        if store.save_config(&candidate).is_ok() {
            return Ok(());
        }
    }
    store
        .save_config(&default)
        .context("guardando la configuración inicial")?;
    Ok(())
}

/// Imprime la configuración actual.
pub fn show(db: &Path) -> Result<()> {
    let store = open(db)?;
    let cfg = store.load_config().context("cargando la configuración")?;
    println!("timezone            = {}", cfg.timezone);
    println!("cycle_start_day     = {}", cfg.cycle_start_day);
    println!("week_start          = {:?}", cfg.week_start);
    println!("sample_interval_secs= {}", cfg.sample_interval_secs);
    println!("fine_retention_days = {}", cfg.fine_retention_days);
    println!("daily_retention_days= {}", cfg.daily_retention_days);
    Ok(())
}

/// Aplica los cambios indicados (solo los `Some`) y guarda.
#[allow(clippy::too_many_arguments)]
pub fn set(
    db: &Path,
    timezone: Option<String>,
    cycle_start_day: Option<u8>,
    week_start: Option<WeekStartArg>,
    sample_interval_secs: Option<u64>,
    fine_retention_days: Option<u32>,
    daily_retention_days: Option<u32>,
) -> Result<()> {
    let store = open(db)?;
    let mut cfg = store.load_config().context("cargando la configuración")?;

    if let Some(tz) = timezone {
        cfg.timezone = tz;
    }
    if let Some(day) = cycle_start_day {
        cfg.cycle_start_day = day.clamp(1, 28);
    }
    if let Some(ws) = week_start {
        cfg.week_start = match ws {
            WeekStartArg::Monday => WeekStart::Monday,
            WeekStartArg::Sunday => WeekStart::Sunday,
        };
    }
    if let Some(s) = sample_interval_secs {
        cfg.sample_interval_secs = s.max(1);
    }
    if let Some(d) = fine_retention_days {
        cfg.fine_retention_days = d;
    }
    if let Some(d) = daily_retention_days {
        cfg.daily_retention_days = d;
    }

    store
        .save_config(&cfg)
        .context("guardando la configuración (¿zona horaria válida?)")?;
    println!("configuración guardada.");
    Ok(())
}

/// Abre la base para leer/escribir la configuración, fijando la zona horaria
/// del sistema si es la primera vez.
fn open(db: &Path) -> Result<Store> {
    let store =
        Store::open(db).with_context(|| format!("abriendo la base de datos {}", db.display()))?;
    ensure_first_run_config(&store)?;
    Ok(store)
}

#[cfg(test)]
mod tests {
    use super::detect_local_timezone;

    #[test]
    fn detection_returns_a_plausible_value_or_none() {
        // No imponemos un valor concreto (depende de la máquina); solo que, si
        // devuelve algo, no esté vacío.
        if let Some(tz) = detect_local_timezone() {
            assert!(!tz.is_empty());
        }
    }
}
