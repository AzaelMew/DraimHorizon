// Real two-client integration against a local relay started by tools/test_relay_e2e.py.
use claim_borders::{
    party::{Claim, Party, Snapshot},
    relay_settings::permits_frame,
};
use claim_borders_relay_client::{Config, Event, Handle, Peer, realm_id};
use std::{
    path::PathBuf,
    time::{Duration, Instant},
};
fn wait<T>(h: &Handle, mut f: impl FnMut(Event) -> Option<T>) -> T {
    let end = Instant::now() + Duration::from_secs(15);
    while Instant::now() < end {
        if let Some(e) = h.try_event() {
            if let Some(v) = f(e) {
                return v;
            }
        } else {
            std::thread::sleep(Duration::from_millis(5));
        }
    }
    panic!("relay integration timed out");
}
fn message(h: &Handle) -> (Peer, String) {
    wait(h, |e| {
        if let Event::Message { peer, body } = e {
            Some((peer, body))
        } else {
            None
        }
    })
}
fn main() {
    let url = std::env::var("DH_RELAY_E2E_URL").expect("local test relay URL");
    let root = std::env::temp_dir().join(format!("claim-relay-smoke-{}", std::process::id()));
    std::fs::create_dir_all(&root).unwrap();
    let make = |uuid, name: &str| {
        Handle::spawn(Config {
            url: url.clone(),
            realm: realm_id("smoke.example:25565"),
            uuid,
            name: name.into(),
            identity_path: PathBuf::from(&root).join(format!("{uuid}.json")),
            access_token: None,
        })
        .unwrap()
    };
    let a = make(1, "Alice");
    let ac = wait(&a, |e| {
        if let Event::Connected { code } = e {
            Some(code)
        } else {
            None
        }
    });
    let b = make(2, "Bob");
    let bc = wait(&b, |e| {
        if let Event::Connected { code } = e {
            Some(code)
        } else {
            None
        }
    });
    wait(&a, |e| {
        if let Event::Peers(peers) = e {
            peers.into_iter().find(|p| p.code == bc)
        } else {
            None
        }
    });
    wait(&b, |e| {
        if let Event::Peers(peers) = e {
            peers.into_iter().find(|p| p.code == ac)
        } else {
            None
        }
    });
    let mut ap = Party::new("server#Alice", 1);
    let mut bp = Party::new("server#Bob", 2);
    let request = ap.invite(2, "Bob", 0).unwrap().remove(0);
    a.send(&bc, &request.body).unwrap();
    let (peer, body) = message(&b);
    assert_eq!(peer.code, ac);
    assert!(permits_frame(None, None, false, "i", &peer.code));
    bp.receive(1, "Alice", &body, 1).unwrap();
    assert!(bp.friends().is_empty());
    // Simulate manual confirmation of the independently exchanged identity code.
    let ack = bp.accept(1, 2).unwrap().remove(0);
    b.send(&ac, &ack.body).unwrap();
    let (peer, body) = message(&a);
    assert!(permits_frame(None, Some(&bc), true, "a", &peer.code));
    ap.receive(2, "Bob", &body, 3).unwrap();
    let frame = ap
        .snapshot_for(
            2,
            Snapshot {
                dimension: "minecraft:overworld".into(),
                position: [-1900.0, 64.0, 48.0],
                health: 15.0,
                max_health: 20.0,
            },
        )
        .unwrap();
    a.send(&bc, &frame.body).unwrap();
    let (peer, body) = message(&b);
    assert!(permits_frame(Some(&ac), None, false, "s", &peer.code));
    assert!(!permits_frame(Some(&ac), None, false, "s", &bc));
    bp.receive(1, "Alice", &body, 4).unwrap();
    assert_eq!(bp.online(&[1], 4)[0].health_percent, 75.0);
    assert!(
        bp.receive(1, "Alice", &body, 5).is_err(),
        "replayed status accepted"
    );
    let claims = [
        Claim {
            dimension: "minecraft:overworld".into(),
            x: -1904,
            z: 48,
        },
        Claim {
            dimension: "minecraft:overworld".into(),
            x: -1920,
            z: 48,
        },
    ];
    for (i, frame) in ap
        .claims_for(2, 1, &claims)
        .unwrap()
        .into_iter()
        .enumerate()
    {
        a.send(&bc, &frame.body).unwrap();
        let (_, body) = message(&b);
        bp.receive(1, "Alice", &body, 6 + i as u64).unwrap();
        if i == 0 {
            assert!(bp.online(&[1], 6)[0].claims.is_none());
        }
    }
    assert_eq!(bp.online(&[1], 8)[0].claims.as_ref().unwrap().len(), 2);
    let traffic_started = Instant::now();
    for n in 0..40 {
        let frame = ap
            .snapshot_for(
                2,
                Snapshot {
                    dimension: "minecraft:overworld".into(),
                    position: [n as f64, 64.0, 48.0],
                    health: 15.0,
                    max_health: 20.0,
                },
            )
            .unwrap();
        a.send(&bc, &frame.body).unwrap();
        let (_, body) = message(&b);
        bp.receive(1, "Alice", &body, 100 + n).unwrap();
    }
    assert!(
        traffic_started.elapsed() < Duration::from_secs(5),
        "telemetry still serialized behind old 125ms pacing"
    );
    drop(b);
    wait(&a, |e| {
        if let Event::Peers(peers) = e {
            (!peers.iter().any(|p| p.code == bc)).then_some(())
        } else {
            None
        }
    });
    ap.forget_session(2);
    bp.disconnect();
    let b = make(2, "Bob");
    let reconnected = wait(&b, |e| {
        if let Event::Connected { code } = e {
            Some(code)
        } else {
            None
        }
    });
    assert_eq!(reconnected, bc);
    wait(&a, |e| {
        if let Event::Peers(peers) = e {
            peers.iter().any(|p| p.code == bc).then_some(())
        } else {
            None
        }
    });
    wait(&b, |e| {
        if let Event::Peers(peers) = e {
            peers.iter().any(|p| p.code == ac).then_some(())
        } else {
            None
        }
    });
    let hello = ap.start_session(2, 1000).unwrap().remove(0);
    a.send(&bc, &hello.body).unwrap();
    let (_, body) = message(&b);
    let reply = bp.receive(1, "Alice", &body, 1001).unwrap().remove(0);
    b.send(&ac, &reply.body).unwrap();
    let (_, body) = message(&a);
    ap.receive(2, "Bob", &body, 1002).unwrap();
    assert!(ap.peer_ready(2));
    bp.forget_session(1);
    assert!(bp.online(&[1], 1003).is_empty());
    drop(a);
    drop(b);
    let _ = std::fs::remove_dir_all(root);
    println!(
        "PASS: two real WebSocket clients, mutual consent, E2E status/claims, pin/replay rejection, high-rate telemetry, reconnect and disconnect cleanup"
    );
}
