# netusage-linux

Monitor de uso de datos por aplicación para Linux de escritorio, al estilo de
la pantalla de "Uso de datos" de Android. Mide el consumo de red total y por
aplicación usando eBPF (`cgroup_skb`) sobre los scopes de cgroup v2 de systemd.

Lenguaje principal: Rust. Distros objetivo: Debian, Arch, Fedora y derivadas.

## Instalacion

Requisitos: kernel >= 5.8, cgroup v2 unificado y BTF
(`/sys/kernel/btf/vmlinux`). Verifica tu entorno con:

```sh
netusaged --check
```

Guias por distribucion:

- [Debian / Ubuntu](docs/install/debian-ubuntu.md) — paquete `.deb`
- [Fedora / openSUSE](docs/install/fedora.md) — paquete `.rpm`
- [Arch / derivadas](docs/install/arch.md) — PKGBUILD
- [Cualquier distro](docs/install/manual-musl.md) — binario estatico musl

## Stack eBPF

Stack eBPF: aya (todo en Rust, kernel + usuario). Ver la decisión 0001 en la
documentación del proyecto para el criterio y el plan de fallback a libbpf-rs.

## Prerrequisitos de toolchain

- Rust estable (canal fijado en `rust-toolchain.toml`) para el espacio de
  usuario.
- Rust `nightly` con el componente `rust-src` para compilar el crate eBPF
  (`crates/netusage-ebpf` lleva su propio `rust-toolchain.toml`):
  `rustup toolchain install nightly --component rust-src`.
- `bpf-linker` para enlazar el objeto eBPF: `cargo install bpf-linker`.
- `bpftool` para diagnosticar y para cargar/descargar programas eBPF en las
  pruebas (Fedora: `sudo dnf install bpftool`; Debian: `linux-tools-common`;
  Arch: `bpf`).

Cargar y enganchar programas eBPF requiere privilegios: root, o las
capabilities `CAP_BPF` + `CAP_PERFMON` + `CAP_NET_ADMIN` (kernel >= 5.8). En
producción se otorgan vía systemd (Fase 4).

## Diagnostico del entorno

El demonio incluye un subcomando que comprueba si el sistema es apto (cgroup v2
unificado, BTF, versión de kernel y privilegios) e imprime un informe:

```
cargo run -p netusaged -- --check
```

Devuelve código de salida 0 si el sistema es apto. Las comprobaciones de
privilegios solo avisan (`WARN`) cuando se ejecuta sin root, porque `--check`
puede usarse solo para diagnosticar.

## Construir el programa eBPF

El crate eBPF se compila aparte (target `bpfel-unknown-none`, nightly,
`build-std`):

```
cd crates/netusage-ebpf
cargo build --release
```

El objeto resultante queda en
`crates/netusage-ebpf/target/bpfel-unknown-none/release/netusage-ebpf`.
