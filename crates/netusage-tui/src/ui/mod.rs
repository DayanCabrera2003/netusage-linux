//! Función raíz de dibujo: compone el layout y delega en los widgets.
//!
//! Todos los widgets son funciones de render puras sobre `&AppState`, testeables
//! con el `TestBackend` de ratatui sin terminal real.

mod app_list;
mod detail;
mod footer;
mod period_bar;
mod summary;

use ratatui::layout::{Alignment, Constraint, Layout, Rect};
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Text};
use ratatui::widgets::{Block, Borders, Paragraph, Wrap};
use ratatui::Frame;

use crate::state::{AppState, ConnState};

/// Dibuja la interfaz completa para el estado dado.
pub fn draw(frame: &mut Frame, state: &AppState) {
    let area = frame.area();

    // Estado de borde: demonio/base no disponibles.
    if let ConnState::Disconnected(reason) = &state.connection {
        draw_disconnected(frame, area, reason);
        return;
    }

    let chunks = Layout::vertical([
        Constraint::Length(3), // selector de periodo
        Constraint::Length(3), // resumen del periodo
        Constraint::Min(1),    // lista de apps
        Constraint::Length(1), // ayuda
    ])
    .split(area);

    period_bar::render(frame, chunks[0], state);
    summary::render(frame, chunks[1], state);
    app_list::render(frame, chunks[2], state);
    footer::render(frame, chunks[3]);

    // El detalle se superpone al resto cuando está abierto.
    if state.show_detail {
        detail::render(frame, area, state);
    }
}

/// Panel central cuando el demonio o la base no están disponibles.
fn draw_disconnected(frame: &mut Frame, area: Rect, reason: &str) {
    let text = Text::from(vec![
        Line::from("Demonio no disponible"),
        Line::from(""),
        Line::from(reason.to_string()),
        Line::from(""),
        Line::from("Pulsa 'r' para reintentar, 'q' para salir."),
    ]);
    let panel = Paragraph::new(text)
        .alignment(Alignment::Center)
        .wrap(Wrap { trim: true })
        .style(Style::default().add_modifier(Modifier::BOLD))
        .block(Block::default().borders(Borders::ALL).title(" netusage "));
    frame.render_widget(panel, area);
}

/// Renderiza `draw_fn` en un `TestBackend` y devuelve el buffer como líneas de
/// texto, para asertar contenido en los tests de los widgets.
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

#[cfg(test)]
mod tests {
    use super::{draw, render_to_lines};
    use crate::period::Period;
    use crate::state::{AppState, ConnState};

    #[test]
    fn disconnected_shows_panel_with_retry_hint() {
        let mut state = AppState::new(Period::Today);
        state.connection = ConnState::Disconnected("no se pudo abrir la base".into());
        let text = render_to_lines(50, 12, |f| draw(f, &state)).join("\n");
        assert!(text.contains("Demonio no disponible"), "{text}");
        assert!(text.contains("reintentar"), "{text}");
        assert!(text.contains("no se pudo abrir la base"), "{text}");
    }
}
