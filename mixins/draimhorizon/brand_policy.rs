// The client brand the official SMP judges, decided from the address the player typed.
//
// The network thread lives in the reusable client library, which the gameplay crate depends on,
// so this module cannot call the gameplay crate's `claim_borders::is_horizon_smp`. The rule is
// restated here for the login brand only; both copies are tested against the same accept and
// reject sets, and the port, trailing-dot and case forgiveness is the same.
//
// Nothing here is authentication: the brand is a client-asserted string the server reads once in
// the configuration phase, and this module only decides which bytes this client asserts.
use std::sync::atomic::{AtomicBool, Ordering};

/// The only server that reads the client brand.
pub const HORIZON_SMP_HOST: &str = "playhorizon.dev";
/// True for `playhorizon.dev` and its subdomains (`play.playhorizon.dev`), with or without a port.
pub fn is_horizon_smp(address: &str) -> bool {
    let address = address.trim().trim_end_matches('.').to_ascii_lowercase();
    let host = match address.rsplit_once(':') {
        Some((host, port)) if port.chars().all(|c| c.is_ascii_digit()) => host,
        _ => address.as_str(),
    }
    .trim_end_matches('.');
    host == HORIZON_SMP_HOST
        || host
            .strip_suffix(HORIZON_SMP_HOST)
            .is_some_and(|prefix| prefix.ends_with('.'))
}
/// Set once per connection attempt, from the address the player typed: an SRV record resolves the
/// connect host and port, and a friend tunnel hides the destination behind its code, so the typed
/// address is the only stable input. The network thread reads it for the same connection.
static ANNOUNCE_HORIZON: AtomicBool = AtomicBool::new(false);
/// True when this connection goes to the official SMP. Only the login brand reads this.
pub fn force_for(address: &str) {
    ANNOUNCE_HORIZON.store(is_horizon_smp(address), Ordering::Release);
}
/// The login `minecraft:brand` payload: a protocol String of `horizon` for the official SMP, and
/// the client's own bytes (`vanilla`, with its length prefix) for every other destination.
pub fn announced_brand(original: Vec<u8>) -> Vec<u8> {
    announced_brand_when(ANNOUNCE_HORIZON.load(Ordering::Acquire), original)
}
fn announced_brand_when(force: bool, original: Vec<u8>) -> Vec<u8> {
    if force {
        // VarInt byte length 7, then the seven UTF-8 bytes.
        b"\x07horizon".to_vec()
    } else {
        original
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn the_official_smp_gets_horizon_and_everything_else_keeps_its_own_brand() {
        for yes in [
            "play.playhorizon.dev",
            "PLAY.PlayHorizon.dev:25565",
            "playhorizon.dev.",
            "mc.playhorizon.dev:1",
        ] {
            assert!(is_horizon_smp(yes), "{yes}");
            assert_eq!(
                announced_brand_when(is_horizon_smp(yes), b"\x07vanilla".to_vec()),
                b"\x07horizon"
            );
        }
        for no in [
            "localhost",
            "127.0.0.1:25565",
            "hypixel.net",
            "evilplayhorizon.dev",
            "playhorizon.dev.evil.com",
            "playhorizon.devx",
            "",
        ] {
            assert!(!is_horizon_smp(no), "{no}");
            assert_eq!(
                announced_brand_when(is_horizon_smp(no), b"\x07vanilla".to_vec()),
                b"\x07vanilla",
                "{no}"
            );
        }
    }
    #[test]
    fn the_announced_brand_is_a_protocol_string_of_seven_bytes() {
        let brand = announced_brand_when(true, Vec::new());
        assert_eq!(brand, b"\x07horizon");
        assert_eq!(usize::from(brand[0]), "horizon".len());
        assert_eq!(&brand[1..], b"horizon");
    }
    #[test]
    fn the_original_bytes_survive_length_prefixes_and_empty_values() {
        for original in [
            Vec::new(),
            b"\x07vanilla".to_vec(),
            b"\x09minecraft".to_vec(),
        ] {
            assert_eq!(announced_brand_when(false, original.clone()), original);
        }
    }
    #[test]
    fn a_later_connection_elsewhere_clears_the_announcement() {
        force_for("play.playhorizon.dev");
        assert_eq!(announced_brand(b"\x07vanilla".to_vec()), b"\x07horizon");
        force_for("hypixel.net");
        assert_eq!(announced_brand(b"\x07vanilla".to_vec()), b"\x07vanilla");
        force_for("localhost");
        assert_eq!(announced_brand(b"\x07vanilla".to_vec()), b"\x07vanilla");
    }
}
