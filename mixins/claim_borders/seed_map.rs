// Vanilla structure catalog, persistent filters and shared map geometry.
use super::claim_borders_map::View;
use serde_json::{Value, json};
pub const SEED: i64 = -3292718948513116954;
pub const DIMENSIONS: [&str; 3] = [
    "minecraft:overworld",
    "minecraft:the_nether",
    "minecraft:the_end",
];
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Group {
    Surface,
    Underground,
    Ocean,
    Nether,
    End,
}
impl Group {
    pub fn label(self) -> &'static str {
        match self {
            Self::Surface => "Суша",
            Self::Underground => "Под землёй",
            Self::Ocean => "Океан",
            Self::Nether => "Незер",
            Self::End => "Край",
        }
    }
    pub fn dimension(self) -> usize {
        match self {
            Self::Nether => 1,
            Self::End => 2,
            _ => 0,
        }
    }
}
pub const GROUPS: [Group; 5] = [
    Group::Surface,
    Group::Underground,
    Group::Ocean,
    Group::Nether,
    Group::End,
];
pub struct Structure {
    pub id: &'static str,
    pub label: &'static str,
    pub group: Group,
    pub item: &'static str,
    pub color: u32,
}
pub const STRUCTURES: [Structure; 34] = [
    Structure {
        id: "village_plains",
        label: "Равнинная деревня",
        group: Group::Surface,
        item: "minecraft:bell",
        color: 0xf3cb73,
    },
    Structure {
        id: "desert_pyramid",
        label: "Пустынная пирамида",
        group: Group::Surface,
        item: "minecraft:chiseled_sandstone",
        color: 0xe7bd75,
    },
    Structure {
        id: "jungle_pyramid",
        label: "Храм джунглей",
        group: Group::Surface,
        item: "minecraft:mossy_cobblestone",
        color: 0x78bf73,
    },
    Structure {
        id: "swamp_hut",
        label: "Хижина ведьмы",
        group: Group::Surface,
        item: "minecraft:cauldron",
        color: 0xb88acb,
    },
    Structure {
        id: "igloo",
        label: "Иглу",
        group: Group::Surface,
        item: "minecraft:snow_block",
        color: 0xd0e8ef,
    },
    Structure {
        id: "pillager_outpost",
        label: "Аванпост",
        group: Group::Surface,
        item: "minecraft:crossbow",
        color: 0xdf8a68,
    },
    Structure {
        id: "mansion",
        label: "Лесной особняк",
        group: Group::Surface,
        item: "minecraft:dark_oak_planks",
        color: 0x967b64,
    },
    Structure {
        id: "monument",
        label: "Океанский монумент",
        group: Group::Ocean,
        item: "minecraft:prismarine",
        color: 0x68cfc0,
    },
    Structure {
        id: "shipwreck",
        label: "Затонувший корабль",
        group: Group::Ocean,
        item: "minecraft:oak_boat",
        color: 0xb89c75,
    },
    Structure {
        id: "ocean_ruin_cold",
        label: "Холодные руины",
        group: Group::Ocean,
        item: "minecraft:stone_bricks",
        color: 0x78a7bb,
    },
    Structure {
        id: "ruined_portal",
        label: "Разрушенный портал",
        group: Group::Surface,
        item: "minecraft:obsidian",
        color: 0xc785db,
    },
    Structure {
        id: "ancient_city",
        label: "Древний город",
        group: Group::Underground,
        item: "minecraft:sculk_shrieker",
        color: 0x54a2b4,
    },
    Structure {
        id: "trial_chambers",
        label: "Камеры испытаний",
        group: Group::Underground,
        item: "minecraft:trial_key",
        color: 0xd58c58,
    },
    Structure {
        id: "buried_treasure",
        label: "Зарытый клад",
        group: Group::Ocean,
        item: "minecraft:chest",
        color: 0xf2db6d,
    },
    Structure {
        id: "fortress",
        label: "Крепость Незера",
        group: Group::Nether,
        item: "minecraft:nether_bricks",
        color: 0xe57569,
    },
    Structure {
        id: "bastion_remnant",
        label: "Бастион",
        group: Group::Nether,
        item: "minecraft:gilded_blackstone",
        color: 0xc1a087,
    },
    Structure {
        id: "stronghold",
        label: "Цитадель",
        group: Group::Underground,
        item: "minecraft:ender_eye",
        color: 0x91bab6,
    },
    Structure {
        id: "end_city",
        label: "Город Края",
        group: Group::End,
        item: "minecraft:purpur_block",
        color: 0xc29ce8,
    },
    Structure {
        id: "mineshaft",
        label: "Заброшенная шахта",
        group: Group::Underground,
        item: "minecraft:rail",
        color: 0xc3a778,
    },
    Structure {
        id: "mineshaft_mesa",
        label: "Шахта пустошей",
        group: Group::Underground,
        item: "minecraft:gold_ore",
        color: 0xe1af65,
    },
    Structure {
        id: "nether_fossil",
        label: "Ископаемое Незера",
        group: Group::Nether,
        item: "minecraft:bone_block",
        color: 0xddd3bd,
    },
    Structure {
        id: "trail_ruins",
        label: "Руины троп",
        group: Group::Surface,
        item: "minecraft:brush",
        color: 0xbf9065,
    },
    Structure {
        id: "village_desert",
        label: "Пустынная деревня",
        group: Group::Surface,
        item: "minecraft:sandstone",
        color: 0xe6ca81,
    },
    Structure {
        id: "village_savanna",
        label: "Деревня саванны",
        group: Group::Surface,
        item: "minecraft:acacia_planks",
        color: 0xc8875e,
    },
    Structure {
        id: "village_snowy",
        label: "Снежная деревня",
        group: Group::Surface,
        item: "minecraft:white_bed",
        color: 0xcbe5ef,
    },
    Structure {
        id: "village_taiga",
        label: "Таёжная деревня",
        group: Group::Surface,
        item: "minecraft:spruce_planks",
        color: 0x9b7c54,
    },
    Structure {
        id: "shipwreck_beached",
        label: "Выброшенный корабль",
        group: Group::Ocean,
        item: "minecraft:spruce_boat",
        color: 0xc1a37a,
    },
    Structure {
        id: "ocean_ruin_warm",
        label: "Тёплые руины",
        group: Group::Ocean,
        item: "minecraft:sandstone",
        color: 0xd4be84,
    },
    Structure {
        id: "ruined_portal_desert",
        label: "Портал пустыни",
        group: Group::Surface,
        item: "minecraft:crying_obsidian",
        color: 0xd7aa79,
    },
    Structure {
        id: "ruined_portal_jungle",
        label: "Портал джунглей",
        group: Group::Surface,
        item: "minecraft:jungle_leaves",
        color: 0x76af72,
    },
    Structure {
        id: "ruined_portal_mountain",
        label: "Горный портал",
        group: Group::Surface,
        item: "minecraft:stone",
        color: 0xa5a4b0,
    },
    Structure {
        id: "ruined_portal_nether",
        label: "Портал Незера",
        group: Group::Nether,
        item: "minecraft:flint_and_steel",
        color: 0xe09b9f,
    },
    Structure {
        id: "ruined_portal_ocean",
        label: "Океанский портал",
        group: Group::Ocean,
        item: "minecraft:magma_block",
        color: 0x7dacce,
    },
    Structure {
        id: "ruined_portal_swamp",
        label: "Болотный портал",
        group: Group::Surface,
        item: "minecraft:mangrove_roots",
        color: 0x92a57c,
    },
];
const LEGACY: [&str; 18] = [
    "village",
    "desert_pyramid",
    "jungle_pyramid",
    "swamp_hut",
    "igloo",
    "pillager_outpost",
    "mansion",
    "monument",
    "shipwreck",
    "ocean_ruin",
    "ruined_portal",
    "ancient_city",
    "trial_chambers",
    "buried_treasure",
    "fortress",
    "bastion_remnant",
    "stronghold",
    "end_city",
];
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Preferences {
    pub biomes: bool,
    pub biome: Option<String>,
    pub structures: u64,
    pub claims: bool,
    pub dimension: usize,
}
impl Default for Preferences {
    fn default() -> Self {
        Self {
            biomes: true,
            biome: None,
            structures: (1u64 << STRUCTURES.len()) - 1,
            claims: true,
            dimension: 0,
        }
    }
}
impl Preferences {
    pub fn same_biomes(&self, other: &Self) -> bool {
        self.biomes == other.biomes && self.biome == other.biome
    }
    pub fn same_layers(&self, other: &Self) -> bool {
        self.same_biomes(other) && self.structures == other.structures
    }
    pub fn shows(&self, id: &str) -> bool {
        structure_index(id).is_some_and(|i| self.structures & (1u64 << i) != 0)
    }
    pub fn toggle(&mut self, i: usize) {
        if i < STRUCTURES.len() {
            self.structures ^= 1u64 << i;
        }
    }
    pub fn encode(&self) -> String {
        json!({"schema":2,"biomes":self.biomes,"biome":self.biome,"structures":self.structures,"claims":self.claims,"dimension":self.dimension.min(2)}).to_string()
    }
    pub fn decode(raw: &str) -> Result<Self, String> {
        if raw.len() > 4096 {
            return Err("seed map settings too large".into());
        }
        let v: Value = serde_json::from_str(raw).map_err(|_| "invalid seed map settings")?;
        if v["schema"] != 1 && v["schema"] != 2 {
            return Err("unsupported seed map settings".into());
        }
        let d = Self::default();
        let mut structures = v["structures"].as_u64().unwrap_or(d.structures) & d.structures;
        if v["schema"] == 1 {
            let old = v["structures"].as_u64().unwrap_or((1 << 18) - 1);
            structures = d.structures;
            for (i, s) in STRUCTURES.iter().enumerate() {
                if let Some(bit) = LEGACY.iter().position(|k| {
                    s.id == *k
                        || s.id
                            .strip_prefix(k)
                            .is_some_and(|tail| tail.starts_with('_'))
                }) && old & (1 << bit) == 0
                {
                    structures &= !(1u64 << i);
                }
            }
        }
        Ok(Self {
            biomes: v["biomes"].as_bool().unwrap_or(true),
            biome: v["biome"]
                .as_str()
                .filter(|s| s.len() <= 128 && s.starts_with("minecraft:"))
                .map(str::to_owned),
            structures,
            claims: v["claims"].as_bool().unwrap_or(true),
            dimension: v["dimension"].as_u64().unwrap_or(0).min(2) as usize,
        })
    }
}
pub fn structure_index(id: &str) -> Option<usize> {
    let id = id.strip_prefix("minecraft:").unwrap_or(id);
    STRUCTURES.iter().position(|s| s.id == id)
}
pub fn filter_indices(dimension: usize, group: Option<Group>, search: &str) -> Vec<usize> {
    let query = search.to_lowercase();
    STRUCTURES
        .iter()
        .enumerate()
        .filter(|(_, s)| {
            s.group.dimension() == dimension
                && group.is_none_or(|g| s.group == g)
                && (query.is_empty()
                    || s.label.to_lowercase().contains(&query)
                    || s.id.contains(&query))
        })
        .map(|(i, _)| i)
        .collect()
}
pub fn map_rect(size: [f32; 2]) -> [f32; 4] {
    let top = if size[0] < 520.0 { 56.0 } else { 30.0 };
    [0.0, top, size[0].max(1.0), (size[1] - top - 32.0).max(1.0)]
}
pub fn dimension_rect(size: [f32; 2], i: usize) -> [f32; 4] {
    if size[0] < 520.0 {
        let w = (size[0] - 16.0) / 3.0;
        [6.0 + i as f32 * (w + 2.0), 31.0, w, 21.0]
    } else {
        let w = ((size[0] - 250.0) / 3.0).min(100.0);
        [size[0] - 32.0 - (3 - i) as f32 * (w + 3.0), 4.0, w, 22.0]
    }
}
pub fn tool_rect(size: [f32; 2], index: usize) -> [f32; 4] {
    let r = map_rect(size);
    let rows = ((r[3] - 24.0) / 28.0).max(1.0) as usize;
    [
        8.0 + (index / rows) as f32 * 28.0,
        r[1] + 12.0 + (index % rows) as f32 * 28.0,
        24.0,
        24.0,
    ]
}
pub fn filter_rect(size: [f32; 2]) -> [f32; 4] {
    let w = (size[0] - 16.0).clamp(160.0, 300.0);
    if size[1] < 300.0 {
        [size[0] - w - 8.0, 4.0, w, (size[1] - 8.0).max(40.0)]
    } else {
        let top = map_rect(size)[1] + 8.0;
        [size[0] - w - 8.0, top, w, (size[1] - top - 40.0).max(40.0)]
    }
}
pub fn filter_rows(size: [f32; 2]) -> usize {
    ((filter_rect(size)[3] - 118.0) / 46.0).max(1.0) as usize
}
pub fn filter_chip_rect(size: [f32; 2], n: usize) -> [f32; 4] {
    let r = filter_rect(size);
    let w = (r[2] - 20.0) / 4.0;
    [
        r[0] + 8.0 + (n % 4) as f32 * (w + 1.0),
        r[1] + 86.0 + (n / 4) as f32 * 46.0,
        w,
        43.0,
    ]
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
/// Hard bound on placement checks, including dense mineshaft/fossil sets.
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
    (count > 0 && count <= 65536).then_some((lo, hi))
}
pub fn biome_dimension(id: &str) -> usize {
    match id.strip_prefix("minecraft:").unwrap_or(id) {
        "nether_wastes" | "soul_sand_valley" | "crimson_forest" | "warped_forest"
        | "basalt_deltas" => 1,
        "the_end" | "end_highlands" | "end_midlands" | "end_barrens" | "small_end_islands" => 2,
        _ => 0,
    }
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
    fn all_vanilla_variants_and_old_filters_are_preserved() {
        let mut names = std::collections::BTreeSet::new();
        for s in STRUCTURES {
            assert!(names.insert(s.id));
            assert!(s.item.starts_with("minecraft:"));
            assert!(Preferences::default().shows(s.id));
        }
        assert_eq!(names.len(), 34);
        assert!(names.contains("mineshaft_mesa"));
        assert!(names.contains("nether_fossil"));
        assert!(names.contains("trail_ruins"));
        let old = Preferences::decode(r#"{"schema":1,"structures":0}"#).unwrap();
        assert!(!old.shows("village_snowy"));
        assert!(!old.shows("ruined_portal_nether"));
        assert!(old.shows("mineshaft"));
        let p = Preferences::decode(r#"{"schema":1}"#).unwrap();
        assert_eq!(p, Preferences::default());
        assert_eq!(Preferences::decode(&p.encode()).unwrap(), p);
        assert!(Preferences::decode(r#"{"schema":99}"#).is_err());
    }
    #[test]
    fn controls_and_filter_chips_fit_small_scaled_windows() {
        for width in [320.0, 480.0, 640.0, 960.0] {
            for height in [180.0, 240.0, 360.0, 540.0] {
                let size = [width, height];
                let map = map_rect(size);
                for i in 0..8 {
                    let r = tool_rect(size, i);
                    assert!(r[0] >= map[0] && r[1] >= map[1]);
                    assert!(r[0] + r[2] <= width && r[1] + r[3] <= map[1] + map[3]);
                }
                let popup = filter_rect(size);
                assert!(
                    popup[0] >= 0.0
                        && popup[1] >= 0.0
                        && popup[0] + popup[2] <= width
                        && popup[1] + popup[3] <= height
                );
                for n in 0..filter_rows(size) * 4 {
                    let r = filter_chip_rect(size, n);
                    assert!(r[0] >= popup[0] && r[0] + r[2] <= popup[0] + popup[2]);
                    assert!(r[1] + r[3] <= popup[1] + popup[3] - 28.0);
                }
            }
        }
    }
    #[test]
    fn filters_and_fullscreen_geometry() {
        let p = Preferences::default();
        assert!(!p.shows("minecraft:unknown"));
        assert!(p.shows("minecraft:shipwreck_beached"));
        let result = filter_indices(1, None, "");
        assert!(result.iter().any(|&i| STRUCTURES[i].id == "nether_fossil"));
        assert!(result.iter().all(|&i| STRUCTURES[i].group.dimension() == 1));
        assert_eq!(
            filter_indices(0, Some(Group::Underground), "шахта").len(),
            2
        );
        let r = map_rect([640.0, 360.0]);
        let v = View {
            center: [-3628.0, -22708.0],
            blocks_per_pixel: 16.0,
        };
        assert_eq!(world(v, r, [320.0, r[1] + r[3] / 2.0]), v.center);
        assert!(region_bounds(v, r[2] / r[3], 1).is_some());
        assert!(
            region_bounds(
                View {
                    blocks_per_pixel: 524288.0,
                    ..v
                },
                r[2] / r[3],
                1
            )
            .is_none()
        );
    }
}
