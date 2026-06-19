//! Widget central: lista de apps ordenada por consumo, con barras de
//! proporción y resaltado de la fila seleccionada.

use ratatui::layout::Rect;
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{List, ListItem, ListState, Paragraph};
use ratatui::Frame;

use crate::format::format_bytes;
use crate::model::AppUsage;
use crate::state::AppState;
use crate::ui::theme;

/// Ancho fijo de la barra de proporción, en caracteres.
const BAR_WIDTH: usize = 18;
/// Ancho de la columna del nombre de app.
const NAME_WIDTH: usize = 22;
/// Clave del cubo de fallback "Sistema / Otros" (persistida por el demonio).
const SYSTEM_OTHER_KEY: &str = "__system_other__";

/// Dibuja la lista de apps. La selección y el scroll los gestiona `ListState`.
pub fn render(frame: &mut Frame, area: Rect, state: &AppState) {
    let apps: &[AppUsage] = state
        .summary
        .as_ref()
        .map(|s| s.apps.as_slice())
        .unwrap_or(&[]);

    // Periodo sin datos: mensaje claro en vez de una lista vacía muda.
    if apps.is_empty() {
        let msg = Paragraph::new(Span::styled("Sin datos para este periodo", theme::dim()))
            .block(theme::panel(" Aplicaciones "));
        frame.render_widget(msg, area);
        return;
    }

    let max = apps.iter().map(|a| a.total()).max().unwrap_or(0).max(1);
    let grand_total: u64 = apps.iter().map(|a| a.total()).sum::<u64>().max(1);

    let items: Vec<ListItem> = apps.iter().map(|app| row(app, max, grand_total)).collect();
    let list = List::new(items)
        .block(theme::panel(" Aplicaciones "))
        .highlight_style(theme::highlight())
        .highlight_symbol("▌ ");

    let mut list_state = ListState::default();
    list_state.select(Some(state.selected.min(apps.len() - 1)));
    frame.render_stateful_widget(list, area, &mut list_state);
}

/// Construye la fila de una app: nombre, barra de bloques coloreada, total y
/// porcentaje del periodo.
fn row(app: &AppUsage, max: u64, grand_total: u64) -> ListItem<'static> {
    let (filled, empty) = theme::bar(app.total(), max, BAR_WIDTH);
    let pct = (app.total() as u128 * 100 / grand_total as u128) as u64;

    let name_style = if app.app_key == SYSTEM_OTHER_KEY {
        theme::dim()
    } else {
        Style::default()
    };

    let line = Line::from(vec![
        Span::styled(
            format!(
                "{:<width$} ",
                truncate(&app.display_name, NAME_WIDTH),
                width = NAME_WIDTH
            ),
            name_style,
        ),
        Span::styled(filled, Style::default().fg(theme::ACCENT)),
        Span::styled(empty, theme::dim()),
        Span::styled(
            format!(" {:>9}", format_bytes(app.total())),
            Style::default().add_modifier(Modifier::BOLD),
        ),
        Span::styled(format!(" {pct:>3}%"), theme::dim()),
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
        let bars = |l: &str| l.chars().filter(|c| *c == '█').count();
        assert!(bars(big) > bars(small), "big={big:?} small={small:?}");
    }

    #[test]
    fn empty_period_shows_message() {
        let state = state_with(vec![], 0);
        let text = render_to_lines(60, 5, |f| super::render(f, f.area(), &state)).join("\n");
        assert!(
            text.contains("Sin datos para este periodo"),
            "debe mostrar el mensaje de vacío: {text}"
        );
    }

    #[test]
    fn selected_row_has_the_marker() {
        let state = state_with(vec![app("a", 100), app("b", 50)], 1);
        let lines = render_to_lines(70, 6, |f| super::render(f, f.area(), &state));
        // El símbolo de selección "▌ " precede a la fila seleccionada (b).
        let b = lines.iter().find(|l| l.contains("b ")).unwrap();
        assert!(
            b.contains('▌'),
            "la fila seleccionada debe llevar marcador: {b:?}"
        );
    }
}
