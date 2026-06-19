//! Subcomando `--check`: ejecuta las comprobaciones de entorno y presenta el
//! informe.
//!
//! Responsabilidad unica: presentar el informe por stdout y traducir el
//! veredicto a un codigo de salida del proceso.

use netusage_common::preflight;
use netusage_common::preflight::EnvReport;

/// Ejecuta los preflight checks y presenta el resultado.
///
/// Con `json = true` imprime el `EnvReport` en una linea JSON (consumo
/// programatico); en caso contrario imprime el informe legible. En ambos casos
/// devuelve el codigo de salida: 0 si el sistema es apto, 1 si no.
pub fn run(json: bool) -> i32 {
    if json {
        return run_json();
    }

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

/// Imprime el informe de entorno como JSON y deriva el codigo de salida de las
/// banderas que bloquean (cgroup v2 y BTF son requisitos duros).
fn run_json() -> i32 {
    let env = EnvReport::gather();
    println!("{}", env.to_json());
    if env.cgroup_v2 && env.btf {
        0
    } else {
        1
    }
}
