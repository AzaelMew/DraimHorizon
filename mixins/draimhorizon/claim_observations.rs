// Observed ownership is separate from a mod owner's signed self-announcement.
use super::claim_borders::{Claim, parse_claims};
use serde_json::{Value, json};
use std::collections::BTreeMap;
pub const PREFIX: &str = "[DHO1] ";
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Record {
    pub claim: Claim,
    pub owner: Option<String>,
    pub at: u64,
    pub reporter: String,
}
#[derive(Clone, Default)]
pub struct Observations {
    pub records: BTreeMap<Claim, Record>,
    pub revision: u64,
}
fn name(n: &str) -> bool {
    !n.is_empty() && n.len() <= 16 && n.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'_')
}
fn valid(c: &Claim) -> bool {
    parse_claims(&format!(
        "Your chunks (1 of 4096): {} at x {} z {}",
        c.dimension, c.x, c.z
    ))
    .ok()
    .flatten()
    .is_some_and(|v| v.len() == 1)
}
pub fn reply(text: &str) -> Option<(i32, i32, Option<String>)> {
    let (bounds, tail) = text.strip_prefix("This chunk (x ")?.split_once(") is ")?;
    let (x, z) = bounds.split_once(", z ")?;
    let parse = |s: &str| {
        let (a, b) = s.split_once(" to ")?;
        let (a, b) = (a.parse::<i32>().ok()?, b.parse::<i32>().ok()?);
        (a % 16 == 0 && a.checked_add(15) == Some(b) && a.unsigned_abs() <= 30_000_000).then_some(a)
    };
    let owner = if tail == "free: /claim it." {
        None
    } else {
        Some(tail.strip_suffix("'s.")?.to_owned())
    };
    if owner.as_ref().is_some_and(|n| !name(n)) {
        return None;
    }
    Some((parse(x)?, parse(z)?, owner))
}
pub fn denied_owner(text: &str) -> Option<String> {
    let owner = text
        .trim()
        .strip_prefix("This land is claimed by ")?
        .strip_suffix('.')?;
    name(owner).then(|| owner.to_owned())
}
/// Bind an owner-only denial to one recent, unambiguous block target.
#[derive(Default)]
pub struct BreakEvidence {
    recent: BTreeMap<Claim, u64>,
    blocked_until: u64,
}
impl BreakEvidence {
    const TTL: u64 = 4000;
    fn prune(&mut self, now: u64) {
        self.recent
            .retain(|_, at| now >= *at && now - *at <= Self::TTL);
    }
    pub fn interrupt(&mut self, now: u64) {
        self.recent.clear();
        self.blocked_until = now.saturating_add(Self::TTL);
    }
    pub fn attempt(&mut self, dimension: &str, pos: [i32; 3], now: u64) {
        self.prune(now);
        if now < self.blocked_until {
            return;
        }
        let claim = Claim {
            dimension: dimension.into(),
            x: pos[0].div_euclid(16) * 16,
            z: pos[2].div_euclid(16) * 16,
        };
        if !valid(&claim) {
            return;
        }
        if self.recent.len() >= 8 && !self.recent.contains_key(&claim) {
            self.interrupt(now);
            return;
        }
        self.recent.insert(claim, now);
    }
    pub fn denial(&mut self, dimension: &str, text: &str, now: u64) -> Option<(Claim, String)> {
        let owner = denied_owner(text)?;
        self.prune(now);
        if now < self.blocked_until || self.recent.len() != 1 {
            return None;
        }
        let claim = self.recent.keys().next()?.clone();
        if claim.dimension != dimension {
            return None;
        }
        self.recent.clear();
        Some((claim, owner))
    }
}
impl Observations {
    pub fn merge(&mut self, r: Record, now: u64) -> Result<bool, String> {
        if !valid(&r.claim)
            || r.owner.as_deref().is_some_and(|n| !name(n))
            || !name(&r.reporter)
            || r.at == 0
            || r.at > now.saturating_add(300000)
        {
            return Err("invalid observed claim".into());
        }
        if let Some(old) = self.records.get(&r.claim) {
            if (old.at, &old.owner) >= (r.at, &r.owner) {
                return Ok(false);
            }
        } else if self.records.len() >= 8192 {
            return Err("observation limit reached".into());
        }
        self.records.insert(r.claim.clone(), r);
        self.revision = self.revision.wrapping_add(1).max(1);
        Ok(true)
    }
    pub fn receive(&mut self, body: &str, reporter: &str, now: u64) -> Result<bool, String> {
        if body.len() > 1024 || !name(reporter) {
            return Err("invalid observation frame".into());
        }
        let value: Value = serde_json::from_str(
            body.strip_prefix(PREFIX)
                .ok_or("invalid observation prefix")?,
        )
        .map_err(|_| "invalid observation JSON")?;
        let rows = value
            .as_array()
            .filter(|r| !r.is_empty() && r.len() <= 4)
            .ok_or("invalid observation page")?;
        let mut next = self.clone();
        let mut changed = false;
        for row in rows {
            let r = decode(row, reporter)?;
            changed |= next.merge(r, now)?;
        }
        if changed {
            *self = next;
        }
        Ok(changed)
    }
    pub fn frames(&self) -> Vec<String> {
        self.records
            .values()
            .collect::<Vec<_>>()
            .chunks(4)
            .map(|rows| {
                format!(
                    "{PREFIX}{}",
                    json!(rows.iter().map(|r| encode(r)).collect::<Vec<_>>())
                )
            })
            .collect()
    }
    pub fn encode(&self, scope: &str) -> String {
        json!({"schema":1,"scope":scope,"observed":self.records.values().map(|r|json!([encode(r),r.reporter])).collect::<Vec<_>>()}).to_string()
    }
    pub fn decode(raw: &str, scope: &str, now: u64) -> Result<Self, String> {
        if raw.len() > 4 * 1024 * 1024 {
            return Err("observation cache too large".into());
        }
        let v: Value = serde_json::from_str(raw).map_err(|_| "invalid observation cache")?;
        if v["schema"] != 1 || v["scope"].as_str() != Some(scope) {
            return Err("observation scope/schema mismatch".into());
        }
        let rows = v["observed"]
            .as_array()
            .filter(|r| r.len() <= 8192)
            .ok_or("invalid observations")?;
        let mut result = Self::default();
        for r in rows {
            result.merge(
                decode(&r[0], r[1].as_str().ok_or("invalid reporter")?)?,
                now,
            )?;
        }
        Ok(result)
    }
}
fn encode(r: &Record) -> Value {
    json!([r.claim.dimension, r.claim.x, r.claim.z, r.owner, r.at])
}
fn decode(v: &Value, reporter: &str) -> Result<Record, String> {
    let r = v
        .as_array()
        .filter(|r| r.len() == 5)
        .ok_or("invalid observed row")?;
    Ok(Record {
        claim: Claim {
            dimension: r[0].as_str().ok_or("invalid dimension")?.into(),
            x: r[1]
                .as_i64()
                .and_then(|n| i32::try_from(n).ok())
                .ok_or("invalid x")?,
            z: r[2]
                .as_i64()
                .and_then(|n| i32::try_from(n).ok())
                .ok_or("invalid z")?,
        },
        owner: if r[3].is_null() {
            None
        } else {
            Some(r[3].as_str().ok_or("invalid owner")?.into())
        },
        at: r[4].as_u64().ok_or("invalid timestamp")?,
        reporter: reporter.into(),
    })
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn actionbar_binds_the_target_not_the_players_chunk_and_rejects_ambiguity() {
        let mut b = BreakEvidence::default();
        assert!(
            b.denial("minecraft:overworld", "This land is claimed by Alex.", 100)
                .is_none()
        );
        b.attempt("minecraft:overworld", [-17, 64, 32], 100);
        let (claim, owner) = b
            .denial(
                "minecraft:overworld",
                "This land is claimed by Freskinlord.",
                200,
            )
            .unwrap();
        assert_eq!((claim.x, claim.z), (-32, 32));
        assert_eq!(owner, "Freskinlord");
        assert!(
            b.denial("minecraft:overworld", "This land is claimed by Alex.", 201)
                .is_none()
        );
        b.attempt("minecraft:overworld", [0, 64, 0], 300);
        b.attempt("minecraft:overworld", [16, 64, 0], 301);
        assert!(
            b.denial("minecraft:overworld", "This land is claimed by Alex.", 400)
                .is_none()
        );
        assert!(
            b.denial("minecraft:the_nether", "This land is claimed by Alex.", 400)
                .is_none()
        );
        b.attempt("minecraft:overworld", [0, 64, 0], 5000);
        assert!(
            b.denial(
                "minecraft:overworld",
                "This land is claimed by Alex.",
                10000
            )
            .is_none()
        );
        b.attempt("minecraft:overworld", [0, 64, 0], 11000);
        b.interrupt(11001);
        assert!(
            b.denial(
                "minecraft:overworld",
                "This land is claimed by Alex.",
                11002
            )
            .is_none()
        );
        assert!(denied_owner("<Steve> This land is claimed by Alex.").is_none());
        assert!(denied_owner("This land is claimed by Alex;op.").is_none());
        assert!(denied_owner("This land is claimed by Alex. extra").is_none());
    }
    #[test]
    fn claim_info_binds_exact_block_bounds_and_owner() {
        assert_eq!(
            reply("This chunk (x -32 to -17, z 48 to 63) is Alex's."),
            Some((-32, 48, Some("Alex".into())))
        );
        assert_eq!(
            reply("This chunk (x 0 to 15, z 0 to 15) is free: /claim it."),
            Some((0, 0, None))
        );
        for s in [
            "<Steve> This chunk (x 0 to 15, z 0 to 15) is Alex's.",
            "This chunk (x 1 to 16, z 0 to 15) is Alex's.",
            "This chunk (x 0 to 20, z 0 to 15) is Alex's.",
        ] {
            assert!(reply(s).is_none());
        }
    }
    #[test]
    fn peers_converge_and_free_tombstone_survives_rejoin() {
        let now = 100000;
        let c = Claim {
            dimension: "minecraft:overworld".into(),
            x: -32,
            z: 48,
        };
        let mut a = Observations::default();
        a.merge(
            Record {
                claim: c.clone(),
                owner: Some("NoModOwner".into()),
                at: now,
                reporter: "Scout".into(),
            },
            now,
        )
        .unwrap();
        let mut b = Observations::default();
        for f in a.frames() {
            assert!(f.len() <= 1024);
            b.receive(&f, "Scout", now).unwrap();
        }
        assert_eq!(b.records[&c].owner.as_deref(), Some("NoModOwner"));
        b.merge(
            Record {
                claim: c.clone(),
                owner: None,
                at: now + 1,
                reporter: "Alex".into(),
            },
            now + 1,
        )
        .unwrap();
        for f in b.frames() {
            a.receive(&f, "Alex", now + 1).unwrap();
        }
        assert!(a.records[&c].owner.is_none());
        assert!(!a.receive(&a.frames()[0], "Alex", now + 1).unwrap());
        let raw = a.encode("s");
        let mut restored = Observations::decode(&raw, "s", now + 1).unwrap();
        assert!(Observations::decode(&raw, "other", now + 1).is_err());
        let old = format!("{PREFIX}[[\"minecraft:overworld\",-32,48,\"NoModOwner\",{now}]]");
        assert!(!restored.receive(&old, "Scout", now + 1).unwrap());
    }
    #[test]
    fn invalid_or_future_rows_do_not_partially_apply() {
        let mut a = Observations::default();
        assert!(a.receive("[DHO1] [[\"minecraft:overworld\",0,0,\"Alex\",100],[\"minecraft:overworld\",1,0,\"Alex\",100]]","Scout",100).is_err());
        assert!(a.records.is_empty());
        assert!(
            a.receive(
                "[DHO1] [[\"minecraft:overworld\",0,0,\"Alex\",999999999]]",
                "Scout",
                100
            )
            .is_err()
        );
    }
}
