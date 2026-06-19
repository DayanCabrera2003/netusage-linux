//! Función raíz de dibujo: compone el layout y delega en los widgets.
//!
//! Todos los widgets son funciones de render puras sobre `&AppState`, testeables
//! con el `TestBackend` de ratatui sin terminal real.

mod summary;

use ratatui::layout::{Constraint, Layout};
use ratatui::Frame;

use crate::state::AppState;

/// Dibuja la interfaz completa para el estado dado.
pub fn draw(frame: &mut Frame, state: &AppState) {
    let area = frame.area();
    let chunks = Layout::vertical([
        Constraint::Length(3), // resumen del periodo
        Constraint::Min(1),    // (resto: widgets de commits siguientes)
    ])
    .split(area);

    summary::render(frame, chunks[0], state);
}

#[cfg(test)]
pub(crate) fn render_to_lines<F>(width: u16, height: u16, draw_fn: F) -> Vec<String>
where
    F: FnOnce(&mut Frame),
{
    use ratatui::backend::TestBackend;
    use ratatui::Terminal;

    let mut terminal = Terminal::new(TestBackend::new(width, height)).unwrap();
    terminal.draw(|frame| draw_fn(frame)).unwrap();
    let buffer = terminal.backend().buffer().clone();

    let mut lines = Vec::new();
    for y in 0..buffer.area.height {
        let mut line = String::new();
        for x in 0..buffer.area.width {
            line.push_str(buffer[(x, y)].symbol());
        }
        lines.push(line.trim_end().to_string());
    }
    lines
}
