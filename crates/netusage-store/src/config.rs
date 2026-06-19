//! Configuración persistida (tabla `config`, clave-valor JSON).
//!
//! Responsabilidad única: leer/escribir `StoreConfig` y exponer los parámetros
//! que gobiernan la agregación temporal (intervalo de muestreo, ciclo de
//! facturación, inicio de semana, zona horaria, retención).

use chrono::Weekday;
use chrono_tz::Tz;
use rusqlite::params;
use serde::{Deserialize, Serialize};

use crate::error::{Result, StoreError};
use crate::store::Store;

/// Clave bajo la que se guarda el struct completo en la tabla `config`.
const CONFIG_KEY: &str = "config";

/// Día de inicio de la semana configurable. Solo lunes o domingo, que son las
/// convenciones habituales.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum WeekStart {
    Monday,
    Sunday,
}

impl WeekStart {
    /// Traduce a `chrono::Weekday` para la aritmética de calendario.
    pub fn weekday(self) -> Weekday {
        match self {
            WeekStart::Monday => Weekday::Mon,
            WeekStart::Sunday => Weekday::Sun,
        }
    }
}

/// Parámetros de persistencia y agregación temporal.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct StoreConfig {
    /// Cada cuántos segundos muestrea el demonio.
    pub sample_interval_secs: u64,
    /// Día del mes (1..=28) en que arranca el ciclo de facturación.
    pub cycle_start_day: u8,
    /// Día en que empieza la semana.
    pub week_start: WeekStart,
    /// Zona horaria IANA (p. ej. "Europe/Madrid").
    pub timezone: String,
    /// Días que se conservan las muestras finas antes de compactarlas.
    pub fine_retention_days: u32,
    /// Días que se conservan los agregados diarios.
    pub daily_retention_days: u32,
}

impl Default for StoreConfig {
    fn default() -> Self {
        Self {
            sample_interval_secs: 2,
            cycle_start_day: 1,
            week_start: WeekStart::Monday,
            timezone: "UTC".to_string(),
            fine_retention_days: 14,
            daily_retention_days: 730,
        }
    }
}

impl StoreConfig {
    /// Resuelve la zona horaria a `chrono_tz::Tz`, validando el nombre IANA.
    pub fn tz(&self) -> Result<Tz> {
        self.timezone
            .parse::<Tz>()
            .map_err(|_| StoreError::UnknownTimezone(self.timezone.clone()))
    }
}

impl Store {
    /// Carga la configuración. Si no hay ninguna guardada, devuelve los valores
    /// por defecto. Valida que la zona horaria sea un nombre IANA conocido.
    pub fn load_config(&self) -> Result<StoreConfig> {
        let json: Option<String> = self
            .conn
            .query_row(
                "SELECT value FROM config WHERE key = ?1",
                [CONFIG_KEY],
                |row| row.get(0),
            )
            .ok();
        let config = match json {
            Some(json) => serde_json::from_str(&json)?,
            None => StoreConfig::default(),
        };
        config.tz()?; // valida la zona horaria
        Ok(config)
    }

    /// Indica si ya hay una configuración guardada (para distinguir el primer
    /// arranque, donde conviene autodetectar la zona horaria).
    pub fn config_exists(&self) -> Result<bool> {
        let count: i64 = self.conn.query_row(
            "SELECT count(*) FROM config WHERE key = ?1",
            [CONFIG_KEY],
            |row| row.get(0),
        )?;
        Ok(count > 0)
    }

    /// Guarda la configuración (sustituye la anterior).
    pub fn save_config(&self, config: &StoreConfig) -> Result<()> {
        config.tz()?; // no permitir persistir una zona horaria inválida
        let json = serde_json::to_string(config)?;
        self.conn.execute(
            "INSERT INTO config(key, value) VALUES (?1, ?2)
             ON CONFLICT(key) DO UPDATE SET value = excluded.value",
            params![CONFIG_KEY, json],
        )?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn missing_config_returns_defaults() {
        let store = Store::open_in_memory().unwrap();
        assert_eq!(store.load_config().unwrap(), StoreConfig::default());
    }

    #[test]
    fn save_then_load_roundtrips() {
        let store = Store::open_in_memory().unwrap();
        let cfg = StoreConfig {
            sample_interval_secs: 5,
            cycle_start_day: 15,
            week_start: WeekStart::Sunday,
            timezone: "Europe/Madrid".to_string(),
            fine_retention_days: 7,
            daily_retention_days: 365,
        };
        store.save_config(&cfg).unwrap();
        assert_eq!(store.load_config().unwrap(), cfg);
    }

    #[test]
    fn invalid_timezone_is_rejected() {
        let store = Store::open_in_memory().unwrap();
        let cfg = StoreConfig {
            timezone: "Mars/Olympus".to_string(),
            ..StoreConfig::default()
        };
        assert!(store.save_config(&cfg).is_err());
    }
}
