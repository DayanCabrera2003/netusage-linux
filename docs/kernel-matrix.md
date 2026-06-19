# Matriz de soporte por versión de kernel

netusage usa eBPF de tipo `cgroup_skb` (contadores de bytes por socket),
`cgroup/sock_create` y un mapa `RingBuf` para la atribución por aplicación. Lo
que cada pieza requiere determina el modo en que se ejecuta.

## Requisitos por característica

| Característica                         | Kernel mínimo | Notas                                   |
|----------------------------------------|---------------|-----------------------------------------|
| `BPF_PROG_TYPE_CGROUP_SKB`             | 4.10          | Conteo de bytes por cgroup.             |
| cgroup v2 unificado                    | 4.5 / 5.x     | Debe estar montado en `/sys/fs/cgroup`. |
| BTF (`/sys/kernel/btf/vmlinux`)        | 5.2           | CO-RE; requiere `CONFIG_DEBUG_INFO_BTF`.|
| `BPF_MAP_TYPE_RINGBUF`                 | 5.8           | Detección de nacimiento de sockets.     |
| `CAP_BPF` / `CAP_PERFMON`              | 5.8           | Cargar eBPF sin ser root.               |

## Modos de ejecución resultantes

El demonio evalúa el entorno al arrancar (`degraded::decide`) y elige uno de
tres modos. Se puede inspeccionar con `netusaged --check --json` (campos
`cgroup_v2`, `btf`, `per_app`, `caps_ok`).

| Modo        | Condición                                              | Comportamiento                                              |
|-------------|--------------------------------------------------------|------------------------------------------------------------|
| **Full**    | cgroup v2 + BTF + kernel >= 5.8                        | Atribución completa por aplicación. Es el caso normal.     |
| **NoPerApp**| cgroup v2 + BTF, kernel 4.10–5.7                       | Solo consumo total del sistema (sin `RingBuf`). Requiere root. |
| **Disabled**| falta cgroup v2 unificado o BTF                        | No arranca; mensaje claro indicando el requisito ausente.  |

La TUI muestra una barra superior de aviso cuando el sistema no está en modo
Full.

## Disposición de cgroups

`netusaged` detecta la disposición leyendo `/proc/mounts`
(`cgroup::detect_layout`):

| Disposición   | Soporte                                                          |
|---------------|-----------------------------------------------------------------|
| `Unified`     | Soportado (cgroup v2 en `/sys/fs/cgroup`).                       |
| `Hybrid`      | No recomendado; arranca con `systemd.unified_cgroup_hierarchy=1`.|
| `LegacyV1`    | No soportado para `cgroup_skb`.                                  |
| `Unknown`     | No se detectó jerarquía reconocible.                            |

## Distribuciones de referencia

| Distribución        | Kernel típico | cgroup v2 | BTF | Modo por defecto |
|---------------------|---------------|-----------|-----|------------------|
| Ubuntu 22.04 / 24.04| 5.15 / 6.8    | sí        | sí  | Full             |
| Debian 12           | 6.1           | sí        | sí  | Full             |
| Fedora 38+          | 6.x           | sí        | sí  | Full             |
| Arch (rolling)      | 6.x           | sí        | sí  | Full             |
| RHEL/CentOS 8       | 4.18          | híbrido*  | sí  | requiere cambio  |

\* RHEL 8 arranca en modo híbrido por defecto; hay que forzar cgroup v2
unificado con `systemd.unified_cgroup_hierarchy=1` en la línea de arranque.

## Verificación

```sh
netusaged --check          # informe legible con el veredicto
netusaged --check --json   # banderas para automatización
sudo netusaged --selftest-load   # prueba real de carga y enganche eBPF
```
