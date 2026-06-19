# Instalación en Debian / Ubuntu

## Desde el paquete `.deb` (recomendado)

Descarga el `.deb` del [release](https://github.com/DayanCabrera2003/netusage-linux/releases)
e instálalo:

```sh
sudo apt install ./netusaged_*.deb
```

El paquete instala los binarios en `/usr/bin`, la unit systemd, y ejecuta su
`postinst` para crear el usuario de sistema `netusaged` y los directorios de
estado (`/var/lib/netusage`) y runtime (`/run/netusage`).

Activa el servicio:

```sh
sudo systemctl enable --now netusaged
netusage-tui
```

## Construir el `.deb` desde el código

Requiere la toolchain de Rust (con la nightly para el crate eBPF) y
`bpf-linker`:

```sh
sudo apt install llvm clang
cargo install bpf-linker cargo-deb

# Compila ambos binarios y arma el paquete.
cargo build --release -p netusaged -p netusage-tui
cargo deb --no-build -p netusaged
# El .deb queda en target/debian/.
```

## Versiones soportadas

- Debian 12+ y Ubuntu 22.04+ traen kernel >= 5.15 y cgroup v2 por defecto:
  modo completo sin configuración extra.
- En Ubuntu, BTF viene activado en los kernels oficiales.
