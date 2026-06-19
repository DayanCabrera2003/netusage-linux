//! Utilidades del lado servidor: aceptar conexiones y despachar peticiones.
//!
//! La lógica concreta de consulta (traducir un `Request` a una query de
//! `netusage-store`) vive en el demonio, que implementa `RequestHandler`. Aquí
//! solo está el transporte: aceptar conexiones en un `UnixListener` y, por cada
//! una, leer peticiones y escribir respuestas con el codec.
//!
//! Se usa un hilo por conexión (modelo síncrono, coherente con el resto del
//! demonio) en vez de un runtime asíncrono.

use std::io::Write;
use std::os::unix::net::{UnixListener, UnixStream};
use std::sync::Arc;

use crate::codec::{decode_from, encode};
use crate::error::IpcError;
use crate::protocol::{Request, Response};

/// Traduce cada petición de solo lectura en una respuesta. Lo implementa el
/// demonio sobre `netusage-store`.
pub trait RequestHandler: Send + Sync + 'static {
    fn handle(&self, request: Request) -> Response;
}

/// Acepta conexiones de `listener` y las atiende en hilos. Bloquea hasta que el
/// listener se cierra o falla.
///
/// Los errores por conexión (cliente que envía basura o se va a medias) se
/// descartan: no deben tumbar el servidor ni el demonio.
pub fn serve<H: RequestHandler>(listener: UnixListener, handler: Arc<H>) {
    for conn in listener.incoming() {
        match conn {
            Ok(stream) => {
                let handler = Arc::clone(&handler);
                std::thread::spawn(move || {
                    let _ = handle_connection(stream, handler.as_ref());
                });
            }
            Err(_) => break,
        }
    }
}

/// Atiende una conexión: lee peticiones y responde hasta que el cliente cierra.
fn handle_connection<H: RequestHandler>(
    mut stream: UnixStream,
    handler: &H,
) -> Result<(), IpcError> {
    loop {
        let request = match decode_from::<Request, _>(&mut stream) {
            Ok(request) => request,
            // El cliente cerró la conexión: fin normal.
            Err(IpcError::Io(_)) => return Ok(()),
            Err(err) => return Err(err),
        };
        let response = handler.handle(request);
        stream.write_all(&encode(&response)?)?;
        stream.flush()?;
    }
}
