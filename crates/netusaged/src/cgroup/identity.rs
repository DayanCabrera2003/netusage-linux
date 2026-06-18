//! Resolución de la identidad de una aplicación a partir del nombre de su
//! scope/servicio de cgroup de systemd.
//!
//! Responsabilidad única: lógica pura de cadena (sin tocar el sistema) que
//! convierte un nombre como `app-gnome-firefox-2838.scope` en una identidad
//! legible `AppIdentity`. Incluye el desescapado de las secuencias `\xNN` que
//! systemd usa en los nombres de unidad.
//!
//! Convenciones de nombres reconocidas (systemd / freedesktop):
//! - `app-<launcher>-<app>-<RANDOM>.scope` (GNOME): el launcher es `gnome`, el
//!   `<RANDOM>` final (numérico o hash hex) se descarta.
//! - `app-<reverse.domain>-<RANDOM>.scope` (KDE): sin token de launcher; la
//!   identidad es el dominio inverso, p. ej. `org.kde.konsole`.
//! - `app-flatpak-<app-id>-<RANDOM>.scope` (Flatpak): launcher `flatpak`.
//! - Sufijo de instancia `@<id>` (systemd reciente): se recorta.
//! - `session.scope`, `init.scope`, servicios de sistema y cualquier nombre no
//!   reconocido se marcan como "no es app" para que el fallback los capture.

/// Identidad resuelta de un cgroup.
///
/// `app_key` es una clave estable (sirve de identificador de persistencia en
/// fases futuras); `display_name` es el texto legible que ve el usuario. Cuando
/// `is_app` es falso, el cgroup no es una aplicación de usuario atribuible y su
/// tráfico debe caer en el cubo de fallback.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AppIdentity {
    pub app_key: String,
    pub display_name: String,
    pub is_app: bool,
}

impl AppIdentity {
    /// Construye una identidad de aplicación a partir de su clave y nombre.
    fn app(app_key: String, display_name: String) -> Self {
        Self {
            app_key,
            display_name,
            is_app: true,
        }
    }

    /// Construye una identidad marcada como "no es app" (irá al fallback).
    fn non_app(raw: &str) -> Self {
        Self {
            app_key: raw.to_string(),
            display_name: raw.to_string(),
            is_app: false,
        }
    }
}

/// Launchers conocidos que preceden al identificador de la app en el nombre del
/// scope. Se consumen como prefijo y no forman parte de la identidad.
const LAUNCHERS: [&str; 3] = ["gnome", "kde", "flatpak"];

/// Convierte el nombre de un scope/servicio de cgroup en una `AppIdentity`.
///
/// `name` es el nombre del directorio del cgroup, p. ej.
/// `app-gnome-firefox-2838.scope`.
pub fn parse_scope(name: &str) -> AppIdentity {
    let core = strip_unit_suffix(name);

    // Solo los nombres `app-...` son candidatos a aplicación de usuario.
    let Some(rest) = core.strip_prefix("app-") else {
        return AppIdentity::non_app(core);
    };

    // Separar un posible token de launcher inicial (gnome/kde/flatpak).
    let app_part = match rest.split_once('-') {
        Some((head, tail)) if LAUNCHERS.contains(&head) => tail,
        _ => rest,
    };

    // Recortar el id transitorio final (instancia `@id` o `-<RANDOM>`).
    let trimmed = strip_transient_id(app_part);
    if trimmed.is_empty() {
        return AppIdentity::non_app(core);
    }

    let app_key = unescape_systemd(trimmed);
    let display_name = display_from_key(&app_key);
    AppIdentity::app(app_key, display_name)
}

/// Quita el sufijo de unidad (`.scope` o `.service`) si está presente.
fn strip_unit_suffix(name: &str) -> &str {
    name.strip_suffix(".scope")
        .or_else(|| name.strip_suffix(".service"))
        .unwrap_or(name)
}

/// Recorta el identificador transitorio que systemd añade al final del nombre.
///
/// Reconoce dos formas:
/// - Instancia `@<id>`: se corta en el último `@` (p. ej. `Nautilus@1` →
///   `Nautilus`).
/// - Sufijo `-<RANDOM>` donde `<RANDOM>` es un id puramente hexadecimal
///   (numérico o hash), p. ej. `firefox-2838` → `firefox`,
///   `org.kde.konsole-c1a2` → `org.kde.konsole`.
fn strip_transient_id(app_part: &str) -> &str {
    if let Some((head, _instance)) = app_part.rsplit_once('@') {
        return head;
    }
    if let Some((head, last)) = app_part.rsplit_once('-') {
        if is_transient_id(last) {
            return head;
        }
    }
    app_part
}

/// Indica si `s` parece un id transitorio de systemd: no vacío y compuesto solo
/// por dígitos hexadecimales (cubre tanto los numéricos tipo PID como los
/// hashes hex).
fn is_transient_id(s: &str) -> bool {
    !s.is_empty() && s.chars().all(|c| c.is_ascii_hexdigit())
}

/// Deriva el nombre legible a partir de la clave de app.
///
/// Para identificadores en notación de dominio inverso (con puntos) se usa el
/// último segmento (`org.kde.konsole` → `konsole`). En otro caso, la propia
/// clave (`firefox` → `firefox`).
fn display_from_key(app_key: &str) -> String {
    match app_key.rsplit_once('.') {
        Some((_, last)) if !last.is_empty() => last.to_string(),
        _ => app_key.to_string(),
    }
}

/// Desescapa las secuencias `\xNN` de un nombre de unidad de systemd.
///
/// systemd codifica los bytes no permitidos en nombres de unidad como `\xNN`
/// (hex). Esta función las decodifica byte a byte y reconstruye la cadena
/// (`\x2d` → `-`, `\x2e` → `.`). Cualquier `\x` malformado se deja tal cual.
/// Es la operación inversa de la convención que aplica `systemd-escape -u`.
pub fn unescape_systemd(name: &str) -> String {
    let bytes = name.as_bytes();
    let mut out: Vec<u8> = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        // Buscar el patrón `\xNN` (cuatro bytes: '\\', 'x', hex, hex).
        if bytes[i] == b'\\' && i + 3 < bytes.len() && bytes[i + 1] == b'x' {
            let hi = (bytes[i + 2] as char).to_digit(16);
            let lo = (bytes[i + 3] as char).to_digit(16);
            if let (Some(hi), Some(lo)) = (hi, lo) {
                out.push((hi * 16 + lo) as u8);
                i += 4;
                continue;
            }
        }
        out.push(bytes[i]);
        i += 1;
    }
    String::from_utf8_lossy(&out).into_owned()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse(name: &str) -> AppIdentity {
        parse_scope(name)
    }

    #[test]
    fn gnome_scope_resolves_to_app_name() {
        let id = parse("app-gnome-firefox-2838.scope");
        assert!(id.is_app);
        assert_eq!(id.app_key, "firefox");
        assert_eq!(id.display_name, "firefox");
    }

    #[test]
    fn kde_reverse_domain_resolves_to_last_segment() {
        let id = parse("app-org.kde.konsole-c1a2.scope");
        assert!(id.is_app);
        assert_eq!(id.app_key, "org.kde.konsole");
        assert_eq!(id.display_name, "konsole");
    }

    #[test]
    fn flatpak_scope_resolves_to_app_id() {
        let id = parse("app-flatpak-org.mozilla.firefox-1234.scope");
        assert!(id.is_app);
        assert_eq!(id.app_key, "org.mozilla.firefox");
        assert_eq!(id.display_name, "firefox");
    }

    #[test]
    fn at_instance_suffix_is_trimmed() {
        let id = parse("app-gnome-org.gnome.Nautilus@1.scope");
        assert!(id.is_app);
        assert_eq!(id.app_key, "org.gnome.Nautilus");
        assert_eq!(id.display_name, "Nautilus");
    }

    #[test]
    fn systemd_escapes_are_decoded_in_app_name() {
        let id = parse("app-foo\\x2dbar-1.scope");
        assert!(id.is_app);
        assert_eq!(id.app_key, "foo-bar");
        assert_eq!(id.display_name, "foo-bar");
    }

    #[test]
    fn non_app_scopes_are_flagged() {
        for name in ["session.scope", "init.scope", "dbus.service"] {
            let id = parse(name);
            assert!(!id.is_app, "{name} debería marcarse como no-app");
        }
    }

    #[test]
    fn empty_and_malformed_never_panic_and_are_non_app() {
        for name in ["", ".scope", "app-.scope", "app-"] {
            let id = parse(name);
            assert!(!id.is_app, "{name:?} debería ser no-app");
        }
    }

    #[test]
    fn unescape_decodes_hex_sequences() {
        assert_eq!(unescape_systemd("foo\\x2dbar"), "foo-bar");
        assert_eq!(unescape_systemd("a\\x2db\\x2ec"), "a-b.c");
        // Sin secuencias: la cadena se devuelve intacta.
        assert_eq!(unescape_systemd("plain"), "plain");
        // `\x2d` es `-` y `\x2e` es `.`, en paridad con `systemd-escape -u`.
        assert_eq!(unescape_systemd("\\x2e"), ".");
    }
}
