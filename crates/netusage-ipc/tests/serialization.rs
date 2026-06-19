//! Tests de ida y vuelta del protocolo IPC y de los casos límite del codec.

use std::io::Cursor;

use netusage_ipc::codec::{decode_from, encode};
use netusage_ipc::error::IpcError;
use netusage_ipc::protocol::{AppUsage, Period, Request, Response};

/// `encode` seguido de `decode_from` devuelve el valor original.
fn roundtrip_request(req: Request) {
    let bytes = encode(&req).unwrap();
    let back: Request = decode_from(&mut Cursor::new(bytes)).unwrap();
    assert_eq!(req, back);
}

fn roundtrip_response(resp: Response) {
    let bytes = encode(&resp).unwrap();
    let back: Response = decode_from(&mut Cursor::new(bytes)).unwrap();
    assert_eq!(resp, back);
}

#[test]
fn requests_roundtrip() {
    roundtrip_request(Request::Ping { version: 1 });
    roundtrip_request(Request::TotalsForPeriod {
        period: Period::Today,
    });
    roundtrip_request(Request::PerAppForPeriod {
        period: Period::LastMonth,
    });
}

#[test]
fn responses_roundtrip() {
    roundtrip_response(Response::Pong { version: 1 });
    roundtrip_response(Response::Totals {
        rx_bytes: 1234,
        tx_bytes: 56,
    });
    roundtrip_response(Response::PerApp {
        entries: vec![AppUsage {
            app_key: "/usr/lib/firefox/firefox".into(),
            display_name: "firefox".into(),
            rx_bytes: 100,
            tx_bytes: 10,
        }],
    });
    roundtrip_response(Response::Error {
        message: "algo falló".into(),
    });
}

#[test]
fn oversized_length_is_protocol_error() {
    // Cabecera de longitud enorme (0xFFFFFFFF) sin cuerpo.
    let mut framed = vec![0xFF, 0xFF, 0xFF, 0xFF];
    framed.extend_from_slice(&[0u8; 8]);
    let res: Result<Request, IpcError> = decode_from(&mut Cursor::new(framed));
    assert!(matches!(res, Err(IpcError::Protocol(_))));
}

#[test]
fn truncated_buffer_is_io_error_not_panic() {
    // Declara 100 bytes de cuerpo pero solo aporta 2: read_exact corta.
    let mut framed = 100u32.to_be_bytes().to_vec();
    framed.extend_from_slice(&[1, 2]);
    let res: Result<Request, IpcError> = decode_from(&mut Cursor::new(framed));
    assert!(matches!(res, Err(IpcError::Io(_))));
}
