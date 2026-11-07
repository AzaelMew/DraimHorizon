// Experimental ore overlays: persisted permission, transient activation and bounded geometry.
use serde_json::{Value, json};
pub const DEFAULT_KEY: &str = "key.keyboard.o";
pub struct Ore {
    pub label: &'static str,
    pub label_en: &'static str,
    pub color: u32,
    pub blocks: &'static [&'static str],
}
pub const ORES: [Ore; 10] = [
    Ore {
        label: "Алмазы",
        label_en: "Diamonds",
        color: 0x54e7e8,
        blocks: &["minecraft:diamond_ore", "minecraft:deepslate_diamond_ore"],
    },
    Ore {
        label: "Древние обломки",
        label_en: "Ancient debris",
        color: 0xd28b76,
        blocks: &["minecraft:ancient_debris"],
    },
    Ore {
        label: "Изумруды",
        label_en: "Emeralds",
        color: 0x67e495,
        blocks: &["minecraft:emerald_ore", "minecraft:deepslate_emerald_ore"],
    },
    Ore {
        label: "Золото",
        label_en: "Gold",
        color: 0xf4ce64,
        blocks: &[
            "minecraft:gold_ore",
            "minecraft:deepslate_gold_ore",
            "minecraft:nether_gold_ore",
        ],
    },
    Ore {
        label: "Железо",
        label_en: "Iron",
        color: 0xe0c1a9,
        blocks: &["minecraft:iron_ore", "minecraft:deepslate_iron_ore"],
    },
    Ore {
        label: "Медь",
        label_en: "Copper",
        color: 0xda9567,
        blocks: &["minecraft:copper_ore", "minecraft:deepslate_copper_ore"],
    },
    Ore {
        label: "Редстоун",
        label_en: "Redstone",
        color: 0xef6474,
        blocks: &["minecraft:redstone_ore", "minecraft:deepslate_redstone_ore"],
    },
    Ore {
        label: "Лазурит",
        label_en: "Lapis lazuli",
        color: 0x7799f2,
        blocks: &["minecraft:lapis_ore", "minecraft:deepslate_lapis_ore"],
    },
    Ore {
        label: "Уголь",
        label_en: "Coal",
        color: 0xaaaaaa,
        blocks: &["minecraft:coal_ore", "minecraft:deepslate_coal_ore"],
    },
    Ore {
        label: "Кварц Незера",
        label_en: "Nether quartz",
        color: 0xf1e5dd,
        blocks: &["minecraft:nether_quartz_ore"],
    },
];
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Preferences {
    pub allowed: bool,
    pub prediction: bool,
    pub ores: u16,
    pub selected: u8,
}
impl Default for Preferences {
    fn default() -> Self {
        Self {
            allowed: false,
            prediction: false,
            ores: 3,
            selected: 0,
        }
    }
}
impl Preferences {
    pub fn normalized(mut self) -> Self {
        self.ores &= (1 << ORES.len()) - 1;
        self.selected %= ORES.len() as u8;
        self
    }
    pub fn encode(self) -> String {
        let p = self.normalized();
        json!({"schema":1,"allowed":p.allowed,"prediction":p.prediction,"ores":p.ores,"selected":p.selected}).to_string()
    }
    pub fn decode(raw: &str) -> Result<Self, String> {
        if raw.len() > 4096 {
            return Err("experimental settings too large".into());
        }
        let v: Value = serde_json::from_str(raw).map_err(|_| "invalid experimental settings")?;
        if v["schema"] != 1 {
            return Err("unsupported experimental settings".into());
        }
        Ok(Self {
            allowed: v["allowed"].as_bool().unwrap_or(false),
            prediction: v["prediction"].as_bool().unwrap_or(false),
            ores: v["ores"].as_u64().unwrap_or(3).min(u64::from(u16::MAX)) as u16,
            selected: v["selected"].as_u64().unwrap_or(0).min(255) as u8,
        }
        .normalized())
    }
    pub fn includes(self, kind: usize) -> bool {
        kind < ORES.len() && self.ores & (1 << kind) != 0
    }
    pub fn classify(self, id: &str) -> Option<u8> {
        ORES.iter()
            .enumerate()
            .find(|(i, o)| self.includes(*i) && o.blocks.contains(&id))
            .map(|(i, _)| i as u8)
    }
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Mark {
    pub position: [i32; 3],
    pub kind: u8,
    pub predicted: bool,
}
pub const MAX_MARKS: usize = 256;
pub const RANGE: f64 = 128.0;
pub fn nearest(mut marks: Vec<Mark>, player: [f64; 3]) -> Vec<Mark> {
    let distance = |m: &Mark| {
        m.position
            .iter()
            .zip(player)
            .map(|(a, b)| (f64::from(*a) + 0.5 - b).powi(2))
            .sum::<f64>()
    };
    marks.retain(|m| usize::from(m.kind) < ORES.len() && distance(m) <= RANGE * RANGE);
    marks.sort_by(|a, b| {
        distance(a)
            .total_cmp(&distance(b))
            .then(a.position.cmp(&b.position))
    });
    marks.dedup_by_key(|m| m.position);
    marks.truncate(MAX_MARKS);
    marks
}
pub fn edges(mark: &Mark) -> Vec<([f64; 3], [f64; 3])> {
    let base = mark.position.map(f64::from);
    let mut lines = Vec::with_capacity(if mark.predicted { 24 } else { 12 });
    for axis in 0..3 {
        for a in 0..2 {
            for b in 0..2 {
                let mut start = base;
                start[(axis + 1) % 3] += a as f64;
                start[(axis + 2) % 3] += b as f64;
                let mut end = start;
                end[axis] += 1.0;
                if mark.predicted {
                    let mut x = start;
                    let mut y = end;
                    x[axis] += 0.3;
                    y[axis] -= 0.3;
                    lines.push((start, x));
                    lines.push((y, end));
                } else {
                    lines.push((start, end));
                }
            }
        }
    }
    lines
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn english_ore_labels_are_not_cyrillic() {
        for ore in ORES {
            assert!(
                !ore.label_en
                    .chars()
                    .any(|c| matches!(c, 'А'..='я' | 'Ё' | 'ё'))
            );
        }
    }
    #[test]
    fn default_never_activates_and_settings_roundtrip() {
        let p = Preferences::default();
        assert!(!p.allowed && !p.prediction);
        assert_eq!(Preferences::decode(&p.encode()).unwrap(), p);
        assert!(!Preferences::decode(r#"{"schema":1}"#).unwrap().allowed);
        assert!(Preferences::decode(r#"{"schema":2}"#).is_err());
        assert_eq!(p.classify("minecraft:deepslate_diamond_ore"), Some(0));
        assert_eq!(p.classify("minecraft:ancient_debris"), Some(1));
        assert_eq!(p.classify("minecraft:gold_ore"), None);
    }
    #[test]
    fn highlights_are_bounded_and_predictions_have_a_different_shape() {
        let m = Mark {
            position: [-5, 12, -5],
            kind: 1,
            predicted: false,
        };
        assert_eq!(edges(&m).len(), 12);
        assert_eq!(
            edges(&Mark {
                predicted: true,
                ..m.clone()
            })
            .len(),
            24
        );
        let marks = (0..400)
            .map(|i| Mark {
                position: [i % 20, 12, i / 20],
                ..m.clone()
            })
            .collect();
        assert_eq!(nearest(marks, [0.0, 12.0, 0.0]).len(), MAX_MARKS);
        assert!(
            nearest(
                vec![Mark {
                    position: [10000, 0, 0],
                    ..m
                }],
                [0.0; 3]
            )
            .is_empty()
        );
    }
}
