//! Contrato de datos de los contadores de tráfico compartido entre el lado
//! kernel (eBPF) y el espacio de usuario.
//!
//! Responsabilidad única: definir el tipo de clave y la capacidad de los mapas
//! de contadores, para que ambos lados usen exactamente los mismos valores y no
//! haya posibilidad de desincronización.
//!
//! Diseño (Fase 2): se pasa de "total de la máquina" (Fase 1) a "por
//! aplicación". El lado kernel mantiene dos mapas `HashMap<u64, u64>`
//! indexados por el cgroup id del paquete: uno acumula los bytes recibidos
//! (RX) y otro los enviados (TX). El cgroup id lo da el helper
//! `bpf_skb_cgroup_id` y coincide con el número de inode del directorio del
//! cgroup en `/sys/fs/cgroup`, lo que permite cruzar el mapa con el árbol de
//! cgroups del espacio de usuario.
//!
//! Se eligen dos mapas escalares `u64` en lugar de un único mapa con un valor
//! struct para no necesitar `aya::Pod` ni arrastrar la dependencia `aya` al
//! crate `netusage-common`, que debe permanecer `no_std` para el lado eBPF.
//! Es la misma decisión de la Fase 1 (ver desviaciones), llevada al modelo por
//! cgroup.

/// Clave de los mapas de contadores: el inode del directorio del cgroup.
///
/// Este entero es a la vez el cgroup id devuelto por `bpf_skb_cgroup_id` en el
/// kernel y el `st_ino` del directorio del cgroup en `/sys/fs/cgroup`. Esa
/// equivalencia es la base de toda la atribución por aplicación: el kernel
/// indexa por cgroup id y el espacio de usuario resuelve ese mismo número a una
/// ruta de cgroup y, de ahí, a la identidad de la app.
pub type CgroupInode = u64;

/// Capacidad máxima de cada mapa de contadores por cgroup.
///
/// Una máquina de escritorio típica tiene del orden de decenas de cgroups de
/// aplicación vivos a la vez; 4096 entradas dejan margen amplio para picos y
/// para cgroups que aparecen y desaparecen entre lecturas. Si en pruebas se
/// observara agotamiento (inserciones fallidas en el kernel), subir este valor
/// y documentarlo.
pub const TRAFFIC_MAP_CAPACITY: u32 = 4096;

/// Nombre del mapa eBPF que acumula los bytes recibidos (ingress) por cgroup.
///
/// El espacio de usuario abre el mapa por este nombre exacto.
pub const RX_MAP_NAME: &str = "RX_BYTES";

/// Nombre del mapa eBPF que acumula los bytes enviados (egress) por cgroup.
pub const TX_MAP_NAME: &str = "TX_BYTES";
