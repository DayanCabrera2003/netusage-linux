//! Widget de pie: barra de ayuda con los atajos de teclado.

use ratatui::layout::Rect;
use ratatui::style::{Modifier, Style};
use ratatui::widgets::Paragraph;
use ratatui::Frame;

/// Texto de ayuda con los atajos disponibles.
const HELP: &str =
    " q salir · Tab/l periodo→ · h periodo← · j/k mover · Enter detalle · r refrescar ";

/// Dibuja la barra de ayuda.
pub fn render(frame: &mut Frame, area: Rect) {
    let help = Paragraph::new(HELP).style(Style::default().add_modifier(Modifier::DIM));
    frame.render_widget(help, area);
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
