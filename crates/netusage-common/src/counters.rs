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

/// Clave de los mapas de contadores: el cgroup id (Fase 2 por cgroup, obsoleto).
///
/// Se conserva mientras quede código que lo referencie; se elimina al retirar el
/// enfoque por cgroup. La atribución por ejecutable usa `SocketCookie`.
pub type CgroupInode = u64;

/// Clave de los mapas de contadores: el socket cookie.
///
/// Es el valor que devuelve el helper `bpf_get_socket_cookie`, único y estable
/// por socket mientras el socket vive. Es la base de la atribución por
/// aplicación: el kernel indexa los bytes por cookie y el espacio de usuario
/// correlaciona cada cookie con el ejecutable del proceso que creó el socket.
pub type SocketCookie = u64;

/// Registro que el kernel publica en el ringbuf al crearse un socket.
///
/// Lleva el `cookie` del socket recién creado y el `pid` del proceso que lo
/// creó (en cuyo contexto corre el programa `cgroup/sock_create`). El espacio de
/// usuario resuelve `pid -> /proc/<pid>/exe -> app` y cachea `cookie -> app`.
///
/// `#[repr(C)]` con padding explícito: se escribe/lee como bytes crudos del
/// ringbuf, así que el layout debe ser estable y sin huecos sin inicializar.
#[repr(C)]
#[derive(Clone, Copy)]
pub struct SockBirth {
    pub cookie: u64,
    pub pid: u32,
    pub _pad: u32,
}

/// Capacidad máxima de cada mapa de contadores por socket.
///
/// Una máquina de escritorio puede tener miles de sockets vivos (un navegador
/// abre cientos). 16384 entradas dejan margen amplio; al ser mapas LRU, si se
/// llenaran el kernel desaloja los cookies menos usados (sockets muertos) en vez
/// de fallar.
pub const TRAFFIC_MAP_CAPACITY: u32 = 16384;

/// Tamaño en bytes del ringbuf de nacimientos de socket (potencia de 2).
///
/// 256 KiB absorben ráfagas de creación de sockets (p. ej. al abrir un
/// navegador) sin que el espacio de usuario, que drena en un hilo dedicado,
/// pierda eventos.
pub const SOCK_BIRTH_RING_BYTES: u32 = 256 * 1024;

/// Nombre del mapa eBPF que acumula los bytes recibidos (ingress) por socket.
///
/// El espacio de usuario abre el mapa por este nombre exacto.
pub const RX_MAP_NAME: &str = "RX_BYTES";

/// Nombre del mapa eBPF que acumula los bytes enviados (egress) por socket.
pub const TX_MAP_NAME: &str = "TX_BYTES";

/// Nombre del ringbuf de nacimientos de socket.
pub const SOCK_BIRTH_MAP_NAME: &str = "SOCK_BIRTH";
