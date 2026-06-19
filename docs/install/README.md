# Instalación de netusage

netusage tiene dos componentes:

- **`netusaged`**: demonio privilegiado que carga eBPF, atribuye el tráfico por
  aplicación y persiste el consumo. Se ejecuta como servicio systemd con
  capabilities (no root pleno).
- **`netusage-tui`**: interfaz de terminal sin privilegios que lee la base de
  datos del demonio.

## Requisitos del sistema

- Kernel Linux **>= 5.8** (para `CAP_BPF`/`CAP_PERFMON` y el modo completo de
  atribución por aplicación). Entre 4.10 y 5.8 funciona en modo degradado
  (solo consumo total, ejecutando como root).
- **cgroup v2 unificado** montado en `/sys/fs/cgroup` (por defecto en las
  distribuciones modernas).
- **BTF** del kernel (`/sys/kernel/btf/vmlinux`, `CONFIG_DEBUG_INFO_BTF=y`).
- systemd (para la unit, el usuario de sistema y los directorios de estado).

Comprueba tu entorno antes de instalar:

```sh
netusaged --check          # informe legible
netusaged --check --json   # informe para scripts
```

Ver [kernel-matrix](../kernel-matrix.md) para el detalle de qué soporta cada
versión de kernel.

## Guías por distribución

- [Debian / Ubuntu](debian-ubuntu.md) — paquete `.deb`
- [Fedora / openSUSE](fedora.md) — paquete `.rpm`
- [Arch / derivadas](arch.md) — `PKGBUILD` del AUR
- [Manual (binario estático musl)](manual-musl.md) — cualquier distribución

## Tras instalar

```sh
sudo systemctl enable --now netusaged
netusage-tui
```
