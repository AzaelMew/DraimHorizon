// Pure, bounded client-to-client friendship and telemetry protocol.
// Sender identity is supplied by the authenticated chat adapter, never JSON.
use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::io::{Read, Write};
use std::path::Path;
use std::sync::atomic::{AtomicU64, Ordering};

use serde_json::{Value, json};

pub const PREFIX: &str = "[DHP1] ";
pub const MAX_FRIENDS: usize = 4;
const MAX_PENDING: usize = 8;
const MAX_CLAIMS: usize = 64;
const MAX_BODY: usize = 180;
const LIVE_TTL: u64 = 20_000;
const INVITE_TTL: u64 = 120_000;
const ASSEMBLY_TTL: u64 = 300_000;
const FILE_LIMIT: u64 = 262_144;

#[derive(Clone, Debug, PartialEq)]
pub struct Friend {
    pub uuid: u128,
    pub name: String,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Snapshot {
    pub dimension: String,
    pub position: [f64; 3],
    pub health: f64,
    pub max_health: f64,
}

impl Snapshot {
    pub fn health_percent(&self) -> f64 {
        if !self.max_health.is_finite() || self.max_health <= 0.0 {
            return 0.0;
        }
        (self.health / self.max_health * 100.0).clamp(0.0, 100.0)
    }
}

#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct Claim {
    pub dimension: String,
    pub x: i32,
    pub z: i32,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Outbound {
    pub uuid: u128,
    pub name: String,
    pub body: String,
}

#[derive(Clone, Debug)]
pub struct OnlineFriend {
    pub uuid: u128,
    pub name: String,
    pub snapshot: Snapshot,
    pub health_percent: f64,
    pub claims: Option<Vec<Claim>>,
}

#[derive(Clone)]
struct Pending {
    name: String,
    nonce: u64,
    at: u64,
}

#[derive(Clone)]
struct Assembly {
    revision: u64,
    total: usize,
    pages: BTreeMap<usize, Claim>,
    at: u64,
}

#[derive(Clone, Default)]
struct Session {
    incoming_nonce: Option<u64>,
    outgoing_nonce: Option<u64>,
    next_seq: u64,
    last_snapshot_seq: u64,
    last_claim_seq: u64,
    snapshot: Option<(Snapshot, u64)>,
    claims: Vec<Claim>,
    claims_revision: Option<u64>,
    assembly: Option<Assembly>,
}

#[derive(Clone)]
struct Record {
    friend: Friend,
    bond: u64,
    session: Session,
}

#[derive(Clone)]
pub struct Party {
    scope: String,
    nonce_state: u64,
    friends: BTreeMap<u128, Record>,
    incoming: BTreeMap<u128, Pending>,
    outgoing: BTreeMap<u128, Pending>,
    handshakes: BTreeMap<u128, Pending>,
}

impl Party {
    pub fn new(scope: &str, nonce_seed: u64) -> Self {
        Self {
            scope: scope.to_owned(),
            nonce_state: nonce_seed,
            friends: BTreeMap::new(),
            incoming: BTreeMap::new(),
            outgoing: BTreeMap::new(),
            handshakes: BTreeMap::new(),
        }
    }

    pub fn awaiting(&mut self, uuid: u128, now: u64) -> bool {
        self.prune(now);
        self.incoming.contains_key(&uuid) || self.outgoing.contains_key(&uuid)
    }

    pub fn forget_session(&mut self, uuid: u128) {
        self.handshakes.remove(&uuid);
        if let Some(record) = self.friends.get_mut(&uuid) {
            record.session = Session::default();
        }
    }

    pub fn friends(&self) -> Vec<Friend> {
        self.friends.values().map(|r| r.friend.clone()).collect()
    }

    pub fn pending(&mut self, now: u64) -> Vec<Friend> {
        self.invitations(now)
    }

    pub fn peer_ready(&self, uuid: u128) -> bool {
        self.session_ready(uuid)
    }

    pub fn invitations(&mut self, now: u64) -> Vec<Friend> {
        self.prune(now);
        self.incoming
            .iter()
            .map(|(&uuid, p)| Friend {
                uuid,
                name: p.name.clone(),
            })
            .collect()
    }

    pub fn invite(&mut self, uuid: u128, name: &str, now: u64) -> Result<Vec<Outbound>, String> {
        valid_name(name)?;
        self.prune(now);
        if self.friends.contains_key(&uuid) {
            return Err("This player is already your friend.".into());
        }
        if self.friends.len() >= MAX_FRIENDS || self.pending_count() >= MAX_PENDING {
            return Err("Friend or invitation limit reached.".into());
        }
        let nonce = self.nonce();
        let out = outbound(uuid, name, json!(["i", nonce]))?;
        self.outgoing.insert(
            uuid,
            Pending {
                name: name.into(),
                nonce,
                at: now,
            },
        );
        Ok(vec![out])
    }

    pub fn accept(&mut self, uuid: u128, now: u64) -> Result<Vec<Outbound>, String> {
        self.prune(now);
        if self.friends.len() >= MAX_FRIENDS && !self.friends.contains_key(&uuid) {
            return Err("Friend limit reached.".into());
        }
        let pending = self
            .incoming
            .get(&uuid)
            .cloned()
            .ok_or("No current invitation from that player.")?;
        // Crossed invitations already advertise both session nonces. Reuse
        // ours so accepting on both clients establishes the same nonce pair.
        let crossing = self.outgoing.get(&uuid).map(|p| p.nonce);
        let nonce = crossing.unwrap_or_else(|| self.nonce());
        let reply = outbound(uuid, &pending.name, json!(["a", pending.nonce, nonce]))?;
        self.friends.insert(
            uuid,
            Record {
                friend: Friend {
                    uuid,
                    name: pending.name,
                },
                bond: crossing.map_or(pending.nonce, |own| own.min(pending.nonce)),
                session: Session {
                    incoming_nonce: Some(nonce),
                    outgoing_nonce: Some(pending.nonce),
                    ..Session::default()
                },
            },
        );
        self.incoming.remove(&uuid);
        // Keep a crossed invitation until its acceptance arrives. Its ack
        // must not reset the session's sequence counters.
        Ok(vec![reply])
    }

    pub fn remove(&mut self, uuid: u128) -> Result<Vec<Outbound>, String> {
        let record = self
            .friends
            .remove(&uuid)
            .ok_or("No friend with that identity.")?;
        self.incoming.remove(&uuid);
        self.outgoing.remove(&uuid);
        self.handshakes.remove(&uuid);
        Ok(vec![outbound(
            uuid,
            &record.friend.name,
            json!(["d", record.bond]),
        )?])
    }

    pub fn start_session(&mut self, uuid: u128, now: u64) -> Result<Vec<Outbound>, String> {
        self.prune(now);
        let name = self
            .friends
            .get(&uuid)
            .ok_or("Not an accepted friend.")?
            .friend
            .name
            .clone();
        let nonce = self.nonce();
        self.handshakes.insert(
            uuid,
            Pending {
                name: name.clone(),
                nonce,
                at: now,
            },
        );
        Ok(vec![outbound(uuid, &name, json!(["h", nonce]))?])
    }

    pub fn receive(
        &mut self,
        uuid: u128,
        name: &str,
        body: &str,
        now: u64,
    ) -> Result<Vec<Outbound>, String> {
        valid_name(name)?;
        self.prune(now);
        if body.len() > MAX_BODY || !body.is_ascii() {
            return Err("Invalid party message length or encoding.".into());
        }
        let raw = body.strip_prefix(PREFIX).ok_or("Not a party message.")?;
        let value: Value = serde_json::from_str(raw).map_err(|_| "Invalid party JSON.")?;
        let a = value.as_array().ok_or("Party frame must be an array.")?;
        let kind = a
            .first()
            .and_then(Value::as_str)
            .ok_or("Missing party frame kind.")?;
        match kind {
            "i" => {
                arity(a, 2)?;
                let nonce = nonzero(&a[1])?;
                if self.friends.get(&uuid).is_some_and(|r| r.bond == nonce) {
                    return Ok(Vec::new());
                }
                if (self.friends.len() >= MAX_FRIENDS && !self.friends.contains_key(&uuid))
                    || self.pending_count() >= MAX_PENDING
                {
                    return Err("Friend or invitation limit reached.".into());
                }
                self.incoming.insert(
                    uuid,
                    Pending {
                        name: name.into(),
                        nonce,
                        at: now,
                    },
                );
            }
            "a" => {
                arity(a, 3)?;
                let (invite, remote_nonce) = (nonzero(&a[1])?, nonzero(&a[2])?);
                let pending = self
                    .outgoing
                    .get(&uuid)
                    .filter(|p| p.nonce == invite)
                    .ok_or("No matching outgoing invitation.")?;
                if self.friends.len() >= MAX_FRIENDS && !self.friends.contains_key(&uuid) {
                    return Err("Friend limit reached.".into());
                }
                let incoming_nonce = pending.nonce;
                if let Some(record) = self.friends.get(&uuid) {
                    if record.bond != invite.min(remote_nonce) {
                        return Err("Acceptance does not match crossed invitations.".into());
                    }
                    self.outgoing.remove(&uuid);
                    self.incoming.remove(&uuid);
                    return Ok(Vec::new());
                }
                let bond = self
                    .incoming
                    .get(&uuid)
                    .map_or(invite, |p| invite.min(p.nonce));
                self.friends.insert(
                    uuid,
                    Record {
                        friend: Friend {
                            uuid,
                            name: name.into(),
                        },
                        bond,
                        session: Session {
                            incoming_nonce: Some(incoming_nonce),
                            outgoing_nonce: Some(remote_nonce),
                            ..Session::default()
                        },
                    },
                );
                self.outgoing.remove(&uuid);
                self.incoming.remove(&uuid);
            }
            "d" => {
                arity(a, 2)?;
                let bond = nonzero(&a[1])?;
                if self.friends.get(&uuid).is_none_or(|r| r.bond != bond) {
                    return Err("Removal does not match this friendship.".into());
                }
                self.friends.remove(&uuid);
                self.handshakes.remove(&uuid);
                self.incoming.remove(&uuid);
                self.outgoing.remove(&uuid);
            }
            "h" => {
                arity(a, 2)?;
                let remote_nonce = nonzero(&a[1])?;
                if !self.friends.contains_key(&uuid) {
                    return Err("Session from an unaccepted player.".into());
                }
                // Simultaneous hellos must use the already advertised local
                // nonce; otherwise crossed replies establish incompatible pairs.
                let incoming_nonce = match self.handshakes.get(&uuid) {
                    Some(pending) => pending.nonce,
                    None => self.nonce(),
                };
                let record = self.friends.get_mut(&uuid).ok_or("Missing friend.")?;
                record.friend.name = name.into();
                record.session = Session {
                    incoming_nonce: Some(incoming_nonce),
                    outgoing_nonce: Some(remote_nonce),
                    ..Session::default()
                };
                return Ok(vec![outbound(
                    uuid,
                    name,
                    json!(["r", remote_nonce, incoming_nonce]),
                )?]);
            }
            "r" => {
                arity(a, 3)?;
                let (hello, remote_nonce) = (nonzero(&a[1])?, nonzero(&a[2])?);
                let pending = self
                    .handshakes
                    .get(&uuid)
                    .filter(|p| p.nonce == hello)
                    .ok_or("No matching session request.")?;
                let incoming_nonce = pending.nonce;
                let record = self
                    .friends
                    .get_mut(&uuid)
                    .ok_or("Session reply from unaccepted player.")?;
                record.friend.name = name.into();
                record.session = Session {
                    incoming_nonce: Some(incoming_nonce),
                    outgoing_nonce: Some(remote_nonce),
                    ..Session::default()
                };
                self.handshakes.remove(&uuid);
            }
            "s" => {
                arity(a, 10)?;
                let (nonce, seq) = (nonzero(&a[1])?, nonzero(&a[2])?);
                let snapshot = checked_snapshot(Snapshot {
                    dimension: decode_dimension(&a[3])?,
                    position: [number(&a[4])?, number(&a[5])?, number(&a[6])?],
                    health: number(&a[7])?,
                    max_health: number(&a[8])?,
                })?;
                if a[9].as_u64() != Some(1) {
                    return Err("Unknown snapshot schema.".into());
                }
                let record = self.checked_session(uuid, nonce, seq, false)?;
                record.friend.name = name.into();
                record.session.last_snapshot_seq = seq;
                record.session.snapshot = Some((snapshot, now));
            }
            "c" => {
                if a.len() != 6 && a.len() != 9 {
                    return Err("Invalid claims frame length.".into());
                }
                let (nonce, seq) = (nonzero(&a[1])?, nonzero(&a[2])?);
                let revision = a[3].as_u64().ok_or("Invalid claims revision.")?;
                let index = a[4].as_u64().ok_or("Invalid claims page index.")? as usize;
                let total = a[5]
                    .as_u64()
                    .filter(|n| *n <= MAX_CLAIMS as u64)
                    .ok_or("Too many claims pages.")? as usize;
                let claim = if total == 0 {
                    if a.len() != 6 || index != 0 {
                        return Err("Invalid empty claims set.".into());
                    }
                    None
                } else {
                    if a.len() != 9 || index >= total {
                        return Err("Invalid claims page index.".into());
                    }
                    Some(checked_claim(Claim {
                        dimension: decode_dimension(&a[6])?,
                        x: origin(&a[7])?,
                        z: origin(&a[8])?,
                    })?)
                };
                let record = self.checked_session(uuid, nonce, seq, true)?;
                let session = &mut record.session;
                if session.claims_revision.is_some_and(|r| revision <= r) {
                    return Err("Stale claims revision.".into());
                }
                if let Some(old) = &session.assembly {
                    if revision < old.revision {
                        return Err("Stale partial claims revision.".into());
                    }
                    if revision == old.revision && total != old.total {
                        return Err("Claims total changed mid-transfer.".into());
                    }
                }
                if total == 0 {
                    session.claims.clear();
                    session.assembly = None;
                    session.claims_revision = Some(revision);
                } else {
                    if session
                        .assembly
                        .as_ref()
                        .is_none_or(|old| old.revision != revision)
                    {
                        session.assembly = Some(Assembly {
                            revision,
                            total,
                            pages: BTreeMap::new(),
                            at: now,
                        });
                    }
                    let assembly = session
                        .assembly
                        .as_mut()
                        .ok_or("Missing claims assembly.")?;
                    let claim = claim.ok_or("Missing claims data.")?;
                    if assembly.pages.get(&index).is_some_and(|old| *old != claim) {
                        return Err("Conflicting claims page.".into());
                    }
                    assembly.pages.insert(index, claim);
                    if assembly.pages.len() == total {
                        let claims: BTreeSet<_> = assembly.pages.values().cloned().collect();
                        if claims.len() != total {
                            session.assembly = None;
                            return Err("Duplicate claim origins.".into());
                        }
                        session.claims = claims.into_iter().collect();
                        session.claims_revision = Some(revision);
                        session.assembly = None;
                    }
                }
                session.last_claim_seq = seq;
                record.friend.name = name.into();
            }
            _ => return Err("Unknown party frame.".into()),
        }
        Ok(Vec::new())
    }

    pub fn snapshot_for(&mut self, uuid: u128, snapshot: Snapshot) -> Result<Outbound, String> {
        let snapshot = checked_snapshot(snapshot)?;
        let record = self
            .friends
            .get_mut(&uuid)
            .ok_or("Not an accepted friend.")?;
        let nonce = record
            .session
            .outgoing_nonce
            .ok_or("Friend session is not ready.")?;
        let seq = record
            .session
            .next_seq
            .checked_add(1)
            .ok_or("Party sequence exhausted.")?;
        let p = snapshot.position.map(round);
        let out = outbound(
            uuid,
            &record.friend.name,
            json!([
                "s",
                nonce,
                seq,
                encode_dimension(&snapshot.dimension),
                p[0],
                p[1],
                p[2],
                snapshot.health,
                snapshot.max_health,
                1
            ]),
        )?;
        record.session.next_seq = seq;
        Ok(out)
    }

    pub fn claims_for(
        &mut self,
        uuid: u128,
        revision: u64,
        claims: &[Claim],
    ) -> Result<Vec<Outbound>, String> {
        if claims.len() > MAX_CLAIMS {
            return Err("Too many shared claims.".into());
        }
        let mut seen = BTreeSet::new();
        for claim in claims {
            if !seen.insert(checked_claim(claim.clone())?) {
                return Err("Duplicate claim origins.".into());
            }
        }
        let record = self
            .friends
            .get_mut(&uuid)
            .ok_or("Not an accepted friend.")?;
        let nonce = record
            .session
            .outgoing_nonce
            .ok_or("Friend session is not ready.")?;
        let mut seq = record.session.next_seq;
        let mut messages = Vec::new();
        if claims.is_empty() {
            seq = seq.checked_add(1).ok_or("Party sequence exhausted.")?;
            messages.push(outbound(
                uuid,
                &record.friend.name,
                json!(["c", nonce, seq, revision, 0, 0]),
            )?);
        } else {
            for (index, claim) in seen.iter().enumerate() {
                seq = seq.checked_add(1).ok_or("Party sequence exhausted.")?;
                messages.push(outbound(
                    uuid,
                    &record.friend.name,
                    json!([
                        "c",
                        nonce,
                        seq,
                        revision,
                        index,
                        seen.len(),
                        encode_dimension(&claim.dimension),
                        claim.x,
                        claim.z
                    ]),
                )?);
            }
        }
        record.session.next_seq = seq;
        Ok(messages)
    }

    pub fn online(&self, tab_uuids: &[u128], now: u64) -> Vec<OnlineFriend> {
        self.friends
            .values()
            .filter(|r| tab_uuids.contains(&r.friend.uuid))
            .filter_map(|r| {
                let (snapshot, at) = r.session.snapshot.as_ref()?;
                if now < *at || now - at > LIVE_TTL {
                    return None;
                }
                Some(OnlineFriend {
                    uuid: r.friend.uuid,
                    name: r.friend.name.clone(),
                    snapshot: snapshot.clone(),
                    health_percent: snapshot.health_percent(),
                    claims: r.session.claims_revision.map(|_| r.session.claims.clone()),
                })
            })
            .collect()
    }

    #[cfg(test)]
    pub fn shared_claims(&self, uuid: u128, tab_uuids: &[u128], now: u64) -> Vec<Claim> {
        if self.online(tab_uuids, now).iter().any(|p| p.uuid == uuid) {
            self.friends
                .get(&uuid)
                .map(|r| r.session.claims.clone())
                .unwrap_or_default()
        } else {
            Vec::new()
        }
    }

    pub fn session_ready(&self, uuid: u128) -> bool {
        self.friends
            .get(&uuid)
            .is_some_and(|r| r.session.outgoing_nonce.is_some())
    }

    pub fn disconnect(&mut self) {
        self.incoming.clear();
        self.outgoing.clear();
        self.handshakes.clear();
        for record in self.friends.values_mut() {
            record.session = Session::default();
        }
    }

    fn checked_session(
        &mut self,
        uuid: u128,
        nonce: u64,
        seq: u64,
        claims: bool,
    ) -> Result<&mut Record, String> {
        let record = self
            .friends
            .get_mut(&uuid)
            .ok_or("Telemetry from an unaccepted player.")?;
        if record.session.incoming_nonce != Some(nonce)
            || seq
                <= if claims {
                    record.session.last_claim_seq
                } else {
                    record.session.last_snapshot_seq
                }
        {
            return Err("Stale party session or sequence.".into());
        }
        Ok(record)
    }

    fn pending_count(&self) -> usize {
        self.incoming.len() + self.outgoing.len()
    }

    fn prune(&mut self, now: u64) {
        for map in [&mut self.incoming, &mut self.outgoing, &mut self.handshakes] {
            map.retain(|_, p| now >= p.at && now - p.at <= INVITE_TTL);
        }
        for record in self.friends.values_mut() {
            if record
                .session
                .assembly
                .as_ref()
                .is_some_and(|a| now < a.at || now - a.at > ASSEMBLY_TTL)
            {
                record.session.assembly = None;
            }
        }
    }

    fn nonce(&mut self) -> u64 {
        // SplitMix64: freshness derives from the adapter-provided per-connection
        // random seed. This protocol does not claim cryptographic secrecy.
        self.nonce_state = self.nonce_state.wrapping_add(0x9E3779B97F4A7C15);
        let mut value = self.nonce_state;
        value = (value ^ (value >> 30)).wrapping_mul(0xBF58476D1CE4E5B9);
        value = (value ^ (value >> 27)).wrapping_mul(0x94D049BB133111EB);
        (value ^ (value >> 31)).max(1)
    }

    pub fn load(path: &Path, scope: &str, nonce_seed: u64) -> Result<Self, String> {
        valid_scope(scope)?;
        let database = read_database(path)?;
        let mut party = Self::new(scope, nonce_seed);
        let Some(entries) = database["scopes"].get(scope) else {
            return Ok(party);
        };
        let entries = entries
            .as_array()
            .filter(|a| a.len() <= MAX_FRIENDS)
            .ok_or("Invalid saved friends list.")?;
        for value in entries {
            let a = value
                .as_array()
                .filter(|a| a.len() == 3)
                .ok_or("Invalid saved friendship.")?;
            let uuid = a[0]
                .as_str()
                .filter(|s| s.len() == 32)
                .and_then(|s| u128::from_str_radix(s, 16).ok())
                .ok_or("Invalid saved friend UUID.")?;
            let name = a[1].as_str().ok_or("Invalid saved friend name.")?;
            valid_name(name)?;
            let bond = nonzero(&a[2])?;
            if party
                .friends
                .insert(
                    uuid,
                    Record {
                        friend: Friend {
                            uuid,
                            name: name.into(),
                        },
                        bond,
                        session: Session::default(),
                    },
                )
                .is_some()
            {
                return Err("Duplicate saved friend UUID.".into());
            }
        }
        Ok(party)
    }

    pub fn save(&self, path: &Path) -> Result<(), String> {
        valid_scope(&self.scope)?;
        let mut database = read_database(path)?;
        let scopes = database["scopes"]
            .as_object_mut()
            .ok_or("Invalid party scopes.")?;
        if !scopes.contains_key(&self.scope) && scopes.len() >= 128 {
            return Err("Too many saved party scopes.".into());
        }
        let entries: Vec<Value> = self
            .friends
            .values()
            .map(|r| json!([format!("{:032x}", r.friend.uuid), r.friend.name, r.bond]))
            .collect();
        scopes.insert(self.scope.clone(), Value::Array(entries));
        let bytes =
            serde_json::to_vec_pretty(&database).map_err(|_| "Cannot encode friendships.")?;
        if bytes.len() as u64 > FILE_LIMIT {
            return Err("Party file is too large.".into());
        }
        let parent = path
            .parent()
            .filter(|p| !p.as_os_str().is_empty())
            .unwrap_or(Path::new("."));
        fs::create_dir_all(parent).map_err(io)?;
        static SERIAL: AtomicU64 = AtomicU64::new(0);
        let filename = path.file_name().ok_or("Party path needs a filename.")?;
        let temporary = parent.join(format!(
            ".{}.{}.{}.tmp",
            filename.to_string_lossy(),
            std::process::id(),
            SERIAL.fetch_add(1, Ordering::Relaxed)
        ));
        let mut file = fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&temporary)
            .map_err(io)?;
        let result = (|| {
            file.write_all(&bytes).map_err(io)?;
            file.sync_all().map_err(io)?;
            drop(file);
            fs::rename(&temporary, path).map_err(io)
        })();
        if result.is_err() {
            let _ = fs::remove_file(temporary);
        }
        result
    }
}

fn io(error: std::io::Error) -> String {
    format!("Party storage error: {error}")
}
fn valid_scope(scope: &str) -> Result<(), String> {
    if scope.is_empty() || scope.len() > 384 || scope.chars().any(char::is_control) {
        return Err("Invalid server/account scope.".into());
    }
    Ok(())
}
fn valid_name(name: &str) -> Result<(), String> {
    if name.is_empty()
        || name.len() > 16
        || !name.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'_')
    {
        return Err("Player name must contain 1-16 ASCII letters, digits or underscores.".into());
    }
    Ok(())
}
fn arity(a: &[Value], length: usize) -> Result<(), String> {
    if a.len() == length {
        Ok(())
    } else {
        Err("Invalid party frame length.".into())
    }
}
fn nonzero(v: &Value) -> Result<u64, String> {
    v.as_u64()
        .filter(|n| *n > 0)
        .ok_or("Invalid party nonce or sequence.".into())
}
fn number(v: &Value) -> Result<f64, String> {
    v.as_f64()
        .filter(|n| n.is_finite())
        .ok_or("Invalid party number.".into())
}
fn dimension(value: &str) -> Result<String, String> {
    let value = if value.contains(':') {
        value.to_owned()
    } else {
        format!("minecraft:{value}")
    };
    let (namespace, path) = value.split_once(':').ok_or("Invalid dimension.")?;
    if value.len() > 64
        || namespace.is_empty()
        || path.is_empty()
        || !namespace
            .bytes()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || b"_.-".contains(&c))
        || !path
            .bytes()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || b"_.-/".contains(&c))
    {
        return Err("Invalid dimension.".into());
    }
    Ok(value)
}
fn encode_dimension(value: &str) -> Value {
    match value {
        "minecraft:overworld" => json!(0),
        "minecraft:the_nether" => json!(1),
        "minecraft:the_end" => json!(2),
        _ => json!(value),
    }
}
fn decode_dimension(value: &Value) -> Result<String, String> {
    match value.as_u64() {
        Some(0) => Ok("minecraft:overworld".into()),
        Some(1) => Ok("minecraft:the_nether".into()),
        Some(2) => Ok("minecraft:the_end".into()),
        Some(_) => Err("Unknown dimension code.".into()),
        None => dimension(value.as_str().ok_or("Invalid dimension.")?),
    }
}
fn checked_snapshot(mut snapshot: Snapshot) -> Result<Snapshot, String> {
    snapshot.dimension = dimension(&snapshot.dimension)?;
    if !snapshot.position.iter().all(|n| n.is_finite())
        || snapshot.position[0].abs() > 30_000_000.0
        || snapshot.position[2].abs() > 30_000_000.0
        || snapshot.position[1].abs() > 2048.0
        || !snapshot.health.is_finite()
        || !snapshot.max_health.is_finite()
        || snapshot.max_health <= 0.0
        || snapshot.max_health > 1_000_000.0
    {
        return Err("Invalid player snapshot.".into());
    }
    snapshot.health = snapshot.health.clamp(0.0, snapshot.max_health);
    Ok(snapshot)
}
fn checked_claim(mut claim: Claim) -> Result<Claim, String> {
    claim.dimension = dimension(&claim.dimension)?;
    if claim.x.rem_euclid(16) != 0
        || claim.z.rem_euclid(16) != 0
        || !(-30_000_000..=29_999_984).contains(&claim.x)
        || !(-30_000_000..=29_999_984).contains(&claim.z)
    {
        return Err("Invalid claimed chunk origin.".into());
    }
    Ok(claim)
}
fn origin(value: &Value) -> Result<i32, String> {
    value
        .as_i64()
        .and_then(|n| i32::try_from(n).ok())
        .ok_or("Invalid claim coordinate.".into())
}
fn round(value: f64) -> f64 {
    (value * 100.0).round() / 100.0
}
fn outbound(uuid: u128, name: &str, value: Value) -> Result<Outbound, String> {
    valid_name(name)?;
    let body = format!(
        "{PREFIX}{}",
        serde_json::to_string(&value).map_err(|_| "Cannot encode party frame.")?
    );
    if body.len() > MAX_BODY || !body.is_ascii() {
        return Err("Party frame exceeds private-message limit.".into());
    }
    Ok(Outbound {
        uuid,
        name: name.into(),
        body,
    })
}
fn read_database(path: &Path) -> Result<Value, String> {
    let metadata = match fs::symlink_metadata(path) {
        Ok(m) => m,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
            return Ok(json!({"schema":1,"scopes":{}}));
        }
        Err(e) => return Err(io(e)),
    };
    if !metadata.is_file() || metadata.len() > FILE_LIMIT {
        return Err("Invalid or oversized party file.".into());
    }
    let mut bytes = Vec::new();
    fs::File::open(path)
        .map_err(io)?
        .take(FILE_LIMIT + 1)
        .read_to_end(&mut bytes)
        .map_err(io)?;
    if bytes.len() as u64 > FILE_LIMIT {
        return Err("Party file is too large.".into());
    }
    let value: Value = serde_json::from_slice(&bytes)
        .map_err(|_| "Malformed party file; refusing to overwrite it.")?;
    let object = value.as_object().ok_or("Invalid party file.")?;
    if object.len() != 2
        || value["schema"].as_u64() != Some(1)
        || !value["scopes"].is_object()
        || value["scopes"].as_object().is_none_or(|s| s.len() > 128)
    {
        return Err("Unknown party storage schema.".into());
    }
    // Validate all scopes so save cannot preserve a corrupt record unnoticed.
    for (scope, entries) in value["scopes"].as_object().ok_or("Invalid party scopes.")? {
        valid_scope(scope)?;
        let entries = entries
            .as_array()
            .filter(|a| a.len() <= MAX_FRIENDS)
            .ok_or("Invalid saved friends list.")?;
        let mut ids = BTreeSet::new();
        for entry in entries {
            let a = entry
                .as_array()
                .filter(|a| a.len() == 3)
                .ok_or("Invalid saved friend.")?;
            let id = a[0]
                .as_str()
                .filter(|s| s.len() == 32)
                .and_then(|s| u128::from_str_radix(s, 16).ok())
                .ok_or("Invalid friend UUID.")?;
            if !ids.insert(id) {
                return Err("Duplicate saved friend.".into());
            }
            valid_name(a[1].as_str().ok_or("Invalid friend name.")?)?;
            nonzero(&a[2])?;
        }
    }
    Ok(value)
}

#[cfg(test)]
mod tests {
    use super::*;
    const A: u128 = 11;
    const B: u128 = 22;
    const C: u128 = 33;

    fn parties() -> (Party, Party) {
        let mut a = Party::new("server|Alice", 101);
        let mut b = Party::new("server|Bob", 202);
        let invite = a.invite(B, "Bob", 0).unwrap().remove(0);
        b.receive(A, "Alice", &invite.body, 1).unwrap();
        let accept = b.accept(A, 2).unwrap().remove(0);
        a.receive(B, "Bob", &accept.body, 3).unwrap();
        (a, b)
    }

    fn snapshot() -> Snapshot {
        Snapshot {
            dimension: "minecraft:overworld".into(),
            position: [123.5, 64.0, -987.0],
            health: 15.0,
            max_health: 20.0,
        }
    }

    #[test]
    fn friendship_requires_explicit_matching_mutual_handshake() {
        let mut a = Party::new("server|Alice", 1);
        let mut b = Party::new("server|Bob", 2);
        assert!(a.accept(B, 0).is_err());
        let invite = a.invite(B, "Bob", 0).unwrap().remove(0);
        b.receive(A, "Alice", &invite.body, 1).unwrap();
        assert!(b.friends().is_empty());
        assert_eq!(b.pending(1)[0].uuid, A);
        assert!(b.snapshot_for(A, snapshot()).is_err());
        let ack = b.accept(A, 2).unwrap().remove(0);
        assert!(a.receive(C, "Mallory", &ack.body, 3).is_err());
        a.receive(B, "Bob", &ack.body, 3).unwrap();
        assert_eq!(a.friends()[0].uuid, B);
        assert_eq!(b.friends()[0].uuid, A);
        assert!(a.receive(B, "Bob", &ack.body, 4).is_err());
    }

    #[test]
    fn crossed_invitations_and_acceptances_keep_matching_sessions() {
        for both_accept in [false, true] {
            let mut a = Party::new("server|Alice", 101);
            let mut b = Party::new("server|Bob", 202);
            let ai = a.invite(B, "Bob", 0).unwrap().remove(0);
            let bi = b.invite(A, "Alice", 0).unwrap().remove(0);
            a.receive(B, "Bob", &bi.body, 1).unwrap();
            b.receive(A, "Alice", &ai.body, 1).unwrap();
            assert!(a.friends().is_empty());
            assert!(b.friends().is_empty());
            let aa = a.accept(B, 2).unwrap().remove(0);
            let ba = both_accept.then(|| b.accept(A, 2).unwrap().remove(0));
            // Telemetry may arrive before the crossed acceptance. Its replay
            // must remain rejected after that acceptance has been consumed.
            let early = b.snapshot_for(A, snapshot());
            if let Ok(frame) = &early {
                a.receive(B, "Bob", &frame.body, 3).unwrap();
            }
            b.receive(A, "Alice", &aa.body, 4).unwrap();
            if let Some(ba) = ba {
                a.receive(B, "Bob", &ba.body, 4).unwrap();
                assert!(a.receive(B, "Bob", &ba.body, 5).is_err());
            }
            if let Ok(frame) = early {
                assert!(a.receive(B, "Bob", &frame.body, 5).is_err());
            }
            let af = a.snapshot_for(B, snapshot()).unwrap();
            let bf = b.snapshot_for(A, snapshot()).unwrap();
            a.receive(B, "Bob", &bf.body, 6).unwrap();
            b.receive(A, "Alice", &af.body, 6).unwrap();
            assert_eq!(a.online(&[B], 6).len(), 1);
            assert_eq!(b.online(&[A], 6).len(), 1);
            let removal = a.remove(B).unwrap().remove(0);
            b.receive(A, "Alice", &removal.body, 7).unwrap();
            assert!(b.friends().is_empty());
        }
    }

    #[test]
    fn snapshots_require_friend_session_sequence_and_tab_presence() {
        let (mut a, mut b) = parties();
        let out = a.snapshot_for(B, snapshot()).unwrap();
        assert!(b.receive(C, "Mallory", &out.body, 4).is_err());
        b.receive(A, "Alice", &out.body, 4).unwrap();
        assert_eq!(b.online(&[A], 4)[0].health_percent, 75.0);
        assert!(b.online(&[], 4).is_empty());
        assert!(b.online(&[A], 20_005).is_empty());
        assert!(b.receive(A, "Alice", &out.body, 5).is_err());
        let mut malformed: Value =
            serde_json::from_str(out.body.strip_prefix(PREFIX).unwrap()).unwrap();
        malformed[1] = json!(999);
        malformed[2] = json!(1000);
        assert!(
            b.receive(A, "Alice", &format!("{PREFIX}{malformed}"), 5)
                .is_err()
        );
    }

    #[test]
    fn disconnect_keeps_friendship_but_invalidates_old_session() {
        let (mut a, mut b) = parties();
        let old = a.snapshot_for(B, snapshot()).unwrap();
        b.receive(A, "Alice", &old.body, 5).unwrap();
        b.disconnect();
        a.disconnect();
        assert_eq!(b.friends().len(), 1);
        assert!(b.online(&[A], 6).is_empty());
        assert!(b.receive(A, "Alice", &old.body, 6).is_err());
        let hello = a.start_session(B, 7).unwrap().remove(0);
        let reply = b.receive(A, "Alice", &hello.body, 8).unwrap().remove(0);
        a.receive(B, "Bob", &reply.body, 9).unwrap();
        let next = a.snapshot_for(B, snapshot()).unwrap();
        b.receive(A, "Alice", &next.body, 10).unwrap();
        assert_eq!(b.online(&[A], 10).len(), 1);
        assert!(b.receive(A, "Alice", &old.body, 11).is_err());
    }

    #[test]
    fn simultaneous_session_hellos_establish_matching_nonces() {
        let (mut a, mut b) = parties();
        a.disconnect();
        b.disconnect();
        let ah = a.start_session(B, 10).unwrap().remove(0);
        let bh = b.start_session(A, 10).unwrap().remove(0);
        let ar = a.receive(B, "Bob", &bh.body, 11).unwrap().remove(0);
        let br = b.receive(A, "Alice", &ah.body, 11).unwrap().remove(0);
        a.receive(B, "Bob", &br.body, 12).unwrap();
        b.receive(A, "Alice", &ar.body, 12).unwrap();
        let af = a.snapshot_for(B, snapshot()).unwrap();
        let bf = b.snapshot_for(A, snapshot()).unwrap();
        a.receive(B, "Bob", &bf.body, 13).unwrap();
        b.receive(A, "Alice", &af.body, 13).unwrap();
        assert_eq!(a.online(&[B], 13).len(), 1);
        assert_eq!(b.online(&[A], 13).len(), 1);
    }

    #[test]
    fn shared_claims_commit_only_complete_valid_revision() {
        let (mut a, mut b) = parties();
        let status = a.snapshot_for(B, snapshot()).unwrap();
        b.receive(A, "Alice", &status.body, 4).unwrap();
        let claims = vec![
            Claim {
                dimension: "overworld".into(),
                x: -16,
                z: 0,
            },
            Claim {
                dimension: "overworld".into(),
                x: 0,
                z: 0,
            },
        ];
        let frames = a.claims_for(B, 1, &claims).unwrap();
        b.receive(A, "Alice", &frames[0].body, 5).unwrap();
        assert!(b.online(&[A], 5)[0].claims.is_none());
        b.receive(A, "Alice", &frames[1].body, 6).unwrap();
        assert_eq!(b.shared_claims(A, &[A], 6).len(), 2);
        let mut update = a
            .claims_for(
                B,
                2,
                &[Claim {
                    dimension: "overworld".into(),
                    x: 32,
                    z: 0,
                }],
            )
            .unwrap();
        b.receive(A, "Alice", &update.remove(0).body, 7).unwrap();
        assert_eq!(b.shared_claims(A, &[A], 7)[0].x, 32);
        let stale = a.claims_for(B, 1, &claims).unwrap();
        assert!(b.receive(A, "Alice", &stale[0].body, 8).is_err());
        let empty = a.claims_for(B, 3, &[]).unwrap().remove(0);
        b.receive(A, "Alice", &empty.body, 9).unwrap();
        assert!(b.online(&[A], 9)[0].claims.as_ref().unwrap().is_empty());
    }

    #[test]
    fn fresh_status_can_overtake_queued_claim_pages() {
        let (mut a, mut b) = parties();
        let claims = [
            Claim {
                dimension: "overworld".into(),
                x: 0,
                z: 0,
            },
            Claim {
                dimension: "overworld".into(),
                x: 16,
                z: 0,
            },
        ];
        let pages = a.claims_for(B, 1, &claims).unwrap();
        let status = a.snapshot_for(B, snapshot()).unwrap();
        // Higher global sequence is sent first by the status-priority queue.
        b.receive(A, "Alice", &status.body, 4).unwrap();
        b.receive(A, "Alice", &pages[0].body, 5).unwrap();
        b.receive(A, "Alice", &pages[1].body, 6).unwrap();
        assert_eq!(b.online(&[A], 6)[0].claims.as_ref().unwrap().len(), 2);
        assert!(b.receive(A, "Alice", &pages[0].body, 7).is_err());
        assert!(b.receive(A, "Alice", &status.body, 7).is_err());
    }

    #[test]
    fn rejected_payloads_cannot_invent_telemetry_or_claims() {
        let (mut a, mut b) = parties();
        for bad in [
            "[DHP1] {}",
            "[DHP1] [\"a\",1,2]",
            "[DHP1] [\"s\",1,1,0,0,0,0,20,0,1]",
        ] {
            assert!(b.receive(A, "Alice", bad, 5).is_err());
        }
        assert!(b.receive(A, "bad name", "[DHP1] [\"i\",1]", 5).is_err());
        assert!(b.receive(A, "Alice", &"x".repeat(MAX_BODY + 1), 5).is_err());
        assert!(
            a.snapshot_for(
                B,
                Snapshot {
                    health: f64::NAN,
                    ..snapshot()
                }
            )
            .is_err()
        );
        assert!(
            a.claims_for(
                B,
                1,
                &[Claim {
                    dimension: "overworld".into(),
                    x: -1,
                    z: 0
                }]
            )
            .is_err()
        );
        assert!(b.online(&[A], 5).is_empty());
    }

    #[test]
    fn removal_is_mutual_and_old_removal_cannot_delete_new_friendship() {
        let (mut a, mut b) = parties();
        let removal = a.remove(B).unwrap().remove(0);
        b.receive(A, "Alice", &removal.body, 5).unwrap();
        assert!(a.friends().is_empty());
        assert!(b.friends().is_empty());
        let invite = a.invite(B, "Bob", 6).unwrap().remove(0);
        b.receive(A, "Alice", &invite.body, 7).unwrap();
        let ack = b.accept(A, 8).unwrap().remove(0);
        a.receive(B, "Bob", &ack.body, 9).unwrap();
        assert!(b.receive(A, "Alice", &removal.body, 10).is_err());
        assert_eq!(b.friends().len(), 1);
    }

    #[test]
    fn reinvite_after_offline_removal_requires_new_acceptance() {
        let (mut a, mut b) = parties();
        a.remove(B).unwrap(); // Bob never received removal while offline.
        let invite = a.invite(B, "Bob", 5).unwrap().remove(0);
        b.receive(A, "Alice", &invite.body, 6).unwrap();
        assert_eq!(b.pending(6).len(), 1);
        let ack = b.accept(A, 7).unwrap().remove(0);
        a.receive(B, "Bob", &ack.body, 8).unwrap();
        let frame = a.snapshot_for(B, snapshot()).unwrap();
        b.receive(A, "Alice", &frame.body, 9).unwrap();
    }

    #[test]
    fn friends_persist_scoped_without_pending_or_telemetry() {
        static SERIAL: AtomicU64 = AtomicU64::new(0);
        let path = std::env::temp_dir().join(format!(
            "horizon-party-test-{}-{}.json",
            std::process::id(),
            SERIAL.fetch_add(1, Ordering::Relaxed)
        ));
        let (a, mut b) = parties();
        a.save(&path).unwrap();
        let loaded = Party::load(&path, "server|Alice", 999).unwrap();
        assert_eq!(loaded.friends(), a.friends());
        assert!(!loaded.peer_ready(B));
        assert!(
            Party::load(&path, "other-server|Alice", 999)
                .unwrap()
                .friends()
                .is_empty()
        );
        assert!(
            Party::load(&path, "server|OtherAccount", 999)
                .unwrap()
                .friends()
                .is_empty()
        );
        b.invite(C, "Charlie", 1).unwrap();
        b.save(&path).unwrap();
        let mut loaded = Party::load(&path, "server|Bob", 999).unwrap();
        assert!(loaded.pending(2).is_empty());
        assert!(loaded.online(&[A], 2).is_empty());
        fs::write(&path, b"bad JSON").unwrap();
        assert!(a.save(&path).is_err());
        assert_eq!(fs::read(&path).unwrap(), b"bad JSON");
        fs::remove_file(path).unwrap();
    }

    #[test]
    fn bounded_messages_and_invitation_expiration() {
        let mut a = Party::new("server|Alice", 1);
        let out = a.invite(B, "Bob", 0).unwrap().remove(0);
        assert!(out.body.len() <= MAX_BODY);
        let mut b = Party::new("server|Bob", 2);
        b.receive(A, "Alice", &out.body, 1).unwrap();
        assert!(b.accept(A, INVITE_TTL + 2).is_err());
        a.disconnect();
        for uuid in 100..108 {
            a.invite(uuid, "Player", 10).unwrap();
        }
        assert!(a.invite(109, "Player", 10).is_err());
    }
}
