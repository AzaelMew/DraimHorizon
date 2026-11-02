// Exact author-name rules; pinned messages never execute server commands.
use serde_json::{Value, json};
use std::collections::BTreeMap;
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Rule {
    pub color: u32,
    pub pinned: bool,
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Preferences {
    pub rules: BTreeMap<String, Rule>,
    pub friends: bool,
    pub tray: bool,
    pub seconds: u16,
    pub rows: u8,
}
impl Default for Preferences {
    fn default() -> Self {
        Self {
            rules: BTreeMap::new(),
            friends: false,
            tray: true,
            seconds: 60,
            rows: 3,
        }
    }
}
impl Preferences {
    pub fn set(&mut self, name: &str, color: u32, pinned: bool) -> Result<(), String> {
        if name.is_empty()
            || name.len() > 16
            || !name.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'_')
        {
            return Err("Нужен точный ник игрока (до 16 символов)".into());
        }
        if self.rules.len() >= 64 && !self.rules.contains_key(&name.to_ascii_lowercase()) {
            return Err("Не больше 64 важных игроков".into());
        }
        self.rules.insert(
            name.to_ascii_lowercase(),
            Rule {
                color: color & 0xffffff,
                pinned,
            },
        );
        Ok(())
    }
    pub fn rule(&self, name: &str, friend: bool) -> Option<Rule> {
        self.rules
            .get(&name.to_ascii_lowercase())
            .cloned()
            .or_else(|| {
                (self.friends && friend).then_some(Rule {
                    color: 0x77d9b0,
                    pinned: true,
                })
            })
    }
    pub fn encode(&self) -> String {
        json!({"schema":1,"rules":self.rules.iter().map(|(name,r)|json!([name,r.color,r.pinned])).collect::<Vec<_>>(),"friends":self.friends,"tray":self.tray,"seconds":self.seconds,"rows":self.rows}).to_string()
    }
    pub fn decode(raw: &str) -> Result<Self, String> {
        if raw.len() > 16384 {
            return Err("priority settings too large".into());
        }
        let v: Value = serde_json::from_str(raw).map_err(|_| "invalid priority settings")?;
        if v["schema"] != 1 {
            return Err("unknown priority schema".into());
        }
        let mut p = Self {
            friends: v["friends"].as_bool().unwrap_or(false),
            tray: v["tray"].as_bool().unwrap_or(true),
            seconds: v["seconds"].as_u64().unwrap_or(60).clamp(10, 180) as u16,
            rows: v["rows"].as_u64().unwrap_or(3).clamp(1, 6) as u8,
            ..Self::default()
        };
        for r in v["rules"]
            .as_array()
            .filter(|a| a.len() <= 64)
            .ok_or("invalid priority players")?
        {
            p.set(
                r[0].as_str().ok_or("invalid nickname")?,
                r[1].as_u64()
                    .and_then(|n| u32::try_from(n).ok())
                    .ok_or("invalid colour")?,
                r[2].as_bool().ok_or("invalid pin mode")?,
            )?;
        }
        Ok(p)
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn names_match_exactly_and_settings_round_trip() {
        let mut p = Preferences::default();
        p.set("Alex", 0xffaa00, true).unwrap();
        assert!(p.rule("ALEX", false).unwrap().pinned);
        assert!(p.rule("Alex2", false).is_none());
        assert!(p.set("Alex;op", 0, true).is_err());
        assert_eq!(Preferences::decode(&p.encode()).unwrap(), p);
    }
}
