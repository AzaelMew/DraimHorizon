// Persistent, owner-key-bound land directory. All-or-nothing paged announcements.
use super::claim_borders::{Claim, parse_claims};
use claim_borders_relay_client::translation::Language;
use serde_json::{Value, json};
use std::{
    collections::{BTreeMap, BTreeSet},
    fs,
    io::{Read, Write},
    path::Path,
    time::{SystemTime, UNIX_EPOCH},
};
/// Error code for a changed owner key; shown via `Directory::error_text`.
pub const KEY_CHANGED: &str = "owner key changed";
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Record {
    pub name: String,
    pub key: String,
    pub revision: u64,
    pub claims: Vec<Claim>,
}
#[derive(Clone)]
struct Transfer {
    key: String,
    revision: u64,
    pages: Vec<Option<Vec<Claim>>>,
    at: u64,
}
#[derive(Clone, Default)]
pub struct Directory {
    pub records: BTreeMap<u128, Record>,
    pending: BTreeMap<u128, Transfer>,
}
fn valid_name(s: &str) -> bool {
    !s.is_empty() && s.len() <= 16 && s.bytes().all(|c| c.is_ascii_alphanumeric() || c == b'_')
}
fn valid_key(s: &str) -> bool {
    s.len() == 24 && s.bytes().all(|c| c.is_ascii_hexdigit())
}
fn claims_value(claims: &[Claim]) -> Value {
    json!(
        claims
            .iter()
            .map(|c| json!([c.dimension, c.x, c.z]))
            .collect::<Vec<_>>()
    )
}
fn decode_claims(v: &Value) -> Result<Vec<Claim>, String> {
    let rows = v.as_array().ok_or("invalid claims")?;
    if rows.len() > 4096 {
        return Err("too many claims".into());
    }
    let mut claims = BTreeSet::new();
    for row in rows {
        let row = row
            .as_array()
            .filter(|r| r.len() == 3)
            .ok_or("invalid claim")?;
        let dim = row[0]
            .as_str()
            .filter(|d| d.len() <= 128)
            .ok_or("invalid dimension")?;
        let x = row[1]
            .as_i64()
            .and_then(|n| i32::try_from(n).ok())
            .ok_or("invalid x")?;
        let z = row[2]
            .as_i64()
            .and_then(|n| i32::try_from(n).ok())
            .ok_or("invalid z")?;
        // Reuse the authoritative parser's dimension, bounds and chunk-origin validation.
        let line = format!("Your chunks (1 of 4096): {dim} at x {x} z {z}");
        let c = parse_claims(&line)
            .map_err(|_| "invalid claim coordinates")?
            .ok_or("invalid claim")?
            .pop()
            .ok_or("missing claim")?;
        if !claims.insert(c) {
            return Err("duplicate claim".into());
        }
    }
    Ok(claims.into_iter().collect())
}
impl Directory {
    pub fn forget_others(&mut self, own: u128) {
        self.records.retain(|id, _| *id == own);
        self.pending.clear();
    }
    pub fn local(
        &mut self,
        uuid: u128,
        name: &str,
        key: &str,
        claims: Vec<Claim>,
    ) -> Result<bool, String> {
        if !self.records.contains_key(&uuid) && self.records.len() >= 256 {
            return Err("claim owner limit reached".into());
        }
        let claims = decode_claims(&claims_value(&claims))?;
        if !valid_name(name) || (!key.is_empty() && !valid_key(key)) {
            return Err("invalid owner".into());
        }
        if self
            .records
            .get(&uuid)
            .is_some_and(|r| r.claims == claims && r.key == key && r.name == name)
        {
            return Ok(false);
        }
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis() as u64;
        let revision = self
            .records
            .get(&uuid)
            .map_or(now.max(1), |r| r.revision.saturating_add(1).max(now));
        self.records.insert(
            uuid,
            Record {
                name: name.into(),
                key: key.into(),
                revision,
                claims,
            },
        );
        Ok(true)
    }
    pub fn frames(record: &Record) -> Result<Vec<String>, String> {
        if record.claims.len() > 4096 {
            return Err("too many claims".into());
        }
        let mut pages = vec![Vec::new()];
        for c in &record.claims {
            let last = pages.last_mut().unwrap();
            last.push(c.clone());
            if format!(
                "[DHC1] {}",
                json!([record.revision, 1023, 1024, claims_value(last)])
            )
            .len()
                > 1024
            {
                let c = last.pop().unwrap();
                if last.is_empty() {
                    return Err("claim is too large".into());
                }
                pages.push(vec![c]);
            }
        }
        if pages.len() > 1024 {
            return Err("too many claim pages".into());
        }
        Ok(pages
            .iter()
            .enumerate()
            .map(|(i, p)| {
                format!(
                    "[DHC1] {}",
                    json!([record.revision, i, pages.len(), claims_value(p)])
                )
            })
            .collect())
    }
    /// Display text for an error returned by `receive`; unknown errors pass through.
    pub fn error_text(error: &str, language: Language) -> String {
        if error == KEY_CHANGED {
            language
                .text(
                    "Ключ владельца приватных территорий изменился; обновите кеш игроков в Клеймах.",
                    "The private claim owner's key changed; refresh the player cache in Claims.",
                )
                .into()
        } else {
            error.into()
        }
    }
    pub fn receive(
        &mut self,
        uuid: u128,
        name: &str,
        key: &str,
        body: &str,
        now: u64,
    ) -> Result<bool, String> {
        if !valid_name(name) || !valid_key(key) || body.len() > 1024 {
            return Err("invalid claim publisher".into());
        }
        if self
            .records
            .get(&uuid)
            .is_some_and(|r| !r.key.is_empty() && r.key != key)
        {
            return Err(KEY_CHANGED.into());
        }
        let value: Value =
            serde_json::from_str(body.strip_prefix("[DHC1] ").ok_or("invalid protocol")?)
                .map_err(|_| "invalid announcement")?;
        let a = value
            .as_array()
            .filter(|a| a.len() == 4)
            .ok_or("invalid announcement")?;
        let revision = a[0].as_u64().filter(|r| *r > 0).ok_or("invalid revision")?;
        let page = a[1].as_u64().ok_or("invalid page")? as usize;
        let total = a[2]
            .as_u64()
            .filter(|n| *n > 0 && *n <= 1024)
            .ok_or("invalid page count")? as usize;
        if page >= total {
            return Err("invalid page".into());
        }
        let claims = decode_claims(&a[3])?;
        if self
            .records
            .get(&uuid)
            .is_some_and(|r| r.revision >= revision)
        {
            return Ok(false);
        }
        self.pending.retain(|_, t| now.saturating_sub(t.at) < 15000);
        if !self.pending.contains_key(&uuid) && self.pending.len() >= 16 {
            return Err("claim transfer queue full".into());
        }
        if self
            .pending
            .get(&uuid)
            .is_some_and(|t| t.revision > revision)
        {
            return Ok(false);
        }
        let transfer = self.pending.entry(uuid).or_insert_with(|| Transfer {
            key: key.into(),
            revision,
            pages: vec![None; total],
            at: now,
        });
        if transfer.revision != revision || transfer.key != key {
            *transfer = Transfer {
                key: key.into(),
                revision,
                pages: vec![None; total],
                at: now,
            };
        }
        if transfer.pages.len() != total {
            return Err("inconsistent claim page count".into());
        }
        if transfer.pages[page]
            .as_ref()
            .is_some_and(|old| *old != claims)
        {
            return Err("conflicting claim page".into());
        }
        if transfer.pages[page].is_none() {
            transfer.at = now;
        }
        transfer.pages[page] = Some(claims);
        if transfer.pages.iter().flatten().map(Vec::len).sum::<usize>() > 4096 {
            self.pending.remove(&uuid);
            return Err("too many claims".into());
        }
        if transfer.pages.iter().any(Option::is_none) {
            return Ok(false);
        }
        let transfer = self.pending.remove(&uuid).unwrap();
        let all: Vec<_> = transfer.pages.into_iter().flatten().flatten().collect();
        let claims = decode_claims(&claims_value(&all))?;
        if !self.records.contains_key(&uuid) && self.records.len() >= 256 {
            return Err("claim owner limit reached".into());
        }
        self.records.insert(
            uuid,
            Record {
                name: name.into(),
                key: key.into(),
                revision,
                claims,
            },
        );
        Ok(true)
    }
    pub fn load(path: &Path, scope: &str) -> Result<Self, String> {
        let file = match fs::File::open(path) {
            Ok(f) => f,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(Self::default()),
            Err(e) => return Err(e.to_string()),
        };
        let mut bytes = Vec::new();
        file.take(32 * 1024 * 1024 + 1)
            .read_to_end(&mut bytes)
            .map_err(|e| e.to_string())?;
        if bytes.len() > 32 * 1024 * 1024 {
            return Err("claim cache too large".into());
        }
        let v: Value = serde_json::from_slice(&bytes).map_err(|_| "invalid claim cache")?;
        if v["schema"] != 1 || v["scope"].as_str() != Some(scope) {
            return Err("claim cache schema or server mismatch".into());
        }
        let rows = v["owners"]
            .as_array()
            .filter(|a| a.len() <= 256)
            .ok_or("invalid claim owners")?;
        let mut store = Self::default();
        for row in rows {
            let a = row
                .as_array()
                .filter(|a| a.len() == 5)
                .ok_or("invalid owner")?;
            let uuid = a[0]
                .as_str()
                .filter(|s| s.len() == 32)
                .and_then(|s| u128::from_str_radix(s, 16).ok())
                .ok_or("invalid owner uuid")?;
            let name = a[1]
                .as_str()
                .filter(|s| valid_name(s))
                .ok_or("invalid owner name")?;
            let key = a[2]
                .as_str()
                .filter(|s| s.is_empty() || valid_key(s))
                .ok_or("invalid owner key")?;
            let revision = a[3]
                .as_u64()
                .filter(|n| *n > 0)
                .ok_or("invalid claim revision")?;
            let claims = decode_claims(&a[4])?;
            if store
                .records
                .insert(
                    uuid,
                    Record {
                        name: name.into(),
                        key: key.into(),
                        revision,
                        claims,
                    },
                )
                .is_some()
            {
                return Err("duplicate claim owner".into());
            }
        }
        Ok(store)
    }
    pub fn save(&self, path: &Path, scope: &str) -> Result<(), String> {
        if path.exists() {
            Self::load(path, scope)?;
        }
        let value = json!({"schema":1,"scope":scope,"owners":self.records.iter().map(|(id,r)|json!([format!("{id:032x}"),r.name,r.key,r.revision,claims_value(&r.claims)])).collect::<Vec<_>>()});
        let bytes = serde_json::to_vec(&value).map_err(|e| e.to_string())?;
        if bytes.len() > 32 * 1024 * 1024 {
            return Err("claim cache too large".into());
        }
        let parent = path.parent().ok_or("invalid cache path")?;
        fs::create_dir_all(parent).map_err(|e| e.to_string())?;
        let temp = path.with_extension("pending");
        let mut file = fs::File::create(&temp).map_err(|e| e.to_string())?;
        file.write_all(&bytes)
            .and_then(|_| file.sync_all())
            .map_err(|e| e.to_string())?;
        fs::rename(temp, path).map_err(|e| e.to_string())
    }
}
pub fn confirmation(text: &str) -> Option<(bool, i32, i32)> {
    let (add, tail) = if let Some(t) = text.strip_prefix("Claimed this chunk (x ") {
        (true, t)
    } else {
        (false, text.strip_prefix("Unclaimed this chunk (x ")?)
    };
    let (x, tail) = tail.split_once(" to ")?;
    let (right, tail) = tail.split_once(", z ")?;
    let (z, tail) = tail.split_once(" to ")?;
    let bottom = tail.split_once(')')?.0;
    let (x, z, right, bottom) = (
        x.parse::<i32>().ok()?,
        z.parse::<i32>().ok()?,
        right.parse::<i32>().ok()?,
        bottom.parse::<i32>().ok()?,
    );
    if x.rem_euclid(16) != 0
        || z.rem_euclid(16) != 0
        || x.checked_add(15) != Some(right)
        || z.checked_add(15) != Some(bottom)
    {
        return None;
    }
    Some((add, x, z))
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn english_key_change_text_is_not_cyrillic() {
        let text = Directory::error_text(KEY_CHANGED, Language::English);
        assert!(!text.chars().any(|c| matches!(c, 'А'..='я' | 'Ё' | 'ё')));
        assert_eq!(Directory::error_text("x", Language::English), "x");
    }
    fn claim(x: i32) -> Claim {
        Claim {
            dimension: "minecraft:overworld".into(),
            x: x * 16,
            z: -16,
        }
    }
    fn record(n: i32) -> Record {
        Record {
            name: "Alex".into(),
            key: "a".repeat(24),
            revision: 7,
            claims: (0..n).map(claim).collect(),
        }
    }
    #[test]
    fn pages_commit_only_when_complete_and_do_not_require_friendship() {
        let r = record(90);
        let pages = Directory::frames(&r).unwrap();
        assert!(pages.len() > 1);
        assert!(pages.iter().all(|p| p.len() <= 1024));
        let mut d = Directory::default();
        for (i, p) in pages.iter().rev().enumerate() {
            assert_eq!(
                d.receive(1, "Alex", &r.key, p, i as u64).unwrap(),
                i == pages.len() - 1
            );
            if i < pages.len() - 1 {
                assert!(d.records.is_empty());
            }
        }
        assert_eq!(d.records[&1], r);
    }
    #[test]
    fn old_replay_changed_key_partial_and_empty_updates_are_handled() {
        let mut d = Directory::default();
        let r = record(3);
        let old = Directory::frames(&r).unwrap();
        for p in &old {
            d.receive(1, "Alex", &r.key, p, 0).unwrap();
        }
        assert!(!d.receive(1, "Alex", &r.key, &old[0], 1).unwrap());
        assert!(d.receive(1, "Alex", &"b".repeat(24), &old[0], 2).is_err());
        let mut empty = record(0);
        empty.revision = 8;
        d.receive(1, "Alex", &r.key, &Directory::frames(&empty).unwrap()[0], 3)
            .unwrap();
        assert!(d.records[&1].claims.is_empty());
        assert!(!d.receive(1, "Alex", &r.key, &old[0], 4).unwrap());
    }
    #[test]
    fn incomplete_transfer_preserves_the_last_known_territory() {
        let mut d = Directory::default();
        let r = record(1);
        d.records.insert(1, r.clone());
        let large = Record {
            revision: 8,
            ..record(150)
        };
        let pages = Directory::frames(&large).unwrap();
        d.receive(1, "Alex", &r.key, &pages[0], 0).unwrap();
        assert_eq!(d.records[&1], r);
        d.receive(1, "Alex", &r.key, &pages[1], 16000).unwrap();
        assert_eq!(d.records[&1], r);
    }
    #[test]
    fn malformed_bounds_duplicate_and_forged_payloads_do_not_replace_land() {
        let mut d = Directory::default();
        let r = record(1);
        d.records.insert(1, r.clone());
        for bad in [
            "[DHC1] [8,0,1,[[\"minecraft:overworld\",1,0]]]",
            "[DHC1] [8,0,1,[[\"minecraft:overworld\",0,0],[\"minecraft:overworld\",0,0]]]",
            "[DHC1] [8,2,1,[]]",
            "[DHC1] [8,0,5000,[]]",
            "[DHC1] [8,0,1,[[\"Bad:Dimension\",0,0]]]",
        ] {
            assert!(d.receive(1, "Alex", &r.key, bad, 0).is_err());
            assert_eq!(d.records[&1], r);
        }
    }
    #[test]
    fn disk_cache_is_scoped_survives_restart_and_keeps_offline_owners() {
        let path = std::env::temp_dir().join(format!("horizon-land-{}.json", std::process::id()));
        let _ = fs::remove_file(&path);
        let mut d = Directory::default();
        d.records.insert(1, record(4));
        d.save(&path, "server#account").unwrap();
        let restored = Directory::load(&path, "server#account").unwrap();
        assert_eq!(restored.records, d.records);
        assert!(Directory::load(&path, "another server").is_err());
        let bytes = fs::read(&path).unwrap();
        fs::write(&path, b"broken").unwrap();
        assert!(d.save(&path, "server#account").is_err());
        assert_eq!(fs::read(&path).unwrap(), b"broken");
        fs::write(&path, &bytes).unwrap();
        fs::remove_file(path).unwrap();
    }
    #[test]
    fn confirmations_take_exact_block_origins_and_local_revisions_change() {
        assert_eq!(
            confirmation(
                "Claimed this chunk (x -16 to -1, z 32 to 47), 1 of 4. /trust <player> lets a friend build."
            ),
            Some((true, -16, 32))
        );
        assert_eq!(
            confirmation("Unclaimed this chunk (x -16 to -1, z 32 to 47)."),
            Some((false, -16, 32))
        );
        assert!(confirmation("Claimed this chunk (x 1 to 16, z 32 to 47).").is_none());
        let mut d = Directory::default();
        assert!(d.local(1, "Alex", "", vec![claim(0)]).unwrap());
        let rev = d.records[&1].revision;
        assert!(!d.local(1, "Alex", "", vec![claim(0)]).unwrap());
        assert!(d.local(1, "Alex", "", Vec::new()).unwrap());
        assert!(d.records[&1].revision > rev);
    }
}
