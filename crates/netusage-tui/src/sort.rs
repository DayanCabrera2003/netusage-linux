//! Ordenación de la lista de apps por consumo.

use crate::model::AppUsage;

/// Ordena `apps` por consumo total descendente; ante empate, por nombre visible
/// ascendente (orden determinista para un render estable y para los tests).
pub fn sort_by_usage(apps: &mut [AppUsage]) {
    apps.sort_by(|a, b| {
        b.total()
            .cmp(&a.total())
            .then_with(|| a.display_name.cmp(&b.display_name))
    });
}

#[cfg(test)]
mod tests {
    use super::sort_by_usage;
    use crate::model::AppUsage;

    fn app(name: &str, rx: u64, tx: u64) -> AppUsage {
        AppUsage {
            app_key: format!("/{name}"),
            display_name: name.to_string(),
            rx_bytes: rx,
            tx_bytes: tx,
        }
    }

    #[test]
    fn orders_descending_by_total() {
        let mut apps = vec![app("a", 10, 0), app("b", 500, 0), app("c", 50, 0)];
        sort_by_usage(&mut apps);
        let names: Vec<_> = apps.iter().map(|a| a.display_name.as_str()).collect();
        assert_eq!(names, ["b", "c", "a"]);
    }

    #[test]
    fn ties_break_by_name() {
        let mut apps = vec![app("zeta", 100, 0), app("alfa", 50, 50)];
        sort_by_usage(&mut apps);
        // Ambas suman 100; desempata por nombre: alfa antes que zeta.
        assert_eq!(apps[0].display_name, "alfa");
    }

    #[test]
    fn empty_list_does_not_panic() {
        let mut apps: Vec<AppUsage> = Vec::new();
        sort_by_usage(&mut apps);
        assert!(apps.is_empty());
    }
}
