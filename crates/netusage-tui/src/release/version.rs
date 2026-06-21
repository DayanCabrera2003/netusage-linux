//! Comparacion de versiones para el aviso de nueva release.
//!
//! Responsabilidad unica: decidir si una version remota (el tag de la ultima
//! release de GitHub) es estrictamente mas nueva que la version compilada de la
//! TUI, con semantica semver. Logica pura y testeable.

use semver::Version;

/// Version compilada de la TUI (la declarada en `Cargo.toml`).
pub const CURRENT: &str = env!("CARGO_PKG_VERSION");

/// Indica si `latest_tag` es una version estrictamente mas nueva que `current`.
///
/// Acepta tags con o sin el prefijo `v` habitual en git (`v0.2.0` o `0.2.0`).
/// Si cualquiera de las dos no es una version semver valida, devuelve `false`:
/// ante datos ambiguos preferimos no avisar.
pub fn is_newer(latest_tag: &str, current: &str) -> bool {
    match (parse(latest_tag), parse(current)) {
        (Some(latest), Some(cur)) => latest > cur,
        _ => false,
    }
}

/// Parsea una version, tolerando el prefijo `v` de los tags de git.
fn parse(s: &str) -> Option<Version> {
    let trimmed = s.trim();
    let trimmed = trimmed.strip_prefix('v').unwrap_or(trimmed);
    Version::parse(trimmed).ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn newer_patch_minor_and_major_are_detected() {
        assert!(is_newer("v0.1.1", "0.1.0"));
        assert!(is_newer("v0.2.0", "0.1.0"));
        assert!(is_newer("v1.0.0", "0.9.9"));
    }

    #[test]
    fn same_or_older_is_not_newer() {
        assert!(!is_newer("v0.1.0", "0.1.0"));
        assert!(!is_newer("0.1.0", "0.1.0"));
        assert!(!is_newer("v0.0.9", "0.1.0"));
    }

    #[test]
    fn prefix_v_is_optional_on_both_sides() {
        assert!(is_newer("0.2.0", "v0.1.0"));
        assert!(is_newer("v0.2.0", "v0.1.0"));
    }

    #[test]
    fn invalid_versions_do_not_trigger_a_notice() {
        assert!(!is_newer("latest", "0.1.0"));
        assert!(!is_newer("", "0.1.0"));
        assert!(!is_newer("v0.2.0", "no-semver"));
    }

    #[test]
    fn a_stable_release_is_newer_than_its_prerelease() {
        // Semver: 0.1.0 > 0.1.0-beta.1.
        assert!(is_newer("v0.1.0", "0.1.0-beta.1"));
        assert!(!is_newer("v0.1.0-beta.1", "0.1.0"));
    }
}
