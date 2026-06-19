//! Widget de pie: barra de ayuda con los atajos de teclado.

use ratatui::layout::Rect;
use ratatui::style::Style;
use ratatui::text::{Line, Span};
use ratatui::widgets::Paragraph;
use ratatui::Frame;

use crate::ui::theme;

/// Atajos como pares (tecla, descripción).
const HINTS: [(&str, &str); 6] = [
    ("q", "salir"),
    ("Tab/l/h", "periodo"),
    ("j/k", "mover"),
    ("Enter", "detalle"),
    ("r", "refrescar"),
    ("Esc", "cerrar"),
];

/// Dibuja la barra de ayuda con las teclas en color de acento.
pub fn render(frame: &mut Frame, area: Rect) {
    let mut spans = vec![Span::raw(" ")];
    for (i, (key, desc)) in HINTS.iter().enumerate() {
        if i > 0 {
            spans.push(Span::styled(" · ", theme::dim()));
        }
        spans.push(Span::styled(*key, Style::default().fg(theme::ACCENT)));
        spans.push(Span::raw(" "));
        spans.push(Span::styled(*desc, theme::dim()));
    }
    frame.render_widget(Paragraph::new(Line::from(spans)), area);
}

#[cfg(test)]
mod tests {
    use crate::ui::render_to_lines;

    #[test]
    fn shows_keybinding_hints() {
        let text = render_to_lines(80, 1, |f| super::render(f, f.area())).join("\n");
        for hint in ["q salir", "periodo", "mover", "detalle", "refrescar"] {
            assert!(text.contains(hint), "falta la pista {hint}: {text}");
        }
    }
}
