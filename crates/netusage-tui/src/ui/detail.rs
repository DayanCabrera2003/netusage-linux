//! Widget de detalle por aplicación: panel emergente con rx/tx/total.

use ratatui::layout::{Constraint, Flex, Layout, Rect};
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span, Text};
use ratatui::widgets::{Clear, Paragraph};
use ratatui::Frame;

use crate::format::format_bytes;
use crate::state::AppState;
use crate::ui::theme;

/// Dibuja el panel de detalle de la app seleccionada, si la hay.
pub fn render(frame: &mut Frame, area: Rect, state: &AppState) {
    let Some(app) = state.selected_app() else {
        return;
    };

    let popup = centered(area, 50, 9);
    let text = Text::from(vec![
        Line::from(Span::styled(
            app.display_name.clone(),
            Style::default()
                .fg(theme::ACCENT)
                .add_modifier(Modifier::BOLD),
        )),
        Line::from(""),
        Line::from(vec![
            Span::raw("↓ rx     "),
            Span::styled(format_bytes(app.rx_bytes), Style::default().fg(theme::RX)),
        ]),
        Line::from(vec![
            Span::raw("↑ tx     "),
            Span::styled(format_bytes(app.tx_bytes), Style::default().fg(theme::TX)),
        ]),
        Line::from(vec![
            Span::raw("Σ total  "),
            Span::styled(
                format_bytes(app.total()),
                Style::default().add_modifier(Modifier::BOLD),
            ),
        ]),
        Line::from(""),
        Line::from(Span::styled(app.app_key.clone(), theme::dim())),
    ]);
    let panel = Paragraph::new(text).block(theme::panel(" Detalle (Esc cierra) "));

    frame.render_widget(Clear, popup); // limpia lo que haya detrás
    frame.render_widget(panel, popup);
}

/// Calcula un rectángulo centrado de `pct_x`% de ancho y `rows` de alto sobre
/// `area`.
fn centered(area: Rect, pct_x: u16, rows: u16) -> Rect {
    let [row] = Layout::vertical([Constraint::Length(rows)])
        .flex(Flex::Center)
        .areas(area);
    let [cell] = Layout::horizontal([Constraint::Percentage(pct_x)])
        .flex(Flex::Center)
        .areas(row);
    cell
}

#[cfg(test)]
mod tests {
    use crate::model::{AppUsage, PeriodSummary};
    use crate::period::Period;
    use crate::state::AppState;
    use crate::ui::render_to_lines;

    #[test]
    fn shows_selected_app_details() {
        let mut state = AppState::new(Period::Today);
        state.set_summary(PeriodSummary {
            period: Period::Today,
            total_rx: 0,
            total_tx: 0,
            apps: vec![AppUsage {
                app_key: "/usr/bin/brave".into(),
                display_name: "brave".into(),
                rx_bytes: 2048,
                tx_bytes: 1024,
            }],
        });
        let text = render_to_lines(60, 16, |f| super::render(f, f.area(), &state)).join("\n");
        assert!(text.contains("brave"), "{text}");
        assert!(text.contains("2.0 KiB"), "falta rx: {text}");
        assert!(text.contains("3.0 KiB"), "falta total: {text}");
    }
}
