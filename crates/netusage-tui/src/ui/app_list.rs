//! Widget central: lista de apps ordenada por consumo, con barras de
//! proporción y resaltado de la fila seleccionada.

use ratatui::layout::Rect;
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, List, ListItem, ListState};
use ratatui::Frame;

use crate::format::format_bytes;
use crate::model::AppUsage;
use crate::state::AppState;

/// Ancho fijo de la barra de proporción, en caracteres.
const BAR_WIDTH: usize = 16;
/// Ancho de la columna del nombre de app.
const NAME_WIDTH: usize = 24;

/// Dibuja la lista de apps. La selección y el scroll los gestiona `ListState`.
pub fn render(frame: &mut Frame, area: Rect, state: &AppState) {
    let apps: &[AppUsage] = state
        .summary
        .as_ref()
        .map(|s| s.apps.as_slice())
        .unwrap_or(&[]);
    let max = apps.iter().map(|a| a.total()).max().unwrap_or(0).max(1);

    let items: Vec<ListItem> = apps.iter().map(|app| row(app, max)).collect();
    let list = List::new(items)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .title(" Aplicaciones "),
        )
        .highlight_style(Style::default().add_modifier(Modifier::REVERSED))
        .highlight_symbol("> ");

    let mut list_state = ListState::default();
    if !apps.is_empty() {
        list_state.select(Some(state.selected.min(apps.len() - 1)));
    }
    frame.render_stateful_widget(list, area, &mut list_state);
}

/// Construye la fila de una app: nombre, barra proporcional y total formateado.
fn row(app: &AppUsage, max: u64) -> ListItem<'static> {
    let filled = ((app.total() as u128 * BAR_WIDTH as u128) / max as u128) as usize;
    let bar = format!("{}{}", "#".repeat(filled), ".".repeat(BAR_WIDTH - filled));
    let line = Line::from(vec![
        Span::raw(format!(
            "{:<width$} ",
            truncate(&app.display_name, NAME_WIDTH),
            width = NAME_WIDTH
        )),
        Span::raw(format!("{bar} ")),
        Span::raw(format!("{:>10}", format_bytes(app.total()))),
    ]);
    ListItem::new(line)
}

/// Trunca `name` a `max` caracteres, con elipsis si se recorta.
fn truncate(name: &str, max: usize) -> String {
    if name.chars().count() <= max {
        name.to_string()
    } else {
        let head: String = name.chars().take(max - 1).collect();
        format!("{head}…")
    }
}

#[cfg(test)]
mod tests {
    use crate::model::{AppUsage, PeriodSummary};
    use crate::period::Period;
    use crate::state::AppState;
    use crate::ui::render_to_lines;

    fn app(name: &str, total: u64) -> AppUsage {
        AppUsage {
            app_key: format!("/{name}"),
            display_name: name.to_string(),
            rx_bytes: total,
            tx_bytes: 0,
        }
    }

    fn state_with(apps: Vec<AppUsage>, selected: usize) -> AppState {
        let mut state = AppState::new(Period::Today);
        state.set_summary(PeriodSummary {
            period: Period::Today,
            total_rx: 0,
            total_tx: 0,
            apps,
        });
        state.selected = selected;
        state
    }

    #[test]
    fn bigger_app_has_a_longer_bar() {
        let state = state_with(vec![app("big", 1000), app("small", 100)], 0);
        let lines = render_to_lines(70, 6, |f| super::render(f, f.area(), &state));
        let big = lines.iter().find(|l| l.contains("big")).unwrap();
        let small = lines.iter().find(|l| l.contains("small")).unwrap();
        let bars = |l: &str| l.chars().filter(|c| *c == '#').count();
        assert!(bars(big) > bars(small), "big={big:?} small={small:?}");
    }

    #[test]
    fn selected_row_has_the_marker() {
        let state = state_with(vec![app("a", 100), app("b", 50)], 1);
        let lines = render_to_lines(70, 6, |f| super::render(f, f.area(), &state));
        // El símbolo de selección "> " precede a la fila seleccionada (b).
        let b = lines.iter().find(|l| l.contains("b ")).unwrap();
        assert!(
            b.contains(">"),
            "la fila seleccionada debe llevar marcador: {b:?}"
        );
    }
}
