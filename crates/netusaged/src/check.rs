//! Subcomando `--check`: ejecuta las comprobaciones de entorno y presenta el
//! informe.
//!
//! Responsabilidad unica: presentar el informe por stdout y traducir el
//! veredicto a un codigo de salida del proceso.

use netusage_common::preflight;

/// Ejecuta los preflight checks, imprime el informe y devuelve el codigo de
/// salida: 0 si el sistema es apto, 1 si no.
pub fn run() -> i32 {
    let report = preflight::run_all();
    print!("{}", report.format_plain());

    if report.is_apt() {
        println!("Resultado: el sistema es apto.");
        0
    } else {
        println!("Resultado: el sistema NO es apto (ver lineas [FAIL]).");
        1
    }
}
