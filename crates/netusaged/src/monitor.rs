//! Presentación de la lista por aplicación.
//!
//! Responsabilidad única: dar formato humano y mostrar la lista de uso por
//! aplicación que produce el agregador.

use crate::aggregator::{AppUsage, SYSTEM_OTHER_KEY};

/// Imprime la lista por aplicación: `display_name | rx | tx`, ya ordenada por el
/// agregador (mayor consumo arriba). El cubo de fallback se marca con `*`.
pub fn print_app_list(usages: &[AppUsage]) {
    println!("--- uso por aplicación ---");
    if usages.is_empty() {
        println!("(sin tráfico atribuido todavía)");
        return;
    }
    for usage in usages {
        let marker = if usage.app_key == SYSTEM_OTHER_KEY {
            "*"
        } else {
            " "
        };
        println!(
            "{marker} {:<28} rx={:>12}  tx={:>12}",
            usage.display_name,
            human_bytes(usage.rx),
            human_bytes(usage.tx),
        );
    }
}

/// Formatea una cantidad de bytes en unidades binarias legibles.
///
/// Por debajo de 1 KiB se muestra el valor exacto en bytes; a partir de ahí se
/// usan dos decimales (KiB/MiB/GiB/TiB).
pub fn human_bytes(bytes: u64) -> String {
    const UNITS: [&str; 5] = ["B", "KiB", "MiB", "GiB", "TiB"];

    if bytes < 1024 {
        return format!("{bytes} B");
    }

    let mut value = bytes as f64;
    let mut unit = 0;
    while value >= 1024.0 && unit < UNITS.len() - 1 {
        value /= 1024.0;
        unit += 1;
    }

    format!("{value:.2} {}", UNITS[unit])
}

#[cfg(test)]
mod tests {
    use super::human_bytes;

    #[test]
    fn formats_bytes_in_binary_units() {
        assert_eq!(human_bytes(0), "0 B");
        assert_eq!(human_bytes(1023), "1023 B");
        assert_eq!(human_bytes(1024), "1.00 KiB");
        assert_eq!(human_bytes(1_048_576), "1.00 MiB");
        assert_eq!(human_bytes(1_073_741_824), "1.00 GiB");
    }
}
