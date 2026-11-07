// Retry transient disconnects and ordinary kicks; permanent refusals require user action.
pub fn allowed(reason: &str, key: Option<&str>) -> bool {
    const KEYS: [&str; 14] = [
        "multiplayer.disconnect.banned",
        "multiplayer.disconnect.ip_banned",
        "multiplayer.disconnect.not_whitelisted",
        "multiplayer.disconnect.incompatible",
        "multiplayer.disconnect.outdated_client",
        "multiplayer.disconnect.outdated_server",
        "multiplayer.disconnect.duplicate_login",
        "multiplayer.disconnect.unverified_username",
        "disconnect.loginFailedInfo.invalidSession",
        "disconnect.loginFailedInfo.insufficientPrivileges",
        "disconnect.loginFailedInfo.userBanned",
        "horizon.multiplayer.not_signed_in",
        "horizon.multiplayer.invalid_address",
        "disconnect.loginFailedInfo.invalidPublicKeySignature",
    ];
    if key.is_some_and(|key| KEYS.iter().any(|prefix| key.starts_with(prefix))) {
        return false;
    }
    let reason = reason.to_lowercase();
    ![
        "banned",
        "ban:",
        "ban ",
        "whitelist",
        "white-list",
        "invalid session",
        "invalid server address",
        "failed to verify username",
        "another location",
        "incompatible",
        "outdated",
        "not authenticated",
        "authentication failed",
        "multiplayer is disabled",
        "microsoft account",
        "забан",
        "заблокирован",
        "белом списке",
        "белый список",
        "недействительн",
        "запрещён",
        "запрещен",
    ]
    .iter()
    .any(|text| reason.contains(text))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn transient_kicks_retry_but_permanent_refusals_do_not() {
        for reason in [
            "Timed out",
            "Disconnected",
            "Kicked for being AFK",
            "Сервер перезагружается",
            "Connection reset",
        ] {
            assert!(
                allowed(reason, Some("multiplayer.disconnect.kicked")),
                "{reason}"
            );
        }
        for reason in [
            "You are banned",
            "Вы забанены",
            "Not on the whitelist",
            "Invalid session",
            "Logged in from another location",
        ] {
            assert!(!allowed(reason, None), "{reason}");
        }
        assert!(!allowed(
            "Custom text",
            Some("multiplayer.disconnect.banned.expiration")
        ));
        assert!(!allowed(
            "Custom text",
            Some("disconnect.loginFailedInfo.invalidSession")
        ));
    }
}
