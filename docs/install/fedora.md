# Instalación en Fedora / openSUSE

## Desde el paquete `.rpm` (recomendado)

Descarga el `.rpm` del [release](https://github.com/DayanCabrera2003/netusage-linux/releases)
e instálalo:

```sh
sudo dnf install ./netusaged-*.rpm     # Fedora
sudo zypper install ./netusaged-*.rpm  # openSUSE
```

El paquete instala los binarios en `/usr/bin`, la unit systemd, y su scriptlet
de postinstalación crea el usuario de sistema `netusaged` y los directorios de
estado y runtime mediante `systemd-sysusers` y `systemd-tmpfiles`, y además
**activa y arranca el servicio** automáticamente. Al arrancar, el demonio crea
la base de datos.

Comprueba que está activo y abre la interfaz:

```sh
systemctl status netusaged
netusage-tui
```

Si no quedó activo (p. ej. instalación en un entorno sin systemd corriendo),
actívalo a mano:

```sh
sudo systemctl enable --now netusaged
```

## Construir el `.rpm` desde el código

```sh
sudo dnf install llvm clang
cargo install bpf-linker cargo-generate-rpm

cargo build --release -p netusaged -p netusage-tui
cargo generate-rpm -p crates/netusaged
# El .rpm queda en target/generate-rpm/.
```

## Notas

- Fedora trae cgroup v2 unificado y BTF activados por defecto desde hace
  varias versiones: modo completo sin configuración extra.
- El binario empaquetado enlaza con la glibc del sistema. Para máxima
  portabilidad entre distribuciones usa el [binario estático
  musl](manual-musl.md).
