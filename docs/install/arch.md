# Instalación en Arch Linux / derivadas

## Desde el AUR

El `PKGBUILD` vive en [`packaging/aur/PKGBUILD`](../../packaging/aur/PKGBUILD).
Compila desde el código fuente.

```sh
# Con un ayudante del AUR (cuando esté publicado):
# yay -S netusage

# O manualmente desde el PKGBUILD del repositorio:
cd packaging/aur
makepkg -si
```

`makepkg` instala las dependencias de compilación (`rustup`, `clang`, `llvm`,
`bpf-linker`), compila ambos binarios (el crate eBPF se compila a través del
`build.rs` con la toolchain nightly) e instala la unit systemd, el fichero
sysusers y el tmpfiles.

Arch ejecuta automáticamente `systemd-sysusers` y `systemd-tmpfiles` mediante
sus hooks de pacman al instalar el paquete, creando el usuario `netusaged` y
los directorios de estado.

Activa el servicio:

```sh
sudo systemctl enable --now netusaged
netusage-tui
```

## Notas

- Asegúrate de tener una toolchain nightly con `rust-src` disponible vía
  `rustup` (el crate eBPF la necesita). El `build.rs` invoca `cargo` en el
  directorio del crate eBPF, que tiene su propio `rust-toolchain.toml`.
- Arch usa kernel reciente con cgroup v2 y BTF: modo completo por defecto.
