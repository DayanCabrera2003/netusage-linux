# Atribución de tráfico por aplicación

Referencia técnica breve de cómo `netusaged` atribuye el tráfico de red a cada
aplicación. La documentación extendida del proyecto vive en la carpeta
`documentacion/` (un nivel por encima del repositorio).

## Por qué por ejecutable y no por cgroup

Un primer enfoque indexaba por el cgroup del socket (scope `app-*.scope` de
systemd). En el escritorio real falla: los navegadores tipo Chromium/Electron
enrutan toda su red por un proceso "network service" que GNOME coloca en el
cgroup del Shell, fuera del scope de la app. La atribución por cgroup no puede
ver ese tráfico. Por eso se atribuye por el **ejecutable** del proceso dueño del
socket, identificándolo a nivel de socket.

## Clave por socket cookie

El kernel cuenta bytes por paquete en dos mapas LRU `HashMap<u64, u64>`
(`RX_BYTES`, `TX_BYTES`) indexados por el **socket cookie** del paquete
(`bpf_get_socket_cookie`), un id único y estable por socket. Los programas
`cgroup_skb` ingress/egress se enganchan al cgroup v2 **raíz**, de modo que ven
todo el tráfico de la máquina.

## Flujo

1. **Nacimiento de socket** (`cgroup/sock_create`): corre en el contexto del
   proceso que crea el socket; publica `(cookie, pid)` en un ring buffer
   (`SOCK_BIRTH`).
2. **Resolución** (`resolver.rs`): un hilo drena el ring buffer y resuelve
   `pid -> /proc/<pid>/exe -> app`, manteniendo un mapa `cookie -> app`.
3. **Conteo** (`cgroup_skb`): cada paquete suma su longitud al mapa del cookie.
4. **Agregación** (`aggregator.rs`): cada intervalo lee los mapas, calcula el
   delta por cookie respecto a la lectura anterior y lo acumula al total de su
   app. Las cookies sin app conocida caen en "Sistema / Otros".

Se usan deltas (no absolutos) para que el total por app sobreviva al desalojo
LRU de cookies de sockets muertos. Los mapas LRU evitan el desbordamiento.

## Fallback "Sistema / Otros"

Cae aquí el tráfico cuyo socket no se pudo atribuir a un ejecutable: sockets
creados antes de arrancar el demonio, procesos efímeros que mueren antes de
resolverse, o sockets de kernel sin proceso dueño. Nunca se pierde del total.

## Limitaciones conocidas

- **VPN/proxy/DNS:** el tráfico se atribuye al proceso que realmente abre el
  socket. Un cliente VPN cargará el tráfico del túnel de otras apps;
  `systemd-resolved` cargará el DNS. Es inherente a la atribución por proceso.
- **Apps interpretadas:** la identidad es el ejecutable, así que varios scripts
  bajo el mismo intérprete (python/java) se agrupan. Mejora futura: usar el
  cmdline.
- **Sockets preexistentes:** los abiertos antes del arranque no tienen evento de
  nacimiento; su tráfico cae en fallback hasta que se cierran.
