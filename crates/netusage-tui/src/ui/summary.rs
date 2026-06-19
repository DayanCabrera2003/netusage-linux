//! Widget de resumen del periodo: total rx/tx.

use ratatui::layout::Rect;
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::Paragraph;
use ratatui::Frame;

use crate::format::format_bytes;
use crate::state::AppState;
use crate::ui::theme;

/// Dibuja el bloque de resumen con el total del periodo activo.
pub fn render(frame: &mut Frame, area: Rect, state: &AppState) {
    let (rx, tx, total) = match &state.summary {
        Some(summary) => (summary.total_rx, summary.total_tx, summary.total()),
        None => (0, 0, 0),
    };

    let line = Line::from(vec![
        Span::styled("  ↓ ", Style::default().fg(theme::RX)),
        Span::styled(
            format!("{:<11}", format_bytes(rx)),
            Style::default().fg(theme::RX),
        ),
        Span::styled("↑ ", Style::default().fg(theme::TX)),
        Span::styled(
            format!("{:<11}", format_bytes(tx)),
            Style::default().fg(theme::TX),
        ),
        Span::styled("Σ ", theme::dim()),
        Span::styled(
            format_bytes(total),
            Style::default().add_modifier(Modifier::BOLD),
        ),
    ]);

    let title = format!(" {} ", state.period.label());
    frame.render_widget(Paragraph::new(line).block(theme::panel(&title)), area);
}

#[cfg(test)]
mod tests {
    use crate::model::{AppUsage, PeriodSummary};
    use crate::period::Period;
    use crate::state::AppState;
    use crate::ui::render_to_lines;

    #[test]
    fn shows_period_label_and_totals() {
        let mut state = AppState::new(Period::Today);
        state.set_summary(PeriodSummary {
            period: Period::Today,
            total_rx: 1024,
            total_tx: 2048,
            apps: vec![AppUsage {
                app_key: "/x".into(),
                display_name: "x".into(),
                rx_bytes: 1024,
                tx_bytes: 2048,
            }],
        });

        let lines = render_to_lines(50, 3, |f| super::render(f, f.area(), &state));
        let text = lines.join("\n");
        assert!(
            text.contains("Hoy"),
            "falta la etiqueta del periodo: {text}"
        );
        assert!(text.contains("1.0 KiB"), "falta rx formateado: {text}");
        assert!(text.contains("2.0 KiB"), "falta tx formateado: {text}");
        assert!(text.contains("3.0 KiB"), "falta el total: {text}");
    }
}
