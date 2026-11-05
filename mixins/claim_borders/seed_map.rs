// Seed-map settings and projection shared by tests and the native UI.
use super::claim_borders_map::View;
use serde_json::{Value, json};
pub const SEED: i64 = -3292718948513116954;
pub const DIMENSIONS: [&str; 3] = [
    "minecraft:overworld",
    "minecraft:the_nether",
    "minecraft:the_end",
];
pub const STRUCTURES: [(&str, &str, u32); 18] = [
    ("village", "Деревни", 0xf3cb73),
    ("desert_pyramid", "Пирамиды", 0xe7bd75),
    ("jungle_pyramid", "Храмы джунглей", 0x78bf73),
    ("swamp_hut", "Хижины ведьм", 0xb88acb),
    ("igloo", "Иглу", 0xd0e8ef),
    ("pillager_outpost", "Аванпосты", 0xdf8a68),
    ("mansion", "Особняки", 0x967b64),
    ("monument", "Монументы", 0x68cfc0),
    ("shipwreck", "Корабли", 0xb89c75),
    ("ocean_ruin", "Руины океана", 0x78a7bb),
    ("ruined_portal", "Разруш. порталы", 0xc785db),
    ("ancient_city", "Древние города", 0x54a2b4),
    ("trial_chambers", "Камеры испытаний", 0xd58c58),
    ("buried_treasure", "Сокровища", 0xf2db6d),
    ("fortress", "Крепости Незера", 0xe57569),
    ("bastion_remnant", "Бастионы", 0xc1a087),
    ("stronghold", "Цитадели", 0x91bab6),
    ("end_city", "Города Энда", 0xc29ce8),
];
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Preferences {
    pub biomes: bool,
    pub biome: Option<String>,
    pub structures: u32,
    pub claims: bool,
}
impl Default for Preferences {
    fn default() -> Self {
        Self {
            biomes: true,
            biome: None,
            structures: (1 << STRUCTURES.len()) - 1,
            claims: true,
        }
    }
}
impl Preferences {
    pub fn shows(&self, id: &str) -> bool {
        structure_index(id).is_some_and(|i| self.structures & (1 << i) != 0)
    }
    pub fn encode(&self) -> String {
        json!({"schema":1,"biomes":self.biomes,"biome":self.biome,"structures":self.structures,"claims":self.claims}).to_string()
    }
    pub fn decode(raw: &str) -> Result<Self, String> {
        if raw.len() > 4096 {
            return Err("seed map settings too large".into());
        }
        let v: Value = serde_json::from_str(raw).map_err(|_| "invalid seed map settings")?;
        if v["schema"] != 1 {
            return Err("unsupported seed map settings".into());
        }
        let d = Self::default();
        Ok(Self {
            biomes: v["biomes"].as_bool().unwrap_or(d.biomes),
            biome: v["biome"]
                .as_str()
                .filter(|s| s.len() <= 128 && s.starts_with("minecraft:"))
                .map(str::to_owned),
            structures: v["structures"].as_u64().unwrap_or(u64::from(d.structures)) as u32
                & d.structures,
            claims: v["claims"].as_bool().unwrap_or(d.claims),
        })
    }
}
pub fn structure_index(id: &str) -> Option<usize> {
    let id = id.strip_prefix("minecraft:").unwrap_or(id);
    STRUCTURES.iter().position(|(key, _, _)| {
        id == *key
            || id
                .strip_prefix(key)
                .is_some_and(|tail| tail.starts_with('_'))
    })
}
pub fn panel_rects(size: [f32; 2]) -> [[f32; 4]; 3] {
    let width = ((size[0] - 148.0) / 3.0).max(1.0);
    std::array::from_fn(|i| {
        [
            6.0 + i as f32 * (width + 4.0),
            53.0,
            width,
            (size[1] - 91.0).max(1.0),
        ]
    })
}
pub fn contains(r: [f32; 4], p: [f32; 2]) -> bool {
    p[0] >= r[0] && p[1] >= r[1] && p[0] < r[0] + r[2] && p[1] < r[1] + r[3]
}
pub fn world(view: View, r: [f32; 4], mouse: [f32; 2]) -> [f64; 2] {
    view.world_aspect(
        [
            (mouse[0] - r[0]) / r[2] * 128.0,
            (mouse[1] - r[1]) / r[3] * 128.0,
        ],
        128.0,
        r[2] / r[3],
    )
}
/// Only random-spread sets with a bounded number of regions are enumerated.
/// At continental zoom the UI requests a closer view instead of truncating markers.
pub fn region_bounds(view: View, aspect: f32, spacing: i32) -> Option<([i32; 2], [i32; 2])> {
    if spacing <= 0 || !aspect.is_finite() || aspect <= 0.0 {
        return None;
    }
    let a = view.world_aspect([0.0, 0.0], 128.0, aspect);
    let b = view.world_aspect([128.0, 128.0], 128.0, aspect);
    let region = f64::from(spacing) * 16.0;
    let lo = a.map(|v| (v / region).floor() as i32);
    let hi = b.map(|v| (v / region).floor() as i32);
    let count =
        (i64::from(hi[0]) - i64::from(lo[0]) + 1) * (i64::from(hi[1]) - i64::from(lo[1]) + 1);
    (count > 0 && count <= 256).then_some((lo, hi))
}
/// Stable, readable biome colours; names come from the bundled generator.
pub fn biome_color(id: &str) -> u32 {
    if id.contains("ocean") {
        if id.contains("warm") {
            0x4095aa
        } else if id.contains("frozen") {
            0x799da9
        } else {
            0x365b83
        }
    } else if id.contains("river") {
        0x477caa
    } else if id.contains("end_highlands") {
        0xbdbb7b
    } else if id.contains("end_midlands") || id.ends_with("the_end") {
        0x96945f
    } else if id.contains("end_barrens") {
        0x6e6a51
    } else if id.contains("end_islands") {
        0x555244
    } else if id.contains("crimson") {
        0xa23e4b
    } else if id.contains("warped") {
        0x337f79
    } else if id.contains("soul_sand") {
        0x786357
    } else if id.contains("basalt") {
        0x55505a
    } else if id.contains("nether") {
        0x8d4840
    } else if id.contains("desert") || id.contains("beach") {
        0xd6c587
    } else if id.contains("badlands") {
        0xb97650
    } else if id.contains("snow")
        || id.contains("frozen")
        || id.contains("ice")
        || id.contains("grove")
    {
        0xc4d7d8
    } else if id.contains("peak") || id.contains("stony") {
        0x92938d
    } else if id.contains("jungle") {
        0x39734b
    } else if id.contains("swamp") {
        0x586f55
    } else if id.contains("dark_forest") {
        0x3b5540
    } else if id.contains("forest") || id.contains("taiga") {
        0x527b57
    } else if id.contains("savanna") {
        0xa5a261
    } else if id.contains("cherry") {
        0xc39fae
    } else if id.contains("mushroom") {
        0x947ba1
    } else if id.contains("deep_dark") {
        0x304c53
    } else {
        0x7da16a
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn exact_seed_filters_and_old_settings() {
        assert_eq!(SEED.to_string(), "-3292718948513116954");
        let mut p = Preferences::decode(r#"{"schema":1}"#).unwrap();
        assert_eq!(p, Preferences::default());
        assert!(p.shows("minecraft:village_snowy"));
        assert!(p.shows("minecraft:ruined_portal_nether"));
        assert!(!p.shows("minecraft:not_a_structure"));
        p.structures &= !1;
        p.biome = Some("minecraft:cherry_grove".into());
        assert!(!p.shows("minecraft:village_plains"));
        assert_eq!(Preferences::decode(&p.encode()).unwrap(), p);
        assert!(Preferences::decode(r#"{"schema":2}"#).is_err());
    }
    #[test]
    fn three_panels_project_independently_and_bound_work() {
        let panels = panel_rects([640.0, 360.0]);
        for (i, r) in panels.iter().enumerate() {
            assert!(r[0] + r[2] < 640.0 - 120.0);
            let view = View {
                center: [-3628.0, -22708.0],
                blocks_per_pixel: 16.0,
            };
            assert_eq!(
                world(view, *r, [r[0] + r[2] / 2.0, r[1] + r[3] / 2.0]),
                view.center
            );
            assert_eq!(
                panels
                    .iter()
                    .filter(|p| contains(**p, [r[0] + 1.0, r[1] + 1.0]))
                    .count(),
                1
            );
            assert!(region_bounds(view, r[2] / r[3], 32).is_some());
            assert!(
                region_bounds(
                    View {
                        blocks_per_pixel: 524288.0,
                        ..view
                    },
                    r[2] / r[3],
                    1
                )
                .is_none()
            );
            assert!(DIMENSIONS[i].starts_with("minecraft:"));
        }
    }
}
