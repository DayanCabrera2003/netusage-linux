//! Reconocimiento de aplicaciones VPN/túnel.
//!
//! Una app VPN puede transportar el tráfico cifrado de otras (sobre todo si es
//! de espacio de usuario, tipo OpenVPN), así que su fila puede no representar
//! consumo "propio". No se oculta (falsearía el total): se marca para que el
//! usuario lo interprete bien.

/// Binarios de VPN/túnel conocidos (se comparan por el basename del ejecutable).
const KNOWN_VPN_BINARIES: &[&str] = &[
    "openvpn",
    "openvpn3",
    "openconnect",
    "wireguard",
    "wg",
    "wg-quick",
    "protonvpn",
    "protonvpn-app",
    "mullvad",
    "mullvad-daemon",
    "nordvpn",
    "nordvpnd",
    "expressvpn",
    "tailscale",
    "tailscaled",
    "vpnc",
    "charon",
];

/// Indica si `app_key`/`display_name` corresponden a una app VPN/túnel.
///
/// Reconoce el binario por su basename (la `app_key` es `uid:<UID>:<ruta>`) o un
/// `display_name` que contenga "vpn".
pub fn is_vpn(app_key: &str, display_name: &str) -> bool {
    if display_name.to_ascii_lowercase().contains("vpn") {
        return true;
    }
    let binary = app_key.rsplit('/').next().unwrap_or(app_key);
    KNOWN_VPN_BINARIES.contains(&binary)
}

#[cfg(test)]
mod tests {
    use super::is_vpn;

    #[test]
    fn recognizes_known_vpn_binaries() {
        assert!(is_vpn("uid:1000:/usr/sbin/openvpn", "openvpn"));
        assert!(is_vpn("uid:1000:/usr/bin/wg", "wg"));
        assert!(is_vpn("uid:1000:/opt/proton/protonvpn-app", "Proton VPN"));
    }

    #[test]
    fn recognizes_by_display_name_vpn() {
        assert!(is_vpn("uid:1000:/usr/bin/whatever", "Mi VPN"));
    }

    #[test]
    fn ordinary_apps_are_not_vpn() {
        assert!(!is_vpn("uid:1000:/usr/lib/firefox/firefox", "firefox"));
        assert!(!is_vpn("__system_other__", "Sistema / Otros"));
    }
}
