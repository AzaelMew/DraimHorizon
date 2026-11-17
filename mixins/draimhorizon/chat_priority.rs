// Exact author-name rules; pinned messages never execute server commands.
use claim_borders_relay_client::translation::Language;
use serde_json::{Value, json};
use std::collections::BTreeMap;
/// Colours the Important player picker offers, in menu order. The first one is the
/// default a new rule starts with.
pub const COLORS: [u32; 8] = [
    0xffcc66, 0x77d9b0, 0xa6c9e5, 0xbcb8e3, 0xdfa8b5, 0xa2d0b7, 0xffffff, 0xff8866,
];
/// `hue` in degrees, `sat` and `val` in 0..=1 to `0xRRGGBB`.
pub fn hsv(hue: f32, sat: f32, val: f32) -> u32 {
    let hue = if hue.is_finite() {
        hue.rem_euclid(360.0)
    } else {
        0.0
    };
    let sat = if sat.is_finite() {
        sat.clamp(0.0, 1.0)
    } else {
        0.0
    };
    let val = if val.is_finite() {
        val.clamp(0.0, 1.0)
    } else {
        0.0
    };
    let c = val * sat;
    let x = c * (1.0 - ((hue / 60.0) % 2.0 - 1.0).abs());
    let m = val - c;
    let (r, g, b) = match (hue / 60.0) as u32 {
        0 => (c, x, 0.0),
        1 => (x, c, 0.0),
        2 => (0.0, c, x),
        3 => (0.0, x, c),
        4 => (x, 0.0, c),
        _ => (c, 0.0, x),
    };
    let channel = |v: f32| ((v + m) * 255.0).round().clamp(0.0, 255.0) as u32;
    channel(r) << 16 | channel(g) << 8 | channel(b)
}
/// `0xRRGGBB` to `(hue degrees, saturation, value)`, the inverse of [`hsv`].
pub fn rgb_hsv(rgb: u32) -> (f32, f32, f32) {
    let r = ((rgb >> 16) & 0xff) as f32 / 255.0;
    let g = ((rgb >> 8) & 0xff) as f32 / 255.0;
    let b = (rgb & 0xff) as f32 / 255.0;
    let max = r.max(g).max(b);
    let min = r.min(g).min(b);
    let delta = max - min;
    let hue = if delta <= f32::EPSILON {
        0.0
    } else if max == r {
        60.0 * ((g - b) / delta).rem_euclid(6.0)
    } else if max == g {
        60.0 * ((b - r) / delta + 2.0)
    } else {
        60.0 * ((r - g) / delta + 4.0)
    };
    let sat = if max <= f32::EPSILON {
        0.0
    } else {
        delta / max
    };
    (hue.rem_euclid(360.0), sat, max)
}
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
            friends: true,
            tray: true,
            seconds: 60,
            rows: 3,
        }
    }
}
impl Preferences {
    pub fn set(&mut self, name: &str, color: u32, pinned: bool) -> Result<(), String> {
        self.set_in(name, color, pinned, Language::Russian)
    }
    pub fn set_in(
        &mut self,
        name: &str,
        color: u32,
        pinned: bool,
        language: Language,
    ) -> Result<(), String> {
        if name.is_empty()
            || name.len() > 16
            || !name.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'_')
        {
            return Err(language
                .text(
                    "Нужен точный ник игрока (до 16 символов)",
                    "An exact player name is required (up to 16 characters)",
                )
                .into());
        }
        if self.rules.len() >= 64 && !self.rules.contains_key(&name.to_ascii_lowercase()) {
            return Err(language
                .text(
                    "Не больше 64 важных игроков",
                    "At most 64 important players",
                )
                .into());
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
                (friend).then_some(Rule {
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
            friends: true,
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
    fn hsv_turns_the_primaries_and_back() {
        assert_eq!(hsv(0.0, 1.0, 1.0), 0xff0000);
        assert_eq!(hsv(60.0, 1.0, 1.0), 0xffff00);
        assert_eq!(hsv(120.0, 1.0, 1.0), 0x00ff00);
        assert_eq!(hsv(180.0, 1.0, 1.0), 0x00ffff);
        assert_eq!(hsv(240.0, 1.0, 1.0), 0x0000ff);
        assert_eq!(hsv(300.0, 1.0, 1.0), 0xff00ff);
        assert_eq!(hsv(0.0, 0.0, 1.0), 0xffffff);
        assert_eq!(hsv(0.0, 1.0, 0.0), 0x000000);
        for color in [0xff2e8c, 0x77d9b0, 0xbcb8e3, 0xffcc66, 0x102030, 0xffffff] {
            let (h, s, v) = rgb_hsv(color);
            assert_eq!(hsv(h, s, v), color, "{color:06X}");
        }
        assert_eq!(hsv(f32::NAN, f32::NAN, f32::NAN), 0x000000);
        assert_eq!(hsv(-120.0, 1.0, 1.0), 0x0000ff);
        assert_eq!(rgb_hsv(0x808080).1, 0.0);
    }
    #[test]
    fn english_errors_are_not_cyrillic() {
        let mut p = Preferences::default();
        let e = p
            .set_in("bad name", 0, true, Language::English)
            .unwrap_err();
        assert!(!e.chars().any(|c| matches!(c, 'А'..='я' | 'Ё' | 'ё')));
        for i in 0..64 {
            p.set(&format!("p{i}"), 0, true).unwrap();
        }
        let e = p.set_in("extra", 0, true, Language::English).unwrap_err();
        assert!(!e.chars().any(|c| matches!(c, 'А'..='я' | 'Ё' | 'ё')));
    }
    #[test]
    fn legacy_opt_out_cannot_hide_friends() {
        let p = Preferences::decode(r#"{"schema":1,"rules":[],"friends":false}"#).unwrap();
        assert!(p.rule("Friend", true).unwrap().pinned);
        assert!(p.rule("Stranger", false).is_none());
    }
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
