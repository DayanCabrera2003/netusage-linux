//! Correlación `socket cookie -> aplicación` a partir del ringbuf de nacimientos
//! de socket.
//!
//! Responsabilidad única: en un hilo dedicado, drenar el ringbuf `SOCK_BIRTH`
//! (cada evento es `(cookie, pid)`), resolver el ejecutable del proceso dueño y
//! mantener un mapa compartido `cookie -> AppIdentity` que el agregador consulta.
//!
//! Resolver al instante (en cuanto nace el socket) mitiga la carrera con
//! procesos efímeros: si esperáramos al muestreo, el proceso podría haber muerto
//! y `/proc/<pid>/exe` ya no existiría.
//!
//! Limitación documentada: los sockets creados antes de arrancar el demonio no
//! generan evento de nacimiento; su tráfico cae en "Sistema / Otros" hasta que
//! se cierran. Mejora futura: backfill inicial cruzando `/proc` con la tabla de
//! sockets.

use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::thread::{self, JoinHandle};
use std::time::Duration;

use aya::maps::{MapData, RingBuf};
use netusage_common::counters::SocketCookie;

use crate::identity::{self, AppIdentity};

/// Mapa compartido `cookie -> identidad de app`, escrito por el resolver y leído
/// (y podado) por el agregador/supervisor.
pub type CookieMap = Arc<Mutex<HashMap<SocketCookie, AppIdentity>>>;

/// Tamaño en bytes de un registro `SockBirth` (cookie u64 + pid u32 + pad u32).
const SOCK_BIRTH_LEN: usize = 16;

/// Espera cuando el ringbuf está vacío, para no ocupar una CPU en vacío.
const IDLE_POLL: Duration = Duration::from_millis(20);

/// Arranca el hilo que drena el ringbuf y alimenta `cookie_map`.
///
/// `ring` es el mapa `SOCK_BIRTH` ya extraído del objeto eBPF (con
/// `bpf.take_map`). El hilo corre hasta que el proceso termina.
pub fn spawn_resolver(mut ring: RingBuf<MapData>, cookie_map: CookieMap) -> JoinHandle<()> {
    thread::spawn(move || loop {
        let mut drained_any = false;
        while let Some(item) = ring.next() {
            drained_any = true;
            if let Some((cookie, pid)) = parse_birth(&item) {
                if let Some(identity) = identity::resolve_pid(pid) {
                    cookie_map.lock().unwrap().insert(cookie, identity);
                }
            }
        }
        if !drained_any {
            thread::sleep(IDLE_POLL);
        }
    })
}

/// Interpreta los bytes de un evento del ringbuf como `(cookie, pid)`.
///
/// Se leen en orden de bytes nativo: el kernel y el espacio de usuario corren en
/// la misma máquina. Devuelve `None` si el registro viene truncado.
fn parse_birth(bytes: &[u8]) -> Option<(SocketCookie, u32)> {
    if bytes.len() < SOCK_BIRTH_LEN {
        return None;
    }
    let cookie = u64::from_ne_bytes(bytes[0..8].try_into().ok()?);
    let pid = u32::from_ne_bytes(bytes[8..12].try_into().ok()?);
    Some((cookie, pid))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_a_well_formed_birth_record() {
        let mut bytes = [0u8; SOCK_BIRTH_LEN];
        bytes[0..8].copy_from_slice(&1234u64.to_ne_bytes());
        bytes[8..12].copy_from_slice(&42u32.to_ne_bytes());
        assert_eq!(parse_birth(&bytes), Some((1234, 42)));
    }

    #[test]
    fn rejects_truncated_record() {
        assert_eq!(parse_birth(&[0u8; 4]), None);
    }
}
