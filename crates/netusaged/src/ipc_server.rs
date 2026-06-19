//! Servidor IPC del demonio sobre socket Unix.
//!
//! Responsabilidad única: traducir cada petición de solo lectura del protocolo
//! `netusage-ipc` en una query de `netusage-store` y construir la respuesta.
//! Nunca escribe (abre la base en modo solo lectura por conexión).
//!
//! El socket es opcional: si falla al crearse, el demonio sigue funcionando (la
//! UI siempre tiene el camino SQLite de solo lectura).

use std::os::unix::fs::PermissionsExt;
use std::os::unix::net::UnixListener;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use anyhow::{Context, Result};
use chrono::Utc;
use netusage_ipc::protocol::{AppUsage, Period as IpcPeriod, Request, Response, PROTOCOL_VERSION};
use netusage_ipc::server::{serve, RequestHandler};
use netusage_store::{Period, Store};

/// Handler que responde abriendo la base en solo lectura por cada consulta.
struct StoreHandler {
    db_path: PathBuf,
}

impl StoreHandler {
    /// Abre la base en solo lectura y ejecuta `query`, devolviendo el error como
    /// texto para la respuesta IPC.
    fn read<T>(
        &self,
        query: impl FnOnce(&Store) -> netusage_store::Result<T>,
    ) -> Result<T, String> {
        let store = Store::open_readonly(&self.db_path).map_err(|e| e.to_string())?;
        query(&store).map_err(|e| e.to_string())
    }
}

/// Traduce el periodo del protocolo al del store.
fn map_period(period: IpcPeriod) -> Period {
    match period {
        IpcPeriod::Today => Period::Today,
        IpcPeriod::ThisWeek => Period::ThisWeek,
        IpcPeriod::ThisMonth => Period::ThisMonth,
        IpcPeriod::LastMonth => Period::LastMonth,
    }
}

impl RequestHandler for StoreHandler {
    fn handle(&self, request: Request) -> Response {
        match request {
            Request::Ping { .. } => Response::Pong {
                version: PROTOCOL_VERSION,
            },
            Request::TotalsForPeriod { period } => {
                match self.read(|store| store.usage_total(map_period(period), Utc::now())) {
                    Ok(total) => Response::Totals {
                        rx_bytes: total.rx_bytes,
                        tx_bytes: total.tx_bytes,
                    },
                    Err(message) => Response::Error { message },
                }
            }
            Request::PerAppForPeriod { period } => {
                match self.read(|store| store.usage_by_app(map_period(period), Utc::now())) {
                    Ok(apps) => Response::PerApp {
                        entries: apps
                            .into_iter()
                            .map(|app| AppUsage {
                                app_key: app.app_key,
                                display_name: app.display_name,
                                rx_bytes: app.rx_bytes,
                                tx_bytes: app.tx_bytes,
                            })
                            .collect(),
                    },
                    Err(message) => Response::Error { message },
                }
            }
        }
    }
}

/// Arranca el servidor IPC en un hilo dedicado, sirviendo consultas de solo
/// lectura sobre la base en `db_path` a través del socket `socket_path`.
///
/// Elimina un socket huérfano previo y fija permisos `0660`.
pub fn spawn(db_path: PathBuf, socket_path: &Path) -> Result<()> {
    // Un socket huérfano de un arranque anterior impediría el bind.
    if socket_path.exists() {
        let _ = std::fs::remove_file(socket_path);
    }
    let listener = UnixListener::bind(socket_path)
        .with_context(|| format!("creando el socket IPC {}", socket_path.display()))?;
    std::fs::set_permissions(socket_path, std::fs::Permissions::from_mode(0o660))
        .with_context(|| format!("fijando permisos del socket {}", socket_path.display()))?;

    let handler = Arc::new(StoreHandler { db_path });
    std::thread::spawn(move || serve(listener, handler));
    Ok(())
}
