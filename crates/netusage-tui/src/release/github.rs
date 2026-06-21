//! Consulta de la ultima release publicada en GitHub.
//!
//! Responsabilidad unica: pedir a la API de GitHub el tag de la ultima release
//! estable del repositorio y devolverlo. Es best-effort: cualquier fallo de red,
//! TLS, codigo de error HTTP o JSON inesperado se traduce en `None`, para que el
//! aviso de nueva release sea silencioso cuando no hay conectividad.

use std::time::Duration;

/// Endpoint de la ultima release estable. `releases/latest` excluye por diseno
/// las prereleases y los borradores, asi que solo avisa de versiones estables.
const LATEST_RELEASE_API: &str =
    "https://api.github.com/repos/DayanCabrera2003/netusage-linux/releases/latest";

/// Tiempo maximo total de la peticion (conexion + lectura). El chequeo corre en
/// un hilo aparte, pero acotamos igualmente para no dejarlo colgado sin red.
const REQUEST_TIMEOUT: Duration = Duration::from_secs(5);

/// Devuelve el `tag_name` de la ultima release estable, o `None` si no se pudo
/// obtener (sin red, error de la API, respuesta inesperada...).
pub fn latest_tag() -> Option<String> {
    let agent: ureq::Agent = ureq::Agent::config_builder()
        .timeout_global(Some(REQUEST_TIMEOUT))
        .build()
        .into();
    // GitHub exige un User-Agent en todas las peticiones a su API.
    let user_agent = format!("netusage-tui/{}", env!("CARGO_PKG_VERSION"));
    let body = agent
        .get(LATEST_RELEASE_API)
        .header("User-Agent", &user_agent)
        .header("Accept", "application/vnd.github+json")
        .call()
        .ok()?
        .body_mut()
        .read_to_string()
        .ok()?;
    tag_from_json(&body)
}

/// Extrae el campo `tag_name` del JSON de una release. Aislado de la red para
/// poder testear el parseo sin depender de GitHub.
fn tag_from_json(body: &str) -> Option<String> {
    let value: serde_json::Value = serde_json::from_str(body).ok()?;
    value
        .get("tag_name")?
        .as_str()
        .map(str::to_string)
        .filter(|s| !s.is_empty())
}

#[cfg(test)]
mod tests {
    use super::tag_from_json;

    #[test]
    fn extracts_tag_name_from_release_json() {
        let body = r#"{"tag_name":"v0.2.0","name":"Release 0.2.0","draft":false}"#;
        assert_eq!(tag_from_json(body), Some("v0.2.0".to_string()));
    }

    #[test]
    fn missing_or_empty_tag_is_none() {
        assert_eq!(tag_from_json(r#"{"name":"x"}"#), None);
        assert_eq!(tag_from_json(r#"{"tag_name":""}"#), None);
    }

    #[test]
    fn malformed_json_is_none() {
        assert_eq!(tag_from_json("no soy json"), None);
        assert_eq!(tag_from_json(""), None);
    }
}
