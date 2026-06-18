//! Mapas eBPF donde se acumulan los bytes de tráfico por socket.
//!
//! Responsabilidad única: declarar los mapas de contadores indexados por socket
//! cookie y exponer helpers para sumar bytes a la entrada de un socket concreto
//! en las direcciones de recibidos (RX) y enviados (TX).
//!
//! A diferencia de la atribución por cgroup, aquí la clave es el socket cookie
//! (`bpf_get_socket_cookie`), de modo que cada socket tiene su propia entrada y
//! el espacio de usuario puede atribuir el tráfico al ejecutable del proceso que
//! creó el socket.
//!
//! Los mapas son `LruHashMap`: cuando se llenan, el kernel desaloja los cookies
//! menos usados (sockets muertos) en vez de fallar. El espacio de usuario
//! acumula por deltas, así que un desalojo no pierde lo ya contado.

use aya_ebpf::{
    macros::map,
    maps::{LruHashMap, RingBuf},
};
use netusage_common::counters::{
    SockBirth, SocketCookie, SOCK_BIRTH_RING_BYTES, TRAFFIC_MAP_CAPACITY,
};

/// Bandera de `insert`: 0 equivale a `BPF_ANY` (crear o sobrescribir).
const BPF_ANY: u64 = 0;

/// Ringbuf por el que el kernel publica el nacimiento de cada socket
/// (`cookie`, `pid`) para que el espacio de usuario resuelva el ejecutable.
#[map(name = "SOCK_BIRTH")]
static SOCK_BIRTH: RingBuf = RingBuf::with_byte_size(SOCK_BIRTH_RING_BYTES, 0);

/// Publica en el ringbuf el nacimiento de un socket. Si el ringbuf está lleno,
/// el evento se descarta silenciosamente (su tráfico caería en "Sistema /
/// Otros" hasta que el socket vuelva a verse o se cierre).
pub fn emit_sock_birth(cookie: SocketCookie, pid: u32) {
    let birth = SockBirth {
        cookie,
        pid,
        _pad: 0,
    };
    let _ = SOCK_BIRTH.output(&birth, 0);
}

/// Bytes recibidos (ingress) acumulados por socket cookie.
#[map(name = "RX_BYTES")]
static RX_BYTES: LruHashMap<SocketCookie, u64> =
    LruHashMap::with_max_entries(TRAFFIC_MAP_CAPACITY, 0);

/// Bytes enviados (egress) acumulados por socket cookie.
#[map(name = "TX_BYTES")]
static TX_BYTES: LruHashMap<SocketCookie, u64> =
    LruHashMap::with_max_entries(TRAFFIC_MAP_CAPACITY, 0);

/// Acumula `bytes` en la entrada de recibidos (ingress) del socket `cookie`.
pub fn add_rx(cookie: SocketCookie, bytes: u64) {
    add(&RX_BYTES, cookie, bytes);
}

/// Acumula `bytes` en la entrada de enviados (egress) del socket `cookie`.
pub fn add_tx(cookie: SocketCookie, bytes: u64) {
    add(&TX_BYTES, cookie, bytes);
}

/// Suma `bytes` a la entrada `cookie` del mapa `counters`.
///
/// Si la entrada ya existe, se incrementa en sitio (no atómico, aceptable para
/// un contador de medición). Si no existe, se inserta con el valor inicial. Al
/// ser `LruHashMap`, `insert` no falla por falta de espacio: el kernel desaloja
/// la entrada menos usada.
fn add(counters: &LruHashMap<SocketCookie, u64>, cookie: SocketCookie, bytes: u64) {
    if let Some(ptr) = counters.get_ptr_mut(&cookie) {
        unsafe {
            *ptr += bytes;
        }
    } else {
        let _ = counters.insert(&cookie, &bytes, BPF_ANY);
    }
}
