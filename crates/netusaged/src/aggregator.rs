//! Agregación del tráfico por aplicación a partir de los contadores por socket.
//!
//! Responsabilidad única: convertir las lecturas de los mapas por cookie en
//! totales por aplicación, usando deltas (para que el total sobreviva al
//! desalojo LRU de cookies muertas) y enviando al cubo "Sistema / Otros" el
//! tráfico de cookies sin app conocida.
//!
//! Los deltas (no los absolutos) son la base natural de la persistencia de la
//! Fase 3.

use std::collections::HashMap;

use anyhow::{Context, Result};
use aya::maps::{HashMap as BpfHashMap, MapData};
use aya::Ebpf;
use netusage_common::counters::{SocketCookie, RX_MAP_NAME, TX_MAP_NAME};

/// Clave reservada del cubo de tráfico no atribuible a una app.
pub const SYSTEM_OTHER_KEY: &str = "__system_other__";

/// Nombre legible del cubo de fallback.
pub const SYSTEM_OTHER_DISPLAY: &str = "Sistema / Otros";

/// Uso agregado de una aplicación (o del cubo de fallback).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AppUsage {
    pub app_key: String,
    pub display_name: String,
    pub rx: u64,
    pub tx: u64,
}

/// Lectura de un socket en un instante: sus contadores absolutos.
pub type CounterSample = (SocketCookie, u64, u64);

/// Acumulador con estado entre muestreos.
pub struct Aggregator {
    /// Último valor absoluto (rx, tx) visto por cookie, para calcular deltas.
    prev: HashMap<SocketCookie, (u64, u64)>,
    /// Total acumulado por `app_key`.
    totals: HashMap<String, AppUsage>,
}

impl Aggregator {
    /// Crea un agregador vacío.
    pub fn new() -> Self {
        Self {
            prev: HashMap::new(),
            totals: HashMap::new(),
        }
    }

    /// Procesa una muestra de contadores y devuelve la lista por app ordenada.
    ///
    /// `current` son los contadores absolutos de cada cookie vivo; `resolve`
    /// traduce un cookie a `(app_key, display_name)` o `None` (-> fallback). Por
    /// cada cookie se suma el delta respecto a la muestra anterior. Las cookies
    /// ausentes se podan de `prev` (sus últimos bytes, si murieron entre
    /// muestreos, se asumen perdidos: imprecisión documentada).
    pub fn sample<F>(&mut self, current: &[CounterSample], resolve: F) -> Vec<AppUsage>
    where
        F: Fn(SocketCookie) -> Option<(String, String)>,
    {
        for &(cookie, rx, tx) in current {
            let (prev_rx, prev_tx) = self.prev.get(&cookie).copied().unwrap_or((0, 0));
            // saturating: un cookie nunca decrece dentro de su vida; saturating
            // protege ante cualquier relectura inconsistente.
            let drx = rx.saturating_sub(prev_rx);
            let dtx = tx.saturating_sub(prev_tx);
            self.prev.insert(cookie, (rx, tx));

            let (app_key, display_name) = resolve(cookie).unwrap_or_else(|| {
                (
                    SYSTEM_OTHER_KEY.to_string(),
                    SYSTEM_OTHER_DISPLAY.to_string(),
                )
            });
            let entry = self.totals.entry(app_key.clone()).or_insert_with(|| AppUsage {
                app_key,
                display_name,
                rx: 0,
                tx: 0,
            });
            entry.rx = entry.rx.saturating_add(drx);
            entry.tx = entry.tx.saturating_add(dtx);
        }

        // Podar de `prev` las cookies que ya no aparecen (sockets desalojados).
        let live: std::collections::HashSet<SocketCookie> =
            current.iter().map(|&(c, _, _)| c).collect();
        self.prev.retain(|cookie, _| live.contains(cookie));

        let mut out: Vec<AppUsage> = self.totals.values().cloned().collect();
        sort_by_total(&mut out);
        out
    }
}

impl Default for Aggregator {
    fn default() -> Self {
        Self::new()
    }
}

/// Ordena la lista por bytes totales (rx + tx) descendente; ante empate,
/// alfabéticamente por nombre visible.
pub fn sort_by_total(usages: &mut [AppUsage]) {
    usages.sort_by(|a, b| {
        let total = |u: &AppUsage| u.rx as u128 + u.tx as u128;
        total(b)
            .cmp(&total(a))
            .then_with(|| a.display_name.cmp(&b.display_name))
    });
}

/// Lee ambos mapas de contadores por socket y devuelve una muestra por cookie.
pub fn read_counters(bpf: &Ebpf) -> Result<Vec<CounterSample>> {
    let rx = read_map(bpf, RX_MAP_NAME)?;
    let tx = read_map(bpf, TX_MAP_NAME)?;

    let mut cookies: Vec<SocketCookie> = rx.keys().chain(tx.keys()).copied().collect();
    cookies.sort_unstable();
    cookies.dedup();

    Ok(cookies
        .into_iter()
        .map(|cookie| {
            (
                cookie,
                rx.get(&cookie).copied().unwrap_or(0),
                tx.get(&cookie).copied().unwrap_or(0),
            )
        })
        .collect())
}

/// Lee todas las entradas de un mapa LRU a un `HashMap` de usuario.
///
/// Las entradas ilegibles (p. ej. una clave desalojada por el kernel entre la
/// enumeración y la lectura) se ignoran: no deben tumbar el muestreo.
fn read_map(bpf: &Ebpf, name: &str) -> Result<HashMap<SocketCookie, u64>> {
    let map: BpfHashMap<&MapData, SocketCookie, u64> = BpfHashMap::try_from(
        bpf.map(name)
            .with_context(|| format!("mapa eBPF '{name}' no encontrado"))?,
    )
    .with_context(|| format!("el mapa '{name}' no es un HashMap<u64, u64>"))?;
    Ok(map.iter().filter_map(|res| res.ok()).collect())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn firefox(_c: SocketCookie) -> Option<(String, String)> {
        Some(("/usr/lib/firefox/firefox".into(), "firefox".into()))
    }

    #[test]
    fn accumulates_deltas_across_samples() {
        let mut agg = Aggregator::new();
        // Primera muestra: el absoluto es el delta inicial (prev = 0).
        let out = agg.sample(&[(1, 100, 10)], firefox);
        assert_eq!((out[0].rx, out[0].tx), (100, 10));
        // Segunda muestra: solo cuenta el incremento.
        let out = agg.sample(&[(1, 150, 30)], firefox);
        assert_eq!((out[0].rx, out[0].tx), (150, 30));
    }

    #[test]
    fn unknown_cookie_goes_to_fallback() {
        let mut agg = Aggregator::new();
        let out = agg.sample(&[(9, 7, 3)], |_| None);
        assert_eq!(out.len(), 1);
        assert_eq!(out[0].app_key, SYSTEM_OTHER_KEY);
        assert_eq!(out[0].display_name, SYSTEM_OTHER_DISPLAY);
        assert_eq!((out[0].rx, out[0].tx), (7, 3));
    }

    #[test]
    fn multiple_cookies_same_app_are_summed() {
        let mut agg = Aggregator::new();
        let out = agg.sample(&[(1, 10, 1), (2, 5, 2)], firefox);
        assert_eq!(out.len(), 1);
        assert_eq!((out[0].rx, out[0].tx), (15, 3));
    }

    #[test]
    fn absent_cookie_is_pruned_from_prev() {
        let mut agg = Aggregator::new();
        agg.sample(&[(1, 100, 0), (2, 50, 0)], firefox);
        assert_eq!(agg.prev.len(), 2);
        // El cookie 2 desaparece (socket desalojado): se poda de prev.
        agg.sample(&[(1, 120, 0)], firefox);
        assert_eq!(agg.prev.len(), 1);
        assert!(agg.prev.contains_key(&1));
    }

    #[test]
    fn sorts_by_total_descending() {
        let mut agg = Aggregator::new();
        let out = agg.sample(
            &[(1, 10, 0)],
            |c| {
                if c == 1 {
                    Some(("a".into(), "a".into()))
                } else {
                    None
                }
            },
        );
        let out2 = {
            // Añadir una app con más tráfico y comprobar que queda arriba.
            agg.sample(&[(1, 10, 0), (2, 500, 0)], |c| match c {
                1 => Some(("a".into(), "a".into())),
                _ => Some(("b".into(), "b".into())),
            })
        };
        let _ = out;
        assert_eq!(out2[0].display_name, "b");
    }
}
