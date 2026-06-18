//! Script de compilación de `netusaged`.
//!
//! Compila el crate eBPF (`netusage-ebpf`) y exporta la ruta del objeto
//! resultante en la variable de entorno `NETUSAGE_EBPF_OBJ`, para que el
//! cargador la embeba con `aya::include_bytes_aligned!`.
//!
//! El crate eBPF se compila aparte (target `bpfel-unknown-none`, nightly,
//! build-std) y queda fuera del workspace. Para que apliquen su
//! `rust-toolchain.toml` (nightly) y su `.cargo/config.toml` (target BPF), se
//! invoca `cargo` en su directorio limpiando las variables que el build padre
//! inyecta (RUSTUP_TOOLCHAIN, RUSTC, RUSTFLAGS, CARGO*), que de otro modo
//! forzarían el toolchain estable y el target del host.

use std::path::PathBuf;
use std::process::Command;

fn main() {
    let manifest_dir = PathBuf::from(std::env::var("CARGO_MANIFEST_DIR").unwrap());
    let ebpf_dir = manifest_dir
        .join("../netusage-ebpf")
        .canonicalize()
        .expect("no se encontró el crate netusage-ebpf");

    // Recompilar si cambian las fuentes del crate eBPF.
    println!("cargo:rerun-if-changed={}", ebpf_dir.join("src").display());
    println!(
        "cargo:rerun-if-changed={}",
        ebpf_dir.join("Cargo.toml").display()
    );

    let mut cmd = Command::new("cargo");
    cmd.current_dir(&ebpf_dir).arg("build").arg("--release");

    // Aislar el entorno para que el crate eBPF use su propio toolchain/target.
    for (key, _) in std::env::vars() {
        if key == "CARGO_HOME" || key == "RUSTUP_HOME" {
            continue;
        }
        if key.starts_with("CARGO")
            || key.starts_with("RUSTC")
            || key == "RUSTUP_TOOLCHAIN"
            || key == "RUSTFLAGS"
        {
            cmd.env_remove(key);
        }
    }

    let status = cmd
        .status()
        .expect("no se pudo ejecutar cargo para netusage-ebpf");
    assert!(status.success(), "falló la compilación de netusage-ebpf");

    let obj = ebpf_dir.join("target/bpfel-unknown-none/release/netusage-ebpf");
    assert!(
        obj.is_file(),
        "objeto eBPF no encontrado en {}",
        obj.display()
    );

    println!("cargo:rerun-if-changed={}", obj.display());
    println!("cargo:rustc-env=NETUSAGE_EBPF_OBJ={}", obj.display());
}
