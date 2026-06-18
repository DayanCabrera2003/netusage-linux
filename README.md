# netusage-linux

Monitor de uso de datos por aplicación para Linux de escritorio, al estilo de
la pantalla de "Uso de datos" de Android. Mide el consumo de red total y por
aplicación usando eBPF (`cgroup_skb`) sobre los scopes de cgroup v2 de systemd.

Lenguaje principal: Rust. Distros objetivo: Debian, Arch, Fedora y derivadas.

## Stack eBPF

Stack eBPF: aya (todo en Rust, kernel + usuario). Ver la decisión 0001 en la
documentación del proyecto para el criterio y el plan de fallback a libbpf-rs.
