//! Backfill de sockets preexistentes al arrancar.
//!
//! Responsabilidad única: para los sockets que ya estaban abiertos antes de
//! arrancar el demonio (no tienen evento `sock_create`), correlacionar su
//! `cookie` con la app dueña, de modo que su tráfico no caiga en "Sistema /
//! Otros".
//!
//! Procedimiento:
//! 1. `sock_diag` (netlink) enumera todos los sockets TCP/UDP (v4 y v6) y
//!    devuelve, por cada uno, su `cookie` y su número de inode.
//! 2. Se cruza inode -> PID escaneando `/proc/<pid>/fd` (cada socket aparece
//!    como un enlace `socket:[inode]`).
//! 3. Se resuelve PID -> ejecutable -> app y se precarga `cookie -> app`.
//!
//! Es la pieza que hace útil la herramienta en un equipo real: el socket del
//! túnel de un VPN o las conexiones persistentes de un navegador suelen
//! preceder al demonio, y sin este backfill concentrarían casi todo el tráfico
//! en el cubo de fallback.

use std::collections::HashMap;

use anyhow::{Context, Result};
use netlink_packet_core::{
    NetlinkHeader, NetlinkMessage, NetlinkPayload, NLM_F_DUMP, NLM_F_REQUEST,
};
use netlink_packet_sock_diag::{
    constants::{AF_INET, AF_INET6, IPPROTO_TCP, IPPROTO_UDP},
    inet::{ExtensionFlags, InetRequest, SocketId, StateFlags},
    SockDiagMessage,
};
use netlink_sys::{protocols::NETLINK_SOCK_DIAG, Socket, SocketAddr};
use netusage_common::counters::SocketCookie;

use crate::identity;
use crate::resolver::CookieMap;

/// Valor de cookie que indica "sin cookie" (`INET_DIAG_NOCOOKIE`): todos los
/// bytes a 0xFF. Esos sockets se omiten.
const NO_COOKIE: u64 = u64::MAX;

/// Precarga `cookie -> app` para los sockets ya abiertos. Devuelve cuántos se
/// correlacionaron.
///
/// No sobrescribe entradas ya presentes en `cookie_map` (un evento de
/// nacimiento reciente es más fiable que el backfill).
pub fn backfill(cookie_map: &CookieMap) -> Result<usize> {
    let sockets = dump_all_sockets().context("enumerando sockets con sock_diag")?;
    if sockets.is_empty() {
        return Ok(0);
    }

    let inode_to_pid = build_inode_pid_map();
    let mut map = cookie_map.lock().unwrap();
    let mut resolved = 0;
    // Desglose para diagnostico (nivel debug): por que un socket no se resolvio.
    // 'sin_pid' alto con 'inode->pid' casi vacio delata falta de
    // CAP_DAC_READ_SEARCH para leer /proc/<pid>/fd de otros usuarios.
    let (mut no_pid, mut resolve_fail) = (0, 0);

    for (inode, cookie) in sockets {
        if cookie == NO_COOKIE || map.contains_key(&cookie) {
            continue;
        }
        let Some(&pid) = inode_to_pid.get(&inode) else {
            no_pid += 1;
            continue;
        };
        if let Some(identity) = identity::resolve_pid(pid) {
            map.insert(cookie, identity);
            resolved += 1;
        } else {
            resolve_fail += 1;
        }
    }
    tracing::debug!(
        "backfill: inode->pid={} resueltos={resolved} sin_pid={no_pid} resolve_fallo={resolve_fail}",
        inode_to_pid.len()
    );
    Ok(resolved)
}

/// Enumera todos los sockets TCP y UDP (IPv4 e IPv6) y devuelve `(inode,
/// cookie)` de cada uno.
fn dump_all_sockets() -> Result<Vec<(u64, SocketCookie)>> {
    let mut out = Vec::new();
    for (family, protocol) in [
        (AF_INET, IPPROTO_TCP),
        (AF_INET6, IPPROTO_TCP),
        (AF_INET, IPPROTO_UDP),
        (AF_INET6, IPPROTO_UDP),
    ] {
        dump_family(family, protocol, &mut out)?;
    }
    Ok(out)
}

/// Realiza un dump de `sock_diag` para una familia y protocolo, añadiendo los
/// `(inode, cookie)` a `out`.
fn dump_family(family: u8, protocol: u8, out: &mut Vec<(u64, SocketCookie)>) -> Result<()> {
    let mut socket = Socket::new(NETLINK_SOCK_DIAG).context("abriendo socket netlink")?;
    socket.bind_auto().context("bind del socket netlink")?;
    socket
        .connect(&SocketAddr::new(0, 0))
        .context("connect del socket netlink")?;

    let socket_id = if family == AF_INET6 {
        SocketId::new_v6()
    } else {
        SocketId::new_v4()
    };
    let mut header = NetlinkHeader::default();
    header.flags = NLM_F_REQUEST | NLM_F_DUMP;
    let mut packet = NetlinkMessage::new(
        header,
        SockDiagMessage::InetRequest(InetRequest {
            family,
            protocol,
            extensions: ExtensionFlags::empty(),
            states: StateFlags::all(),
            socket_id,
        })
        .into(),
    );
    packet.finalize();

    let mut buf = vec![0u8; packet.buffer_len()];
    packet.serialize(&mut buf[..]);
    socket
        .send(&buf[..], 0)
        .context("enviando petición sock_diag")?;

    let mut recv_buf = vec![0u8; 16 * 1024];
    'recv: while let Ok(size) = socket.recv(&mut &mut recv_buf[..], 0) {
        let mut offset = 0;
        while offset < size {
            let bytes = &recv_buf[offset..size];
            let rx = match NetlinkMessage::<SockDiagMessage>::deserialize(bytes) {
                Ok(rx) => rx,
                Err(_) => break 'recv,
            };
            let len = rx.header.length as usize;
            match rx.payload {
                NetlinkPayload::InnerMessage(SockDiagMessage::InetResponse(response)) => {
                    let inode = response.header.inode as u64;
                    let cookie = u64::from_ne_bytes(response.header.socket_id.cookie);
                    out.push((inode, cookie));
                }
                NetlinkPayload::Done(_) => break 'recv,
                _ => {}
            }
            if len == 0 {
                break 'recv;
            }
            offset += len;
        }
    }
    Ok(())
}

/// Construye el mapa inode-de-socket -> PID recorriendo `/proc/<pid>/fd`.
///
/// Cada descriptor que apunta a un socket aparece como un enlace simbólico
/// `socket:[<inode>]`. Si un mismo inode lo comparten varios PID (sockets
/// heredados por fork), gana el primero visto: a efectos de ejecutable suelen
/// compartir binario.
fn build_inode_pid_map() -> HashMap<u64, u32> {
    let mut map = HashMap::new();
    let Ok(proc_dir) = std::fs::read_dir("/proc") else {
        return map;
    };
    for entry in proc_dir.flatten() {
        let Some(pid) = entry
            .file_name()
            .to_str()
            .and_then(|name| name.parse::<u32>().ok())
        else {
            continue;
        };
        let fd_dir = entry.path().join("fd");
        let Ok(fds) = std::fs::read_dir(&fd_dir) else {
            continue;
        };
        for fd in fds.flatten() {
            if let Ok(target) = std::fs::read_link(fd.path()) {
                if let Some(inode) = parse_socket_inode(&target.to_string_lossy()) {
                    map.entry(inode).or_insert(pid);
                }
            }
        }
    }
    map
}

/// Extrae el inode de un destino de enlace `socket:[<inode>]`; `None` si no lo
/// es.
fn parse_socket_inode(target: &str) -> Option<u64> {
    let inner = target.strip_prefix("socket:[")?.strip_suffix(']')?;
    inner.parse::<u64>().ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_socket_link_targets() {
        assert_eq!(parse_socket_inode("socket:[12345]"), Some(12345));
        assert_eq!(parse_socket_inode("anon_inode:[eventfd]"), None);
        assert_eq!(parse_socket_inode("/dev/null"), None);
        assert_eq!(parse_socket_inode("socket:[]"), None);
    }
}
