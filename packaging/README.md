# Empaquetado de netusaged

Artefactos de despliegue del servicio de sistema `netusaged`. No es
documentación del proyecto; describe cómo instalar el demonio.

## Instalación manual

Requisitos: kernel >= 5.8 con cgroup v2 unificado y BTF (ver `netusaged --check`).

```sh
# 1. Binario
sudo install -m 0755 target/release/netusaged /usr/bin/netusaged

# 2. Ficheros systemd
sudo install -m 0644 packaging/systemd/netusaged.service /etc/systemd/system/netusaged.service
sudo install -m 0644 packaging/systemd/sysusers.d/netusaged.conf /usr/lib/sysusers.d/netusaged.conf
sudo install -m 0644 packaging/systemd/tmpfiles.d/netusaged.conf /usr/lib/tmpfiles.d/netusaged.conf

# 3. Crear el usuario de servicio y los directorios (en este orden)
sudo systemd-sysusers
sudo systemd-tmpfiles --create

# 4. Arrancar
sudo systemctl daemon-reload
sudo systemctl enable --now netusaged
```

## Modelo de privilegios

El servicio corre como el usuario de sistema `netusaged`, **sin root pleno**.
En kernels >= 5.8 systemd le concede solo `CAP_BPF`, `CAP_PERFMON` y
`CAP_NET_ADMIN` (`AmbientCapabilities`), suficientes para cargar y enganchar los
programas eBPF. En kernels < 5.8 esas capabilities no existen y el demonio
requiere root (lo detecta y lo declara en el log).

Verificación:

```sh
pid=$(systemctl show -p MainPID --value netusaged)
ps -o user= -p "$pid"        # netusaged (no root)
getpcaps "$pid"              # cap_bpf, cap_perfmon, cap_net_admin
```

## Cómo accede la interfaz a los datos

- **Camino primario (SQLite solo lectura):** la base vive en
  `/var/lib/netusage/netusage.db`, propiedad de `netusaged:netusaged` con modo
  `0644`. Cualquier usuario puede leerla; nadie salvo el demonio puede
  escribirla. La UI la abre con `SQLITE_OPEN_READ_ONLY`.
- **Camino opcional (socket IPC):** `/run/netusage/netusaged.sock` (modo `0660`,
  grupo `netusaged`), protocolo `netusage-ipc` (postcard). Solo expone
  operaciones de lectura. Útil para refresco en vivo sin sondear el fichero.

## Hardening

La unit aplica un sandbox amplio (`ProtectSystem=strict`, `ProtectHome`,
`PrivateTmp`, etc.). Notas importantes:

- `ProtectControlGroups=false`: el demonio engancha eBPF al cgroup v2 raíz.
- `/proc` no se restringe: la atribución por app lee `/proc/<pid>/exe`.
- `MemoryDenyWriteExecute=false`: el JIT de eBPF necesita páginas ejecutables.
- `SystemCallFilter` permite `bpf` y `perf_event_open` explícitamente (no están
  en `@system-service`).
