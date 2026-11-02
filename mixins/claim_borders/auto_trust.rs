// A mutual friend is granted access once, after a server acknowledgement.
use serde_json::{Value, json};
use std::{
    collections::{BTreeMap, BTreeSet},
    fs,
    io::{Read, Write},
    path::Path,
};
#[derive(Clone, Debug)]
pub struct Pending {
    pub uuid: u128,
    pub name: String,
    pub at: u64,
}
#[derive(Default)]
pub struct Trust {
    pub granted: BTreeSet<u128>,
    pub blocked: BTreeSet<u128>,
    pub pending: Option<Pending>,
    tried: BTreeMap<u128, u64>,
    pub failed: bool,
}
fn valid(s: &str) -> bool {
    !s.is_empty() && s.len() <= 16 && s.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'_')
}
impl Trust {
    pub fn next(
        &mut self,
        friends: &[(u128, String)],
        online: &[(u128, String)],
        now: u64,
    ) -> Option<String> {
        self.blocked
            .retain(|id| friends.iter().any(|(f, _)| f == id));
        if self
            .pending
            .as_ref()
            .is_some_and(|p| now.saturating_sub(p.at) > 10000)
        {
            self.pending = None;
        }
        if self.failed || self.pending.is_some() {
            return None;
        }
        let (id, name) = friends
            .iter()
            .filter(|(id, _)| {
                !self.granted.contains(id)
                    && !self.blocked.contains(id)
                    && self
                        .tried
                        .get(id)
                        .is_none_or(|t| now.saturating_sub(*t) >= 60000)
            })
            .find_map(|(id, _)| online.iter().find(|(o, n)| o == id && valid(n)))?;
        self.tried.insert(*id, now);
        self.pending = Some(Pending {
            uuid: *id,
            name: name.clone(),
            at: now,
        });
        Some(format!("trust {name}"))
    }
    pub fn manual(&mut self, command: &str, friends: &[(u128, String)]) {
        let words: Vec<_> = command.split_whitespace().collect();
        if words.len() != 2 || words[0] != "untrust" {
            return;
        }
        if let Some((id, _)) = friends
            .iter()
            .find(|(_, n)| n.eq_ignore_ascii_case(words[1]))
        {
            self.blocked.insert(*id);
            self.granted.remove(id);
            if self.pending.as_ref().is_some_and(|p| p.uuid == *id) {
                self.pending = None;
            }
        }
    }
    pub fn reply(&mut self, text: &str) -> bool {
        let Some(p) = &self.pending else {
            return false;
        };
        if text == format!("{} may now build and open things on your land.", p.name)
            || text == format!("You already trust {}.", p.name)
        {
            let p = self.pending.take().unwrap();
            self.granted.insert(p.uuid);
            return true;
        }
        if text.contains("Unknown command") || text.contains("Claims are off on this server") {
            self.failed = true;
            self.pending = None;
        }
        false
    }
    pub fn load(path: &Path, scope: &str) -> Result<Self, String> {
        let file = match fs::File::open(path) {
            Ok(f) => f,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(Self::default()),
            Err(e) => return Err(e.to_string()),
        };
        let mut data = String::new();
        file.take(65537)
            .read_to_string(&mut data)
            .map_err(|e| e.to_string())?;
        if data.len() > 65536 {
            return Err("trust cache too large".into());
        }
        let v: Value = serde_json::from_str(&data).map_err(|_| "invalid trust cache")?;
        if v["schema"] != 1 || v["scope"].as_str() != Some(scope) {
            return Err("trust cache server/schema mismatch".into());
        }
        let ids = |v: &Value| -> Result<BTreeSet<u128>, String> {
            v.as_array()
                .filter(|a| a.len() <= 256)
                .ok_or("invalid trusted ids")?
                .iter()
                .map(|x| {
                    x.as_str()
                        .filter(|s| s.len() == 32)
                        .and_then(|s| u128::from_str_radix(s, 16).ok())
                        .ok_or("invalid trusted UUID".into())
                })
                .collect()
        };
        Ok(Self {
            granted: ids(&v["granted"])?,
            blocked: ids(&v["blocked"])?,
            ..Self::default()
        })
    }
    pub fn save(&self, path: &Path, scope: &str) -> Result<(), String> {
        if path.exists() {
            Self::load(path, scope)?;
        }
        let ids = |v: &BTreeSet<u128>| v.iter().map(|id| format!("{id:032x}")).collect::<Vec<_>>();
        let data=json!({"schema":1,"scope":scope,"granted":ids(&self.granted),"blocked":ids(&self.blocked)}).to_string();
        fs::create_dir_all(path.parent().ok_or("invalid trust path")?)
            .map_err(|e| e.to_string())?;
        let temp = path.with_extension("pending");
        let mut f = fs::File::create(&temp).map_err(|e| e.to_string())?;
        f.write_all(data.as_bytes())
            .and_then(|_| f.sync_all())
            .map_err(|e| e.to_string())?;
        fs::rename(temp, path).map_err(|e| e.to_string())
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn only_online_mutual_friends_get_grants_and_acknowledgements_stop_repeats() {
        let mut t = Trust::default();
        let friends = vec![(1, "Alex".into())];
        assert!(t.next(&friends, &[(2, "Other".into())], 0).is_none());
        assert_eq!(t.next(&friends, &friends, 0), Some("trust Alex".into()));
        assert!(!t.reply("Other may now build and open things on your land."));
        assert!(t.reply("Alex may now build and open things on your land."));
        assert!(t.next(&friends, &friends, 100000).is_none());
    }
    #[test]
    fn manual_untrust_is_respected_and_failures_do_not_spam() {
        let mut t = Trust::default();
        let f = vec![(1, "Alex".into())];
        t.manual("untrust Alex", &f);
        assert!(t.next(&f, &f, 0).is_none());
        t.next(&[], &[], 1);
        assert!(t.next(&f, &f, 2).is_some());
        assert!(t.next(&f, &f, 12000).is_none());
        assert!(t.next(&f, &f, 60002).is_some());
    }
}
