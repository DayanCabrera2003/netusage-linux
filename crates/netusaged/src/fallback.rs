//! Política de fallback y agregación del tráfico por aplicación.
//!
//! Responsabilidad única: convertir las muestras por cgroup (inode -> rx/tx)
//! en una lista agregada por aplicación, enviando al cubo "Sistema / Otros"
//! todo el tráfico cuyo cgroup no se reconoce como una app.
//!
//! Cae en "Sistema / Otros" el tráfico de cgroups que no están en el registro:
//! `session.scope`, servicios de sistema, `init.scope`, o cgroups que
//! aparecieron y desaparecieron entre escaneos (carreras). Reduce el tamaño de
//! este cubo, como evolución futura no implementada aquí, envolver los
//! lanzamientos del usuario con `systemd-run --user --scope` para forzar un
//! scope propio por proceso.

use std::collections::HashMap;

use netusage_common::counters::CgroupInode;

/// Clave reservada del cubo de tráfico no atribuible a una app.
pub const SYSTEM_OTHER_KEY: &str = "__system_other__";

/// Nombre legible del cubo de fallback.
pub const SYSTEM_OTHER_DISPLAY: &str = "Sistema / Otros";

/// Muestra de contadores de un cgroup leída de los mapas eBPF.
#[derive(Debug, Clone, Copy)]
pub struct CgroupSample {
    pub inode: CgroupInode,
    pub rx: u64,
    pub tx: u64,
}

/// Uso agregado de una aplicación (o del cubo de fallback).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AppUsage {
    pub app_key: String,
    pub display_name: String,
    pub rx: u64,
    pub tx: u64,
}

/// Agrega las `samples` por aplicación.
///
/// `resolve` traduce el inode de un cgroup a su identidad `(app_key,
/// display_name)`; devuelve `None` si el cgroup no es una app conocida, en cuyo
/// caso su tráfico se suma al cubo "Sistema / Otros".
///
/// El resultado se ordena por bytes totales (rx + tx) de forma descendente, de
/// modo que las apps que más consumen quedan arriba. El cubo de fallback se
/// trata como una "app" más a efectos de orden.
pub fn aggregate<F>(samples: impl IntoIterator<Item = CgroupSample>, resolve: F) -> Vec<AppUsage>
where
    F: Fn(CgroupInode) -> Option<(String, String)>,
{
    let mut by_key: HashMap<String, AppUsage> = HashMap::new();

    for sample in samples {
        let (app_key, display_name) = resolve(sample.inode).unwrap_or_else(|| {
            (
                SYSTEM_OTHER_KEY.to_string(),
                SYSTEM_OTHER_DISPLAY.to_string(),
            )
        });

        let entry = by_key.entry(app_key.clone()).or_insert_with(|| AppUsage {
            app_key,
            display_name,
            rx: 0,
            tx: 0,
        });
        entry.rx = entry.rx.saturating_add(sample.rx);
        entry.tx = entry.tx.saturating_add(sample.tx);
    }

    let mut out: Vec<AppUsage> = by_key.into_values().collect();
    out.sort_by(|a, b| {
        let total = |u: &AppUsage| u.rx as u128 + u.tx as u128;
        total(b)
            .cmp(&total(a))
            .then_with(|| a.display_name.cmp(&b.display_name))
    });
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample(inode: CgroupInode, rx: u64, tx: u64) -> CgroupSample {
        CgroupSample { inode, rx, tx }
    }

    #[test]
    fn known_apps_and_fallback_are_separated() {
        // Registro parcial: 10 -> firefox, 20 -> konsole. El resto es fallback.
        let resolve = |inode: CgroupInode| match inode {
            10 => Some(("firefox".to_string(), "firefox".to_string())),
            20 => Some(("org.kde.konsole".to_string(), "konsole".to_string())),
            _ => None,
        };

        let samples = vec![
            sample(10, 100, 10),
            sample(20, 50, 5),
            sample(30, 7, 3),  // desconocido -> Sistema / Otros
            sample(40, 1, 1),  // desconocido -> Sistema / Otros
        ];

        let agg = aggregate(samples, resolve);

        // firefox arriba (110 total), luego konsole (55), luego fallback (12).
        assert_eq!(agg[0].app_key, "firefox");
        assert_eq!((agg[0].rx, agg[0].tx), (100, 10));
        assert_eq!(agg[1].display_name, "konsole");

        let other = agg
            .iter()
            .find(|u| u.app_key == SYSTEM_OTHER_KEY)
            .expect("debe existir el cubo de fallback");
        assert_eq!((other.rx, other.tx), (8, 4));
        assert_eq!(other.display_name, SYSTEM_OTHER_DISPLAY);
    }

    #[test]
    fn multiple_cgroups_of_same_app_are_summed() {
        // Dos cgroups distintos resuelven a la misma app: se acumulan.
        let resolve = |_inode: CgroupInode| Some(("firefox".to_string(), "firefox".to_string()));
        let agg = aggregate(vec![sample(1, 10, 1), sample(2, 5, 2)], resolve);
        assert_eq!(agg.len(), 1);
        assert_eq!((agg[0].rx, agg[0].tx), (15, 3));
    }

    #[test]
    fn empty_input_yields_empty_output() {
        let agg = aggregate(Vec::<CgroupSample>::new(), |_| None);
        assert!(agg.is_empty());
    }
}
