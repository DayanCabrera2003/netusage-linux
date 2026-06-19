//! Cliente síncrono de solo lectura del socket Unix.
//!
//! Lo usa la interfaz para consultar al demonio. No ofrece ninguna operación de
//! escritura: el `enum Request` no las define.

use std::io::Write;
use std::os::unix::net::UnixStream;
use std::path::Path;

use crate::codec::{decode_from, encode};
use crate::error::IpcError;
use crate::protocol::{Request, Response, PROTOCOL_VERSION};

/// Conexión a un demonio `netusaged` por su socket Unix.
#[derive(Debug)]
pub struct Client {
    stream: UnixStream,
}

impl Client {
    /// Conecta al socket en `path` y valida la versión del protocolo con un
    /// `Ping` inicial.
    pub fn connect<P: AsRef<Path>>(path: P) -> Result<Client, IpcError> {
        let stream = UnixStream::connect(path)?;
        let mut client = Client { stream };
        client.handshake()?;
        Ok(client)
    }

    /// Comprueba que el servidor habla la misma versión del protocolo.
    fn handshake(&mut self) -> Result<(), IpcError> {
        match self.request(&Request::Ping {
            version: PROTOCOL_VERSION,
        })? {
            Response::Pong { version } if version == PROTOCOL_VERSION => Ok(()),
            Response::Pong { version } => Err(IpcError::VersionMismatch {
                expected: PROTOCOL_VERSION,
                got: version,
            }),
            _ => Err(IpcError::Protocol(
                "respuesta inesperada al handshake".to_string(),
            )),
        }
    }

    /// Envía una petición y devuelve la respuesta.
    pub fn request(&mut self, req: &Request) -> Result<Response, IpcError> {
        let bytes = encode(req)?;
        self.stream.write_all(&bytes)?;
        self.stream.flush()?;
        decode_from(&mut self.stream)
    }
}
