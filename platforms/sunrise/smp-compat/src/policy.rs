//! Connection-local policy: no global switch, DNS lookup or retained reconnect state.

/// Replace only the initial brand for the official Horizon SMP domain.
/// Preserve the engine's bytes verbatim on every other connection.
pub fn brand_for_address(address: &str, original: Vec<u8>) -> Vec<u8> {
    if horizon_smp(address) {
        // A protocol String: VarInt byte length, followed by seven UTF-8 bytes.
        b"\x07horizon".to_vec()
    } else {
        original
    }
}

fn horizon_smp(address: &str) -> bool {
    let address = address.trim();
    let host = match address.rsplit_once(':') {
        Some((host, port)) => {
            if port.is_empty() || port.parse::<u16>().is_err() {
                return false;
            }
            host
        }
        None => address,
    };
    let host = host.trim_end_matches('.').to_ascii_lowercase();
    host == "playhorizon.dev" || host.ends_with(".playhorizon.dev")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn official_smp_gets_a_protocol_encoded_horizon_brand() {
        for address in [
            "play.playhorizon.dev",
            "playhorizon.dev:25565",
            " PLAY.PLAYHORIZON.DEV.:25565 ",
        ] {
            assert_eq!(brand_for_address(address, vec![0xff]), b"\x07horizon");
        }
    }

    #[test]
    fn unrelated_and_malformed_addresses_keep_the_original_bytes() {
        for address in [
            "",
            "localhost",
            "127.0.0.1:25565",
            "[::1]:25565",
            "otherplayhorizon.dev",
            "playhorizon.dev.attacker.example",
            "playhorizon.dev:",
            "playhorizon.dev:65536",
            "playhorizon.dev:-1",
            "user@playhorizon.dev",
        ] {
            let original = vec![0xff, 0, 7];
            assert_eq!(brand_for_address(address, original.clone()), original);
        }
    }

    #[test]
    fn reconnects_and_parallel_connections_do_not_inherit_the_brand() {
        std::thread::scope(|scope| {
            for address in ["play.playhorizon.dev", "sunrise.example", "localhost"] {
                scope.spawn(move || {
                    for _ in 0..100 {
                        let original = b"\x07vanilla".to_vec();
                        let result = brand_for_address(address, original.clone());
                        assert_eq!(
                            result,
                            if address == "play.playhorizon.dev" {
                                b"\x07horizon".to_vec()
                            } else {
                                original
                            }
                        );
                    }
                });
            }
        });
    }
}
