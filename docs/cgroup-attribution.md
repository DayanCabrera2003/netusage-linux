# Atribución de tráfico por aplicación

Referencia técnica breve de cómo `netusaged` atribuye el tráfico de red a cada
aplicación (Fase 2). La documentación extendida del proyecto vive en la carpeta
`documentacion/` (un nivel por encima del repositorio).

## Clave por inode de cgroup

El kernel cuenta bytes por paquete en dos mapas eBPF `HashMap<u64, u64>`
(`RX_BYTES`, `TX_BYTES`) indexados por el **cgroup id** del paquete, obtenido con
el helper `bpf_skb_cgroup_id`. Ese cgroup id es exactamente el número de inode
del directorio del cgroup en `/sys/fs/cgroup`. El espacio de usuario resuelve la
ruta de cada cgroup a su inode con `statx` y cruza ambos lados por ese entero.

## Flujo

1. **Descubrimiento** (`cgroup/discovery.rs`): escanea
   `…/user@<UID>.service/app.slice` y enumera los scopes/servicios de app.
2. **Identidad** (`cgroup/identity.rs`): parsea el nombre del scope de systemd
   (`app-<launcher>-<app>-<RANDOM>.scope`) a un nombre legible, desescapando las
   secuencias `\xNN`.
3. **Enganche** (`attach.rs`): engancha los programas `cgroup_skb` ingress y
   egress al descriptor del cgroup en modo `AllowMultiple`, guardando los links.
4. **Vigilancia** (`cgroup/watcher.rs`): un hilo con `inotify` detecta el
   nacimiento y la muerte de cgroups y emite eventos; la identidad y el inode se
   resuelven en el nacimiento, antes de que el cgroup pueda desaparecer.
5. **Registro** (`cgroup/registry.rs`): cachea `inode -> {ruta, identidad,
   links}` mientras el cgroup vive.
6. **Lectura y agregación** (`monitor.rs`, `fallback.rs`, `supervisor.rs`): cada
   intervalo lee los mapas, agrega por aplicación y presenta la lista.

## Fallback "Sistema / Otros"

El tráfico de cgroups que no son apps reconocidas (`session.scope`, servicios de
sistema y de D-Bus, `init.scope`, o cgroups perdidos por una carrera) se agrega
en un único cubo `Sistema / Otros`. Nunca se pierde de la cuenta total.

## Race conocido

Una app de vida muy corta puede nacer y morir entre que se descubre y se
engancha; su tráfico cae entonces en el cubo de fallback. La vigilancia por
`inotify` lo mitiga resolviendo identidad e inode en el nacimiento. La evolución
natural, si hiciera falta exactitud total, es capturar el evento `cgroup_mkdir`
con un raw tracepoint eBPF (estilo pktstat-bpf), descrita en `documentacion/`.
