//! Framing por longitud y (de)serialización con `postcard`.
//!
//! Cada mensaje va precedido de su longitud como `u32` big-endian. El cuerpo se
//! serializa con `postcard`, cuyo formato de wire es estable desde la 1.0. Se
//! valida un máximo de tamaño para no asignar memoria a peticiones hostiles.

use std::io::Read;

use serde::de::DeserializeOwned;
use serde::Serialize;

use crate::error::IpcError;

/// Tamaño máximo de un mensaje (16 MiB). Una respuesta por-app realista es de
/// pocos KiB; este tope solo evita asignaciones desmedidas ante un cliente o
/// servidor malicioso o corrupto.
const MAX_FRAME_LEN: usize = 16 * 1024 * 1024;

/// Serializa `msg` y le antepone su longitud (`u32` big-endian).
pub fn encode<T: Serialize>(msg: &T) -> Result<Vec<u8>, IpcError> {
    let body = postcard::to_allocvec(msg)?;
    let len = body.len() as u32;
    let mut out = Vec::with_capacity(4 + body.len());
    out.extend_from_slice(&len.to_be_bytes());
    out.extend_from_slice(&body);
    Ok(out)
}

/// Lee un mensaje con framing por longitud de `r` y lo deserializa.
///
/// Devuelve `IpcError::Protocol` si la longitud declarada excede el máximo, e
/// `IpcError::Io` (sin panic) si el stream se corta antes de tiempo.
pub fn decode_from<T: DeserializeOwned, R: Read>(r: &mut R) -> Result<T, IpcError> {
    let mut len_buf = [0u8; 4];
    r.read_exact(&mut len_buf)?;
    let len = u32::from_be_bytes(len_buf) as usize;
    if len > MAX_FRAME_LEN {
        return Err(IpcError::Protocol(format!(
            "frame de {len} bytes excede el máximo de {MAX_FRAME_LEN}"
        )));
    }
    let mut body = vec![0u8; len];
    r.read_exact(&mut body)?;
    Ok(postcard::from_bytes(&body)?)
}
