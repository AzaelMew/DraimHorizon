//! Bounded, background WebSocket transport with authenticated end-to-end encryption.
//! UUIDs and names are self-claimed. Callers must pair identity codes out of band.
mod crypto;
mod websocket;
use crypto::{Identity, Verified, string};
use serde_json::{Value, json};
use std::{
    collections::{HashMap, HashSet, VecDeque},
    path::PathBuf,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
        mpsc::{self, Receiver, SyncSender, TryRecvError},
    },
    thread,
    time::Duration,
};

#[derive(Clone, Debug)]
pub struct Config {
    pub url: String,
    pub realm: String,
    pub uuid: u128,
    pub name: String,
    pub identity_path: PathBuf,
    pub access_token: Option<String>,
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Peer {
    pub code: String,
    pub uuid: u128,
    pub name: String,
}
#[derive(Clone, Debug)]
pub enum Event {
    Connected { code: String },
    Peers(Vec<Peer>),
    Message { peer: Peer, body: String },
    Disconnected { reason: String },
    Notice { message: String },
}
struct Command {
    peer_code: String,
    body: String,
}
pub struct Handle {
    commands: SyncSender<Command>,
    events: Receiver<Event>,
    stop: Arc<AtomicBool>,
    connected: Arc<AtomicBool>,
}
impl Handle {
    pub fn spawn(config: Config) -> Result<Self, String> {
        websocket::Url::parse(&config.url)?;
        if crypto::unhex::<32>(&config.realm).is_err()
            || config.name.is_empty()
            || config.name.len() > 16
            || !config
                .name
                .bytes()
                .all(|b| b.is_ascii_alphanumeric() || b == b'_')
        {
            return Err("invalid realm/name".into());
        }
        if config
            .access_token
            .as_ref()
            .is_some_and(|s| s.len() > 512 || s.chars().any(char::is_control))
        {
            return Err("invalid access token".into());
        }
        let (command_tx, command_rx) = mpsc::sync_channel(16);
        let (event_tx, event_rx) = mpsc::sync_channel(128);
        let stop = Arc::new(AtomicBool::new(false));
        let stopped = stop.clone();
        let connected = Arc::new(AtomicBool::new(false));
        let online = connected.clone();
        thread::Builder::new()
            .name("claim-relay".into())
            .spawn(move || worker(config, command_rx, event_tx, stopped, online))
            .map_err(|e| e.to_string())?;
        Ok(Self {
            commands: command_tx,
            events: event_rx,
            stop,
            connected,
        })
    }
    pub fn try_event(&self) -> Option<Event> {
        self.events.try_recv().ok()
    }
    pub fn send(&self, peer_code: &str, body: &str) -> Result<(), String> {
        crypto::unhex::<12>(peer_code)?;
        if body.is_empty() || body.len() > 1024 {
            return Err("message must contain 1..1024 UTF-8 bytes".into());
        }
        if !self.connected.load(Ordering::Acquire) {
            return Err("relay is disconnected".into());
        }
        self.commands
            .try_send(Command {
                peer_code: peer_code.to_owned(),
                body: body.to_owned(),
            })
            .map_err(|e| format!("relay queue unavailable: {e}"))
    }
}
impl Drop for Handle {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Release);
    }
}
fn emit(tx: &SyncSender<Event>, event: Event) -> Result<(), String> {
    tx.try_send(event)
        .map_err(|_| "event queue full or closed".into())
}
fn worker(
    config: Config,
    commands: Receiver<Command>,
    events: SyncSender<Event>,
    stop: Arc<AtomicBool>,
    online: Arc<AtomicBool>,
) {
    let identity = match Identity::load(&config.identity_path) {
        Ok(i) => i,
        Err(reason) => {
            let _ = emit(&events, Event::Disconnected { reason });
            return;
        }
    };
    let mut backoff = 1;
    while !stop.load(Ordering::Acquire) {
        let result = session(&config, &identity, &commands, &events, &stop, &online);
        online.store(false, Ordering::Release);
        // Pending application data is never sent across reconnects without user retry.
        while commands.try_recv().is_ok() {}
        if stop.load(Ordering::Acquire) {
            break;
        }
        let reason = result.err().unwrap_or_else(|| "relay disconnected".into());
        if emit(&events, Event::Disconnected { reason }).is_err() {
            break;
        }
        for _ in 0..backoff * 10 {
            if stop.load(Ordering::Acquire) {
                return;
            }
            thread::sleep(Duration::from_millis(100));
        }
        backoff = (backoff * 2).min(30);
    }
}
fn session(
    config: &Config,
    identity: &Identity,
    commands: &Receiver<Command>,
    events: &SyncSender<Event>,
    stop: &AtomicBool,
    online: &AtomicBool,
) -> Result<(), String> {
    let url = websocket::Url::parse(&config.url)?;
    let mut ws = websocket::WebSocket::connect(&url, config.access_token.as_deref())?;
    let deadline = std::time::Instant::now() + Duration::from_secs(10);
    let challenge = loop {
        if stop.load(Ordering::Acquire) {
            return Ok(());
        }
        if std::time::Instant::now() > deadline {
            return Err("relay authentication timeout".into());
        }
        if let Some(text) = ws.poll()? {
            break serde_json::from_str::<Value>(&text).map_err(|_| "invalid relay challenge")?;
        }
    };
    if challenge.as_array().is_none_or(|a| a.len() != 2) || challenge[0] != "challenge" {
        return Err("invalid relay challenge".into());
    }
    let nonce = crypto::unhex::<32>(string(&challenge[1])?)?;
    let cert = identity.certificate(config);
    let cert_text = string(&cert["data"])?;
    let mut auth = b"DHRelay-auth-v1\0".to_vec();
    auth.extend_from_slice(&nonce);
    auth.extend_from_slice(cert_text.as_bytes());
    ws.send(&json!(["auth", cert_text, cert["sig"], identity.sign(&auth)]).to_string())?;
    let mut peers = HashMap::<String, Verified>::new();
    let mut replay = Replay::default();
    let mut authenticated = false;
    let mut next_send = std::time::Instant::now();
    while !stop.load(Ordering::Acquire) {
        if !authenticated && std::time::Instant::now() > deadline {
            return Err("relay authentication timeout".into());
        }
        if authenticated && std::time::Instant::now() >= next_send {
            match commands.try_recv() {
                Ok(command) => {
                    next_send = std::time::Instant::now() + Duration::from_millis(125);
                    if let Some(peer) = peers.get(&command.peer_code) {
                        let envelope = crypto::seal(identity, &config.realm, peer, &command.body)?;
                        ws.send(
                            &json!(["box", command.peer_code, envelope.to_string()]).to_string(),
                        )?;
                    } else {
                        emit(
                            events,
                            Event::Notice {
                                message: "recipient is offline".into(),
                            },
                        )?;
                    }
                }
                Err(TryRecvError::Empty) => {}
                Err(TryRecvError::Disconnected) => return Ok(()),
            }
        }
        let Some(text) = ws.poll()? else { continue };
        let value: Value = serde_json::from_str(&text).map_err(|_| "invalid relay JSON")?;
        let fields = value.as_array().ok_or("expected relay array")?;
        match fields
            .first()
            .and_then(Value::as_str)
            .ok_or("missing relay type")?
        {
            "ready" if fields.len() == 2 => {
                if authenticated || value[1] != identity.peer(config).code {
                    return Err("invalid ready identity".into());
                }
                authenticated = true;
                online.store(true, Ordering::Release);
                emit(
                    events,
                    Event::Connected {
                        code: identity.peer(config).code,
                    },
                )?;
            }
            "peer" if authenticated && fields.len() == 3 => {
                let peer =
                    crypto::certificate(&json!({"data":value[1],"sig":value[2]}), &config.realm)?;
                if peer.peer.code == identity.peer(config).code || peers.len() >= 64 {
                    return Err("invalid peer directory".into());
                }
                peers.insert(peer.peer.code.clone(), peer);
                emit_peers(events, &peers)?;
            }
            "gone" if authenticated && fields.len() == 2 => {
                peers.remove(string(&value[1])?);
                emit_peers(events, &peers)?;
            }
            "box" if authenticated && fields.len() == 3 => {
                let sender = string(&value[1])?;
                let opaque = string(&value[2])?;
                if opaque.len() > 8192 {
                    return Err("opaque message too large".into());
                }
                let Some(peer) = peers.get(sender) else {
                    continue;
                };
                // Invalid peer content cannot terminate the connection or flood UI notices.
                let Ok(envelope) = serde_json::from_str::<Value>(opaque) else {
                    continue;
                };
                if let Ok((nonce, body)) = crypto::open(identity, &config.realm, peer, &envelope)
                    && replay.insert(format!("{sender}:{nonce}"))
                {
                    emit(
                        events,
                        Event::Message {
                            peer: peer.peer.clone(),
                            body,
                        },
                    )?;
                }
            }
            "error" if fields.len() == 2 => {
                if !authenticated {
                    return Err("relay authentication rejected".into());
                }
                emit(
                    events,
                    Event::Notice {
                        message: "relay rejected request or recipient offline".into(),
                    },
                )?;
            }
            _ => return Err("unexpected relay message".into()),
        }
    }
    Ok(())
}
fn emit_peers(events: &SyncSender<Event>, peers: &HashMap<String, Verified>) -> Result<(), String> {
    let mut list: Vec<_> = peers.values().map(|p| p.peer.clone()).collect();
    list.sort_by(|a, b| a.code.cmp(&b.code));
    emit(events, Event::Peers(list))
}
/// Realm identifier derived from the normalized Minecraft server address.
pub fn realm_id(server: &str) -> String {
    use sha2::{Digest, Sha256};
    let normalized = server.trim().to_ascii_lowercase();
    let normalized = normalized.strip_suffix(":25565").unwrap_or(&normalized);
    crypto::hex(&Sha256::digest(normalized.as_bytes()))
}
#[derive(Default)]
struct Replay {
    order: VecDeque<String>,
    seen: HashSet<String>,
}
impl Replay {
    fn insert(&mut self, key: String) -> bool {
        if self.seen.contains(&key) {
            return false;
        }
        self.seen.insert(key.clone());
        self.order.push_back(key);
        if self.order.len() > 1024
            && let Some(old) = self.order.pop_front()
        {
            self.seen.remove(&old);
        }
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn identity() -> Identity {
        let path = std::env::temp_dir().join(format!(
            "claim-identity-{}",
            crypto::hex(&crypto::random::<16>().unwrap())
        ));
        let identity = Identity::load(&path).unwrap();
        std::fs::remove_file(path).unwrap();
        identity
    }
    fn config() -> Config {
        Config {
            url: "ws://127.0.0.1:8765/".into(),
            realm: realm_id("test"),
            uuid: 1,
            name: "Alice".into(),
            identity_path: PathBuf::new(),
            access_token: None,
        }
    }
    #[test]
    fn certificate_and_encryption_authenticate_every_field() {
        let a = identity();
        let b = identity();
        let c = config();
        let ap = crypto::certificate(&a.certificate(&c), &c.realm).unwrap();
        let bp = crypto::certificate(&b.certificate(&c), &c.realm).unwrap();
        assert_eq!(ap.peer.code, a.peer(&c).code);
        let msg = crypto::seal(&a, &c.realm, &bp, "hello").unwrap();
        assert_eq!(crypto::open(&b, &c.realm, &ap, &msg).unwrap().1, "hello");
        assert!(crypto::open(&a, &c.realm, &ap, &msg).is_err());
        assert!(crypto::open(&b, "other", &ap, &msg).is_err());
        for i in 0..8 {
            let mut tamper = msg.clone();
            tamper[i] = json!("tampered");
            assert!(crypto::open(&b, &c.realm, &ap, &tamper).is_err())
        }
        let mut cert = a.certificate(&c);
        let mut literal: Value = serde_json::from_str(cert["data"].as_str().unwrap()).unwrap();
        literal[5] = json!("Mallory");
        cert["data"] = json!(literal.to_string());
        assert!(crypto::certificate(&cert, &c.realm).is_err());
    }
    #[test]
    fn realm_normalization() {
        assert_eq!(realm_id(" EXAMPLE.org:25565 "), realm_id("example.org"));
        assert_eq!(realm_id("[::1]:25565"), realm_id("[::1]"));
        assert_ne!(realm_id("example.org:25566"), realm_id("example.org"));
    }
    #[test]
    fn replay_cache_is_bounded() {
        let mut replay = Replay::default();
        assert!(replay.insert("x".into()));
        assert!(!replay.insert("x".into()));
        for i in 0..2048 {
            assert!(replay.insert(i.to_string()));
        }
        assert_eq!(replay.seen.len(), 1024);
    }
}
