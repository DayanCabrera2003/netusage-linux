//! Contrato de datos del contador de tráfico compartido entre el lado kernel
//! (eBPF) y el espacio de usuario.
//!
//! Responsabilidad única: definir las claves y el tamaño del mapa de
//! contadores, para que ambos lados usen exactamente los mismos valores y no
//! haya posibilidad de desincronización.
//!
//! Diseño (Fase 1): se usa un mapa eBPF `Array<u64>` con dos entradas
//! escalares en vez de una struct. Es lo más simple para contar el total de la
//! máquina y no requiere `aya::Pod`. La atribución por aplicación (Fase 2)
//! cambiará a un mapa indexado por cgroup.

/// Número de entradas del mapa de contadores de tráfico.
pub const TRAFFIC_MAP_ENTRIES: u32 = 2;

/// Clave de la entrada que acumula los bytes recibidos (ingress).
pub const COUNTER_RX: u32 = 0;

/// Clave de la entrada que acumula los bytes enviados (egress).
pub const COUNTER_TX: u32 = 1;
