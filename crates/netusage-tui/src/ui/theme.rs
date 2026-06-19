//! Paleta y estilos compartidos por los widgets.
//!
//! Centraliza colores y helpers para un aspecto coherente: bordes redondeados,
//! títulos con acento, descarga/subida en colores distintos y un resaltado de
//! selección legible.

use ratatui::style::{Color, Modifier, Style};
use ratatui::widgets::{Block, BorderType, Borders};

/// Color de acento (barras, atajos, títulos).
pub const ACCENT: Color = Color::Cyan;
/// Descarga (rx).
pub const RX: Color = Color::Green;
/// Subida (tx).
pub const TX: Color = Color::Yellow;
/// Bordes y texto secundario.
pub const DIM: Color = Color::DarkGray;
/// Fondo de avisos (barra de modo degradado).
pub const WARN: Color = Color::Yellow;
/// Fondo del resaltado (pestaña activa, fila seleccionada).
pub const HIGHLIGHT_BG: Color = Color::Blue;

/// Estilo de un título de bloque.
pub fn title() -> Style {
    Style::default().fg(ACCENT).add_modifier(Modifier::BOLD)
}

/// Estilo del elemento resaltado (selección/pestaña activa).
pub fn highlight() -> Style {
    Style::default()
        .bg(HIGHLIGHT_BG)
        .fg(Color::White)
        .add_modifier(Modifier::BOLD)
}

/// Estilo tenue para texto secundario.
pub fn dim() -> Style {
    Style::default().fg(DIM)
}

/// Bloque con borde redondeado, borde tenue y título con acento.
pub fn panel(title_text: &str) -> Block<'_> {
    Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(DIM))
        .title(title_text)
        .title_style(title())
}

/// Caracteres de bloque parciales por octavos, para barras suaves.
const EIGHTHS: [&str; 8] = [" ", "▏", "▎", "▍", "▌", "▋", "▊", "▉"];

/// Construye una barra de `width` celdas que representa `value/max`, usando
/// bloques parciales para una transición suave. Devuelve la cadena (la parte
/// llena más el relleno tenue) lista para colorear.
pub fn bar(value: u64, max: u64, width: usize) -> (String, String) {
    let max = max.max(1);
    let eighths = (value as u128 * (width as u128) * 8 / max as u128) as usize;
    let full = (eighths / 8).min(width);
    let rem = eighths % 8;

    let mut filled = "█".repeat(full);
    let mut used = full;
    if rem > 0 && used < width {
        filled.push_str(EIGHTHS[rem]);
        used += 1;
    }
    let empty = "░".repeat(width.saturating_sub(used));
    (filled, empty)
}
