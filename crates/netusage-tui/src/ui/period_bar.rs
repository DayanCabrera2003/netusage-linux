//! Widget del selector de periodo: pestañas hoy/semana/mes/mes anterior.

use ratatui::layout::Rect;
use ratatui::style::{Modifier, Style};
use ratatui::widgets::{Block, Borders, Tabs};
use ratatui::Frame;

use crate::period::Period;
use crate::state::AppState;

/// Dibuja las cuatro pestañas resaltando el periodo activo.
pub fn render(frame: &mut Frame, area: Rect, state: &AppState) {
    let titles: Vec<&str> = Period::all().iter().map(|p| p.label()).collect();
    let selected = Period::all()
        .iter()
        .position(|p| *p == state.period)
        .unwrap_or(0);

    let tabs = Tabs::new(titles)
        .select(selected)
        .highlight_style(Style::default().add_modifier(Modifier::REVERSED))
        .block(Block::default().borders(Borders::ALL).title(" Periodo "));
    frame.render_widget(tabs, area);
}

#[cfg(test)]
mod tests {
    use crate::period::Period;
    use crate::state::AppState;
    use crate::ui::render_to_lines;

    #[test]
    fn shows_all_period_labels() {
        let state = AppState::new(Period::Month);
        let text = render_to_lines(60, 3, |f| super::render(f, f.area(), &state)).join("\n");
        for label in ["Hoy", "Semana", "Mes", "Mes anterior"] {
            assert!(text.contains(label), "falta la etiqueta {label}: {text}");
        }
    }
}
