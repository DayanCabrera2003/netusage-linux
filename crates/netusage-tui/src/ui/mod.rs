//! Función raíz de dibujo: compone el layout y delega en los widgets.
//!
//! Todos los widgets son funciones de render puras sobre `&AppState`, testeables
//! con el `TestBackend` de ratatui sin terminal real.

mod app_list;
mod detail;
mod footer;
mod period_bar;
mod summary;
mod theme;

use ratatui::layout::{Alignment, Constraint, Layout, Rect};
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span, Text};
use ratatui::widgets::{Paragraph, Wrap};
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

    // Reservar una fila superior por cada aviso presente: modo degradado
    // (advertencia) y nueva release (informativo). Cada uno ocupa una linea solo
    // si lo hay, de modo que sin avisos la interfaz se muestra limpia.
    let degraded_height = if state.degraded_note.is_some() { 1 } else { 0 };
    let update_height = if state.update_note.is_some() { 1 } else { 0 };
    let chunks = Layout::vertical([
        Constraint::Length(degraded_height), // aviso de modo degradado (opcional)
        Constraint::Length(update_height),   // aviso de nueva release (opcional)
        Constraint::Length(3),               // selector de periodo
        Constraint::Length(3),               // resumen del periodo
        Constraint::Min(1),                  // lista de apps
        Constraint::Length(1),               // ayuda
    ])
    .split(area);

    if let Some(note) = &state.degraded_note {
        draw_degraded_banner(frame, chunks[0], note);
    }
    if let Some(note) = &state.update_note {
        draw_update_banner(frame, chunks[1], note);
    }
    period_bar::render(frame, chunks[2], state);
    summary::render(frame, chunks[3], state);
    app_list::render(frame, chunks[4], state);
    footer::render(frame, chunks[5]);

    // El detalle se superpone al resto cuando está abierto.
    if state.show_detail {
        detail::render(frame, area, state);
    }
}

/// Barra superior de una linea con el aviso de modo degradado, en colores de
/// advertencia para que destaque sin robar espacio al contenido.
fn draw_degraded_banner(frame: &mut Frame, area: Rect, note: &str) {
    let banner = Paragraph::new(Line::from(Span::styled(
        format!(" {note} "),
        Style::default()
            .fg(ratatui::style::Color::Black)
            .bg(theme::WARN)
            .add_modifier(Modifier::BOLD),
    )))
    .alignment(Alignment::Center);
    frame.render_widget(banner, area);
}

/// Barra superior de una linea con el aviso de nueva release, en color de acento
/// (informativo, no de advertencia) para distinguirlo del aviso degradado.
fn draw_update_banner(frame: &mut Frame, area: Rect, note: &str) {
    let banner = Paragraph::new(Line::from(Span::styled(
        format!(" {note} "),
        Style::default()
            .fg(ratatui::style::Color::Black)
            .bg(theme::ACCENT)
            .add_modifier(Modifier::BOLD),
    )))
    .alignment(Alignment::Center);
    frame.render_widget(banner, area);
}

/// Panel central cuando el demonio o la base no están disponibles.
fn draw_disconnected(frame: &mut Frame, area: Rect, reason: &str) {
    let text = Text::from(vec![
        Line::from(Span::styled(
            "Demonio no disponible",
            Style::default().fg(theme::TX).add_modifier(Modifier::BOLD),
        )),
        Line::from(""),
        Line::from(Span::styled(reason.to_string(), theme::dim())),
        Line::from(""),
        Line::from(vec![
            Span::raw("Pulsa "),
            Span::styled("r", Style::default().fg(theme::ACCENT)),
            Span::raw(" para reintentar, "),
            Span::styled("q", Style::default().fg(theme::ACCENT)),
            Span::raw(" para salir."),
        ]),
    ]);
    let panel = Paragraph::new(text)
        .alignment(Alignment::Center)
        .wrap(Wrap { trim: true })
        .block(theme::panel(" netusage "));
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
    fn degraded_note_shows_top_banner() {
        let mut state = AppState::new(Period::Today);
        state.connection = ConnState::Ready;
        state.degraded_note = Some("Modo degradado: solo total".into());
        let text = render_to_lines(60, 14, |f| draw(f, &state)).join("\n");
        assert!(text.contains("Modo degradado: solo total"), "{text}");
    }

    #[test]
    fn no_degraded_note_hides_banner() {
        let mut state = AppState::new(Period::Today);
        state.connection = ConnState::Ready;
        let text = render_to_lines(60, 14, |f| draw(f, &state)).join("\n");
        assert!(!text.contains("Modo degradado"), "{text}");
    }

    #[test]
    fn update_note_shows_top_banner() {
        let mut state = AppState::new(Period::Today);
        state.connection = ConnState::Ready;
        state.update_note = Some("Nueva version v0.2.0 disponible".into());
        let text = render_to_lines(70, 14, |f| draw(f, &state)).join("\n");
        assert!(text.contains("Nueva version v0.2.0 disponible"), "{text}");
    }

    #[test]
    fn no_update_note_hides_banner() {
        let mut state = AppState::new(Period::Today);
        state.connection = ConnState::Ready;
        let text = render_to_lines(70, 14, |f| draw(f, &state)).join("\n");
        assert!(!text.contains("Nueva version"), "{text}");
    }

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
