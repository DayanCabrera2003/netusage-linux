# Instalación manual (binario estático musl)

Para distribuciones sin paquete propio, los binarios estáticos musl no dependen
de la glibc del sistema y funcionan en cualquier Linux con kernel y cgroup
adecuados.

## Obtener los binarios

Descárgalos del [release](https://github.com/DayanCabrera2003/netusage-linux/releases)
o compílalos:

```sh
rustup target add x86_64-unknown-linux-musl
sudo <gestor-de-paquetes> install musl-tools llvm clang   # para musl-gcc y eBPF
cargo install bpf-linker

cargo build --release --target x86_64-unknown-linux-musl \
    -p netusaged -p netusage-tui
# Binarios en target/x86_64-unknown-linux-musl/release/
```

## Instalar a mano

```sh
sudo install -Dm755 netusaged    /usr/bin/netusaged
sudo install -Dm755 netusage-tui /usr/bin/netusage-tui

# Integración con systemd (desde el repo).
sudo install -Dm644 packaging/systemd/netusaged.service \
    /usr/lib/systemd/system/netusaged.service
sudo install -Dm644 packaging/systemd/sysusers.d/netusaged.conf \
    /usr/lib/sysusers.d/netusaged.conf
sudo install -Dm644 packaging/systemd/tmpfiles.d/netusaged.conf \
    /usr/lib/tmpfiles.d/netusaged.conf

# Crear el usuario de sistema y los directorios.
sudo systemd-sysusers
sudo systemd-tmpfiles --create

sudo systemctl daemon-reload
sudo systemctl enable --now netusaged
```

## Verificar

```sh
netusaged --check
sudo netusaged --selftest-load   # prueba la carga y enganche de eBPF
netusage-tui
```
