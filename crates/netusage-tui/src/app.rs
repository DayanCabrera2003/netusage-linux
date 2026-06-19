//! Orquestación: inicializa la terminal, ejecuta el bucle draw/handle y la
//! restaura al salir.
//!
//! El bucle es síncrono: dibuja, espera una tecla con un timeout igual al
//! intervalo de refresco (lo que marca el ritmo de polling en vivo) y aplica el
//! mensaje resultante. `ratatui::init` instala un hook de pánico que restaura la
//! terminal, así que el teardown está garantizado incluso ante un panic.

use std::time::{Duration, Instant};

use ratatui::crossterm::event::{self, Event, KeyEventKind};
use ratatui::DefaultTerminal;

use crate::cli::Cli;
use crate::data::DataSource;
use crate::error::Result;
use crate::event::map_key;
use crate::state::{AppState, ConnState};
use crate::ui;
use crate::update::{update, Message};

/// Arranca la TUI y entra en el bucle principal.
pub fn run(cli: Cli) -> Result<()> {
    let data = DataSource::new(cli.db);
    let mut state = AppState::new(cli.period.to_period());
    // Aviso de modo degradado: se calcula una vez al arrancar (el entorno no
    // cambia durante la sesion).
    state.degraded_note = crate::health::degraded_note();
    let refresh = Duration::from_secs(cli.refresh_secs.max(1));

    let mut terminal = ratatui::init();
    let result = run_loop(&mut terminal, &data, &mut state, refresh);
    ratatui::restore();
    result
}

/// Bucle principal: carga datos cuando hace falta, dibuja, atiende teclas y
/// refresca en vivo según el intervalo.
fn run_loop(
    terminal: &mut DefaultTerminal,
    data: &DataSource,
    state: &mut AppState,
    refresh: Duration,
) -> Result<()> {
    let mut last_refresh = Instant::now();
    loop {
        // Un cambio de periodo (o el arranque) deja la conexión en Loading.
        if matches!(state.connection, ConnState::Loading) {
            load(data, state);
        }
        terminal.draw(|frame| ui::draw(frame, state))?;

        let timeout = refresh.saturating_sub(last_refresh.elapsed());
        if event::poll(timeout)? {
            if let Event::Key(key) = event::read()? {
                if key.kind == KeyEventKind::Press {
                    if let Some(msg) = map_key(key, state.show_detail) {
                        update(state, msg);
                    }
                }
            }
        }

        // Refresco en vivo: re-consultar al cumplirse el intervalo.
        if last_refresh.elapsed() >= refresh {
            load(data, state);
            last_refresh = Instant::now();
        }

        if state.should_quit {
            return Ok(());
        }
    }
}

/// Consulta los datos del periodo activo y actualiza el estado.
fn load(data: &DataSource, state: &mut AppState) {
    match data.fetch(state.period) {
        Ok(summary) => update(state, Message::DataLoaded(summary)),
        Err(err) => update(state, Message::DataFailed(err.to_string())),
    }
}
