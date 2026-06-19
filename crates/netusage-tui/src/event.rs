//! Traducción de eventos de teclado a mensajes del reductor.
//!
//! Función pura y testeable. El bucle de `app` lee las teclas con crossterm (de
//! forma síncrona, con un timeout que marca el ritmo de refresco) y delega aquí
//! el mapeo. No se usa un runtime asíncrono; ver desviaciones.

use ratatui::crossterm::event::{KeyCode, KeyEvent};

use crate::update::Message;

/// Mapea una tecla a un mensaje, o `None` si no hace nada.
///
/// `show_detail` cambia el significado de `Esc`: cierra el detalle si está
/// abierto, y si no, sale.
pub fn map_key(key: KeyEvent, show_detail: bool) -> Option<Message> {
    match key.code {
        KeyCode::Char('q') => Some(Message::Quit),
        KeyCode::Esc if show_detail => Some(Message::CloseDetail),
        KeyCode::Esc => Some(Message::Quit),
        KeyCode::Tab | KeyCode::Char('l') | KeyCode::Right => Some(Message::NextPeriod),
        KeyCode::Char('h') | KeyCode::Left => Some(Message::PrevPeriod),
        KeyCode::Char('j') | KeyCode::Down => Some(Message::SelectNext),
        KeyCode::Char('k') | KeyCode::Up => Some(Message::SelectPrev),
        KeyCode::Enter => Some(Message::ToggleDetail),
        KeyCode::Char('r') => Some(Message::Refresh),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::map_key;
    use crate::update::Message;
    use ratatui::crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

    fn key(code: KeyCode) -> KeyEvent {
        KeyEvent::new(code, KeyModifiers::empty())
    }

    #[test]
    fn maps_navigation_and_actions() {
        assert!(matches!(
            map_key(key(KeyCode::Char('q')), false),
            Some(Message::Quit)
        ));
        assert!(matches!(
            map_key(key(KeyCode::Tab), false),
            Some(Message::NextPeriod)
        ));
        assert!(matches!(
            map_key(key(KeyCode::Char('h')), false),
            Some(Message::PrevPeriod)
        ));
        assert!(matches!(
            map_key(key(KeyCode::Down), false),
            Some(Message::SelectNext)
        ));
        assert!(matches!(
            map_key(key(KeyCode::Enter), false),
            Some(Message::ToggleDetail)
        ));
        assert!(map_key(key(KeyCode::Char('x')), false).is_none());
    }

    #[test]
    fn esc_closes_detail_or_quits() {
        assert!(matches!(
            map_key(key(KeyCode::Esc), true),
            Some(Message::CloseDetail)
        ));
        assert!(matches!(
            map_key(key(KeyCode::Esc), false),
            Some(Message::Quit)
        ));
    }
}
