//! Loopback check of the actual mixin helper's bytes, not a running Sunrise client.
extern crate self as mod_draimhorizon_smp_compat;

#[path = "../src/policy.rs"]
mod policy;
pub use policy::brand_for_address;
include!("../src/mixins/brand.rs");

use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::time::Duration;

fn varint(mut value: usize, out: &mut Vec<u8>) {
    loop {
        let mut byte = (value & 127) as u8;
        value >>= 7;
        if value != 0 {
            byte |= 128;
        }
        out.push(byte);
        if value == 0 {
            break;
        }
    }
}

fn read_varint(input: &mut impl Read) -> usize {
    let mut result = 0;
    for shift in (0..35).step_by(7) {
        let mut byte = [0];
        input.read_exact(&mut byte).unwrap();
        result |= ((byte[0] & 127) as usize) << shift;
        if byte[0] < 128 {
            return result;
        }
    }
    panic!("oversized VarInt");
}

fn read_string(input: &mut impl Read) -> String {
    let length = read_varint(input);
    assert!(length <= 64);
    let mut bytes = vec![0; length];
    input.read_exact(&mut bytes).unwrap();
    String::from_utf8(bytes).unwrap()
}

#[test]
fn helper_bytes_pass_a_loopback_horizon_brand_gate_only_for_the_smp_address() {
    // This test owns the framing. Sunrise's real writer, SDK replacement,
    // authentication, encryption and extension negotiation are not exercised.
    for (address, expected) in [
        ("play.playhorizon.dev:25565", true),
        ("unrelated.example", false),
        ("playhorizon.dev.attacker.example", false),
    ] {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let socket = listener.local_addr().unwrap();
        std::thread::scope(|scope| {
            let server = scope.spawn(move || {
                let (mut connection, _) = listener.accept().unwrap();
                connection
                    .set_read_timeout(Some(Duration::from_secs(2)))
                    .unwrap();
                let length = read_varint(&mut connection);
                assert!(length <= 128);
                let mut frame = vec![0; length];
                connection.read_exact(&mut frame).unwrap();
                let mut reader = frame.as_slice();
                // Java 26.3 configuration Serverbound CustomPayload has ID 2.
                assert_eq!(read_varint(&mut reader), 2);
                assert_eq!(read_string(&mut reader), "minecraft:brand");
                let client_brand = read_string(&mut reader);
                assert!(reader.is_empty(), "unexpected bytes after brand");
                connection
                    .write_all(&[u8::from(client_brand == "horizon")])
                    .unwrap();
            });
            let mut connection = TcpStream::connect(socket).unwrap();
            connection
                .set_read_timeout(Some(Duration::from_secs(2)))
                .unwrap();
            let payload = brand(address, b"\x07vanilla".to_vec());
            let mut body = vec![2];
            varint("minecraft:brand".len(), &mut body);
            body.extend_from_slice(b"minecraft:brand");
            body.extend_from_slice(&payload);
            let mut frame = Vec::new();
            varint(body.len(), &mut frame);
            frame.extend_from_slice(&body);
            connection.write_all(&frame).unwrap();
            let mut accepted = [0];
            connection.read_exact(&mut accepted).unwrap();
            assert_eq!(accepted[0] == 1, expected, "{address}");
            server.join().unwrap();
        });
    }
}
