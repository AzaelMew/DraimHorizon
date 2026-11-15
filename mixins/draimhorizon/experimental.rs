// Experimental ore overlays: persisted permission, transient activation and bounded geometry.
use serde_json::{Value, json};
pub const DEFAULT_KEY: &str = "key.keyboard.o";
pub struct Ore {
    pub label: &'static str,
    pub label_en: &'static str,
    pub color: u32,
    pub blocks: &'static [&'static str],
}
/// The first entries are ores the miner can collect; later ones are highlight-only.
pub const MINABLE: usize = 10;
pub const ORES: [Ore; 14] = [
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
    Ore {
        label: "Сундуки",
        label_en: "Chests",
        color: 0xd8a24a,
        blocks: &["minecraft:chest", "minecraft:trapped_chest"],
    },
    Ore {
        label: "Эндер-сундук",
        label_en: "Ender chest",
        color: 0x2fb7a3,
        blocks: &["minecraft:ender_chest"],
    },
    Ore {
        label: "Портал Энда",
        label_en: "End portal",
        color: 0x9a6bff,
        blocks: &["minecraft:end_portal", "minecraft:end_portal_frame"],
    },
    Ore {
        label: "Портал Незера",
        label_en: "Nether portal",
        color: 0xc23bd6,
        blocks: &["minecraft:nether_portal"],
    },
];
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Preferences {
    pub allowed: bool,
    pub prediction: bool,
    pub prediction_radius: u8,
    pub ores: u16,
    pub selected: u8,
}
impl Default for Preferences {
    fn default() -> Self {
        Self {
            allowed: false,
            prediction: false,
            prediction_radius: 0,
            ores: 3,
            selected: 0,
        }
    }
}
impl Preferences {
    pub fn normalized(mut self) -> Self {
        self.ores &= (1 << ORES.len()) - 1;
        self.selected %= ORES.len() as u8;
        self.prediction_radius = self.prediction_radius.min(2);
        self
    }
    pub fn encode(self) -> String {
        let p = self.normalized();
        json!({"schema":1,"allowed":p.allowed,"prediction":p.prediction,"prediction_radius":p.prediction_radius,"ores":p.ores,"selected":p.selected}).to_string()
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
            prediction_radius: v["prediction_radius"].as_u64().unwrap_or(0).min(2) as u8,
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
/// Row-major chunk square, bounded even for malformed saved settings.
pub fn prediction_chunks(center: (i32, i32), radius: u8) -> Vec<(i32, i32)> {
    let radius = i32::from(radius.min(2));
    (-radius..=radius)
        .flat_map(|z| (-radius..=radius).map(move |x| (center.0 + x, center.1 + z)))
        .collect()
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Mark {
    pub position: [i32; 3],
    pub kind: u8,
    pub predicted: bool,
}
pub const MAX_MARKS: usize = 256;
pub const RANGE: f64 = 128.0;
/// Chunks read around the player: the whole highlight range, so flying shows what is ahead.
pub const SCAN_CHUNKS: i32 = 8;
/// Each highlight-only kind is capped separately so a base full of chests cannot hide a portal.
pub const MAX_EXTRA_MARKS: usize = 64;
/// Item icons of the highlight-only kinds (the menu tiles); the portals show their frame and
/// obsidian items, though the highlighted blocks are the portals themselves.
const EXTRA_ICONS: [&str; ORES.len() - MINABLE] = [
    "minecraft:chest",
    "minecraft:ender_chest",
    "minecraft:end_portal_frame",
    "minecraft:obsidian",
];
pub fn icon(kind: u8) -> Option<&'static str> {
    EXTRA_ICONS
        .get(usize::from(kind).checked_sub(MINABLE)?)
        .copied()
}
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
    let mut extra = [0usize; ORES.len()];
    marks.retain(|m| {
        let kind = usize::from(m.kind);
        kind < MINABLE || {
            extra[kind] += 1;
            extra[kind] <= MAX_EXTRA_MARKS
        }
    });
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
    fn prediction_range_migrates_roundtrips_and_bounds_chunk_squares() {
        assert_eq!(
            Preferences::decode(r#"{"schema":1}"#)
                .unwrap()
                .prediction_radius,
            0
        );
        for radius in 0..=2 {
            let p = Preferences {
                prediction_radius: radius,
                ..Preferences::default()
            };
            assert_eq!(Preferences::decode(&p.encode()).unwrap(), p);
            let chunks = prediction_chunks((-3, -7), radius);
            let r = i32::from(radius);
            assert_eq!(chunks.len(), ((2 * r + 1).pow(2)) as usize);
            assert_eq!(chunks.first(), Some(&(-3 - r, -7 - r)));
            assert_eq!(chunks.last(), Some(&(-3 + r, -7 + r)));
            assert!(chunks.contains(&(-3, -7)));
        }
        assert_eq!(prediction_chunks((0, 0), 255).len(), 25);
        assert_eq!(
            Preferences::decode(r#"{"schema":1,"prediction_radius":999}"#)
                .unwrap()
                .prediction_radius,
            2
        );
    }
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
    fn container_and_portal_blocks_have_their_own_kinds_when_enabled() {
        let p = Preferences {
            ores: (1 << ORES.len()) - 1,
            ..Preferences::default()
        };
        for (id, kind) in [
            ("minecraft:chest", 10),
            ("minecraft:trapped_chest", 10),
            ("minecraft:ender_chest", 11),
            ("minecraft:end_portal", 12),
            ("minecraft:end_portal_frame", 12),
            ("minecraft:nether_portal", 13),
        ] {
            assert_eq!(p.classify(id), Some(kind), "{id}");
        }
        assert_eq!(Preferences::default().classify("minecraft:chest"), None);
    }
    #[test]
    fn menu_icons_of_new_kinds_and_chests_do_not_hide_portals() {
        assert_eq!(icon(0), None);
        assert_eq!(icon(10), Some("minecraft:chest"));
        assert_eq!(icon(11), Some("minecraft:ender_chest"));
        assert_eq!(icon(12), Some("minecraft:end_portal_frame"));
        assert_eq!(icon(13), Some("minecraft:obsidian"));
        let mut marks: Vec<Mark> = (0..200)
            .map(|i| Mark {
                position: [i % 20, 12, i / 20],
                kind: 10,
                predicted: false,
            })
            .collect();
        marks.push(Mark {
            position: [100, 12, 0],
            kind: 13,
            predicted: false,
        });
        let kept = nearest(marks, [0.0, 12.0, 0.0]);
        assert_eq!(
            kept.iter().filter(|m| m.kind == 10).count(),
            MAX_EXTRA_MARKS
        );
        assert!(kept.iter().any(|m| m.kind == 13));
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
