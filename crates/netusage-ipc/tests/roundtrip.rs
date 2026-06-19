//! Test de integración end-to-end: servidor y cliente sobre un socket Unix real.

use std::os::unix::net::UnixListener;
use std::sync::Arc;

use netusage_ipc::client::Client;
use netusage_ipc::error::IpcError;
use netusage_ipc::protocol::{Period, Request, Response, PROTOCOL_VERSION};
use netusage_ipc::server::{serve, RequestHandler};

/// Handler de prueba con totales fijos.
struct TestHandler;

impl RequestHandler for TestHandler {
    fn handle(&self, request: Request) -> Response {
        match request {
            Request::Ping { .. } => Response::Pong {
                version: PROTOCOL_VERSION,
            },
            Request::TotalsForPeriod { .. } => Response::Totals {
                rx_bytes: 42,
                tx_bytes: 7,
            },
            Request::PerAppForPeriod { .. } => Response::PerApp { entries: vec![] },
        }
    }
}

/// Handler que finge una versión distinta, para probar el mismatch.
struct MismatchHandler;

impl RequestHandler for MismatchHandler {
    fn handle(&self, _request: Request) -> Response {
        Response::Pong { version: 999 }
    }
}

fn temp_socket(name: &str) -> std::path::PathBuf {
    let dir = std::env::temp_dir().join(format!("nu-ipc-{}-{name}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    dir.join("netusaged.sock")
}

#[test]
fn ping_handshake_and_totals_roundtrip() {
    let sock = temp_socket("ok");
    let _ = std::fs::remove_file(&sock);
    let listener = UnixListener::bind(&sock).unwrap();
    std::thread::spawn(move || serve(listener, Arc::new(TestHandler)));

    // connect ya hace el handshake (Ping) y valida la versión.
    let mut client = Client::connect(&sock).unwrap();
    let resp = client
        .request(&Request::TotalsForPeriod {
            period: Period::Today,
        })
        .unwrap();
    assert_eq!(
        resp,
        Response::Totals {
            rx_bytes: 42,
            tx_bytes: 7
        }
    );

    std::fs::remove_file(&sock).ok();
}

#[test]
fn version_mismatch_is_detected_on_connect() {
    let sock = temp_socket("mismatch");
    let _ = std::fs::remove_file(&sock);
    let listener = UnixListener::bind(&sock).unwrap();
    std::thread::spawn(move || serve(listener, Arc::new(MismatchHandler)));

    let err = Client::connect(&sock).unwrap_err();
    assert!(matches!(err, IpcError::VersionMismatch { got: 999, .. }));

    std::fs::remove_file(&sock).ok();
}
