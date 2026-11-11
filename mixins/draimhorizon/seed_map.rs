// Vanilla structure catalog, persistent filters and shared map geometry.
use super::claim_borders_map::View;
use claim_borders_relay_client::translation::Language;
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
        self.label_in(Language::Russian)
    }
    pub fn label_in(self, language: Language) -> &'static str {
        match self {
            Self::Surface => language.text("Суша", "Land"),
            Self::Underground => language.text("Под землёй", "Underground"),
            Self::Ocean => language.text("Океан", "Ocean"),
            Self::Nether => language.text("Незер", "Nether"),
            Self::End => language.text("Край", "End"),
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
    pub label_en: &'static str,
    pub group: Group,
    pub item: &'static str,
    pub color: u32,
}
impl Structure {
    pub fn label_in(&self, language: Language) -> &'static str {
        language.text(self.label, self.label_en)
    }
}
pub const STRUCTURES: [Structure; 35] = [
    Structure {
        id: "village_plains",
        label: "Равнинная деревня",
        label_en: "Plains Village",
        group: Group::Surface,
        item: "minecraft:bell",
        color: 0xf3cb73,
    },
    Structure {
        id: "desert_pyramid",
        label: "Пустынная пирамида",
        label_en: "Desert Pyramid",
        group: Group::Surface,
        item: "minecraft:chiseled_sandstone",
        color: 0xe7bd75,
    },
    Structure {
        id: "jungle_pyramid",
        label: "Храм джунглей",
        label_en: "Jungle Temple",
        group: Group::Surface,
        item: "minecraft:mossy_cobblestone",
        color: 0x78bf73,
    },
    Structure {
        id: "swamp_hut",
        label: "Хижина ведьмы",
        label_en: "Witch Hut",
        group: Group::Surface,
        item: "minecraft:cauldron",
        color: 0xb88acb,
    },
    Structure {
        id: "igloo",
        label: "Иглу",
        label_en: "Igloo",
        group: Group::Surface,
        item: "minecraft:snow_block",
        color: 0xd0e8ef,
    },
    Structure {
        id: "pillager_outpost",
        label: "Аванпост",
        label_en: "Pillager Outpost",
        group: Group::Surface,
        item: "minecraft:crossbow",
        color: 0xdf8a68,
    },
    Structure {
        id: "mansion",
        label: "Лесной особняк",
        label_en: "Woodland Mansion",
        group: Group::Surface,
        item: "minecraft:dark_oak_planks",
        color: 0x967b64,
    },
    Structure {
        id: "monument",
        label: "Океанский монумент",
        label_en: "Ocean Monument",
        group: Group::Ocean,
        item: "minecraft:prismarine",
        color: 0x68cfc0,
    },
    Structure {
        id: "shipwreck",
        label: "Затонувший корабль",
        label_en: "Shipwreck",
        group: Group::Ocean,
        item: "minecraft:oak_boat",
        color: 0xb89c75,
    },
    Structure {
        id: "ocean_ruin_cold",
        label: "Холодные руины",
        label_en: "Cold Ocean Ruins",
        group: Group::Ocean,
        item: "minecraft:stone_bricks",
        color: 0x78a7bb,
    },
    Structure {
        id: "ruined_portal",
        label: "Разрушенный портал",
        label_en: "Ruined Portal",
        group: Group::Surface,
        item: "minecraft:obsidian",
        color: 0xc785db,
    },
    Structure {
        id: "ancient_city",
        label: "Древний город",
        label_en: "Ancient City",
        group: Group::Underground,
        item: "minecraft:sculk_shrieker",
        color: 0x54a2b4,
    },
    Structure {
        id: "trial_chambers",
        label: "Камеры испытаний",
        label_en: "Trial Chambers",
        group: Group::Underground,
        item: "minecraft:trial_key",
        color: 0xd58c58,
    },
    Structure {
        id: "buried_treasure",
        label: "Зарытый клад",
        label_en: "Buried Treasure",
        group: Group::Ocean,
        item: "minecraft:chest",
        color: 0xf2db6d,
    },
    Structure {
        id: "fortress",
        label: "Крепость Незера",
        label_en: "Nether Fortress",
        group: Group::Nether,
        item: "minecraft:nether_bricks",
        color: 0xe57569,
    },
    Structure {
        id: "bastion_remnant",
        label: "Бастион",
        label_en: "Bastion",
        group: Group::Nether,
        item: "minecraft:gilded_blackstone",
        color: 0xc1a087,
    },
    Structure {
        id: "stronghold",
        label: "Цитадель",
        label_en: "Stronghold",
        group: Group::Underground,
        item: "minecraft:ender_eye",
        color: 0x91bab6,
    },
    Structure {
        id: "end_city",
        label: "Город Края · без корабля",
        label_en: "End City · no ship",
        group: Group::End,
        item: "minecraft:purpur_block",
        color: 0xc29ce8,
    },
    Structure {
        id: "mineshaft",
        label: "Заброшенная шахта",
        label_en: "Abandoned Mineshaft",
        group: Group::Underground,
        item: "minecraft:rail",
        color: 0xc3a778,
    },
    Structure {
        id: "mineshaft_mesa",
        label: "Шахта пустошей",
        label_en: "Badlands Mineshaft",
        group: Group::Underground,
        item: "minecraft:gold_ore",
        color: 0xe1af65,
    },
    Structure {
        id: "nether_fossil",
        label: "Ископаемое Незера",
        label_en: "Nether Fossil",
        group: Group::Nether,
        item: "minecraft:bone_block",
        color: 0xddd3bd,
    },
    Structure {
        id: "trail_ruins",
        label: "Руины троп",
        label_en: "Trail Ruins",
        group: Group::Surface,
        item: "minecraft:brush",
        color: 0xbf9065,
    },
    Structure {
        id: "village_desert",
        label: "Пустынная деревня",
        label_en: "Desert Village",
        group: Group::Surface,
        item: "minecraft:sandstone",
        color: 0xe6ca81,
    },
    Structure {
        id: "village_savanna",
        label: "Деревня саванны",
        label_en: "Savanna Village",
        group: Group::Surface,
        item: "minecraft:acacia_planks",
        color: 0xc8875e,
    },
    Structure {
        id: "village_snowy",
        label: "Снежная деревня",
        label_en: "Snowy Village",
        group: Group::Surface,
        item: "minecraft:white_bed",
        color: 0xcbe5ef,
    },
    Structure {
        id: "village_taiga",
        label: "Таёжная деревня",
        label_en: "Taiga Village",
        group: Group::Surface,
        item: "minecraft:spruce_planks",
        color: 0x9b7c54,
    },
    Structure {
        id: "shipwreck_beached",
        label: "Выброшенный корабль",
        label_en: "Beached Shipwreck",
        group: Group::Ocean,
        item: "minecraft:spruce_boat",
        color: 0xc1a37a,
    },
    Structure {
        id: "ocean_ruin_warm",
        label: "Тёплые руины",
        label_en: "Warm Ocean Ruins",
        group: Group::Ocean,
        item: "minecraft:sandstone",
        color: 0xd4be84,
    },
    Structure {
        id: "ruined_portal_desert",
        label: "Портал пустыни",
        label_en: "Desert Portal",
        group: Group::Surface,
        item: "minecraft:crying_obsidian",
        color: 0xd7aa79,
    },
    Structure {
        id: "ruined_portal_jungle",
        label: "Портал джунглей",
        label_en: "Jungle Portal",
        group: Group::Surface,
        item: "minecraft:jungle_leaves",
        color: 0x76af72,
    },
    Structure {
        id: "ruined_portal_mountain",
        label: "Горный портал",
        label_en: "Mountain Portal",
        group: Group::Surface,
        item: "minecraft:stone",
        color: 0xa5a4b0,
    },
    Structure {
        id: "ruined_portal_nether",
        label: "Портал Незера",
        label_en: "Nether Portal",
        group: Group::Nether,
        item: "minecraft:flint_and_steel",
        color: 0xe09b9f,
    },
    Structure {
        id: "ruined_portal_ocean",
        label: "Океанский портал",
        label_en: "Ocean Portal",
        group: Group::Ocean,
        item: "minecraft:magma_block",
        color: 0x7dacce,
    },
    Structure {
        id: "ruined_portal_swamp",
        label: "Болотный портал",
        label_en: "Swamp Portal",
        group: Group::Surface,
        item: "minecraft:mangrove_roots",
        color: 0x92a57c,
    },
    Structure {
        id: "end_city_ship",
        label: "Город с элитрами · корабль",
        label_en: "End City · ship with elytra",
        group: Group::End,
        item: "minecraft:elytra",
        color: 0xead88a,
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
        json!({"schema":3,"biomes":self.biomes,"biome":self.biome,"structures":self.structures,"claims":self.claims,"dimension":self.dimension.min(2)}).to_string()
    }
    pub fn decode(raw: &str) -> Result<Self, String> {
        if raw.len() > 4096 {
            return Err("seed map settings too large".into());
        }
        let v: Value = serde_json::from_str(raw).map_err(|_| "invalid seed map settings")?;
        if v["schema"] != 1 && v["schema"] != 2 && v["schema"] != 3 {
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
        if v["schema"] == 2 && structures & (1 << structure_index("end_city").unwrap()) != 0 {
            structures |= 1 << structure_index("end_city_ship").unwrap();
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
                    || s.label_en.to_lowercase().contains(&query)
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
    let width = if size[0] >= 640.0 && size[1] >= 320.0 {
        124.0
    } else {
        24.0
    };
    [
        8.0 + (index / rows) as f32 * (width + 4.0),
        r[1] + 12.0 + (index % rows) as f32 * 28.0,
        width,
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
/// Filter chip height: an icon and up to [`CHIP_LINES`] centred label rows.
pub const CHIP_HEIGHT: f32 = 51.0;
pub const CHIP_PITCH: f32 = CHIP_HEIGHT + 2.0;
pub const CHIP_LINES: usize = 3;
pub const LINE_HEIGHT: f32 = 9.0;
pub fn filter_rows(size: [f32; 2]) -> usize {
    ((filter_rect(size)[3] - 118.0) / CHIP_PITCH).max(1.0) as usize
}
pub fn filter_chip_rect(size: [f32; 2], n: usize) -> [f32; 4] {
    let r = filter_rect(size);
    let w = (r[2] - 20.0) / 4.0;
    [
        r[0] + 8.0 + (n % 4) as f32 * (w + 1.0),
        r[1] + 86.0 + (n / 4) as f32 * CHIP_PITCH,
        w,
        CHIP_HEIGHT,
    ]
}
/// Breaks `text` into at most `max_lines` rows no wider than `width`, only at spaces.
/// A word wider than a row, or text that does not fit in the last row, ends in "…".
pub fn wrap_words(
    text: &str,
    width: f32,
    max_lines: usize,
    measure: &dyn Fn(&str) -> f32,
) -> Vec<String> {
    let clip = |line: &str| {
        if measure(line) <= width {
            return line.to_owned();
        }
        let mut out = String::new();
        for c in line.chars() {
            out.push(c);
            if measure(&format!("{out}…")) > width {
                out.pop();
                break;
            }
        }
        out.push('…');
        out
    };
    let mut lines: Vec<String> = Vec::new();
    let mut rest = text.split_whitespace().peekable();
    while let Some(word) = rest.next() {
        let mut line = word.to_owned();
        while let Some(next) = rest.peek() {
            let joined = format!("{line} {next}");
            if measure(&joined) > width {
                break;
            }
            line = joined;
            rest.next();
        }
        if lines.len() + 1 == max_lines {
            // Last row: whatever remains is shown clipped.
            for next in rest.by_ref() {
                line.push(' ');
                line.push_str(next);
            }
            lines.push(clip(&line));
            return lines;
        }
        lines.push(clip(&line));
    }
    lines
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
/// Select only a local waypoint under the map cursor, in screen pixels at any zoom.
pub fn waypoint_at<'a>(
    view: View,
    rect: [f32; 4],
    mouse: [f32; 2],
    dimension: &str,
    waypoints: impl Iterator<Item = (u64, &'a str, [f64; 3])>,
) -> Option<u64> {
    if !contains(rect, mouse) || rect[3] <= 0.0 {
        return None;
    }
    let at = world(view, rect, mouse);
    let radius = 7.0 * 128.0 * f64::from(view.blocks_per_pixel) / f64::from(rect[3]);
    waypoints
        .filter(|(_, dim, _)| *dim == dimension)
        .map(|(id, _, position)| (id, (position[0] - at[0]).hypot(position[2] - at[1])))
        .filter(|(_, distance)| *distance <= radius)
        .min_by(|a, b| a.1.total_cmp(&b.1))
        .map(|(id, _)| id)
}
pub const SEARCH_TILE_BLOCKS: i32 = 1024;
pub const MAX_SEARCH_TILES: usize = 512;
#[derive(Clone, Debug, PartialEq)]
pub struct Mark {
    pub id: String,
    pub position: [f64; 2],
}
pub struct SearchArea {
    pub tiles: Vec<(i32, i32)>,
    pub limited: bool,
}
pub fn tile_view(at: (i32, i32)) -> View {
    View {
        center: [
            f64::from(at.0) * 1024.0 + 512.0,
            f64::from(at.1) * 1024.0 + 512.0,
        ],
        blocks_per_pixel: 8.0,
    }
}
/// Plan a bounded working set; a landscape 2 km scale fits in full.
pub fn search_tiles(view: View, aspect: f32) -> Option<SearchArea> {
    if !(0.1..=32.0).contains(&aspect)
        || !(0.25..=524288.0).contains(&view.blocks_per_pixel)
        || !view
            .center
            .iter()
            .all(|p| p.is_finite() && p.abs() <= 30_000_000.0)
    {
        return None;
    }
    let a = view.world_aspect([0.0, 0.0], 128.0, aspect);
    let b = view.world_aspect([128.0, 128.0], 128.0, aspect);
    let size = f64::from(SEARCH_TILE_BLOCKS);
    let mut lo = a.map(|v| (v / size).floor().max((-30_000_000.0 / size).floor()) as i32);
    let mut hi = b.map(|v| ((v / size).ceil() - 1.0).min((30_000_000.0 / size).floor()) as i32);
    let w = i64::from(hi[0]) - i64::from(lo[0]) + 1;
    let h = i64::from(hi[1]) - i64::from(lo[1]) + 1;
    if w <= 0 || h <= 0 {
        return None;
    }
    let limited = w * h > MAX_SEARCH_TILES as i64;
    if limited {
        let nx = ((MAX_SEARCH_TILES as f64 * w as f64 / h as f64)
            .sqrt()
            .ceil() as i64)
            .clamp(1, w.min(MAX_SEARCH_TILES as i64));
        let ny = (MAX_SEARCH_TILES as i64 / nx).clamp(1, h);
        for (axis, n) in [nx, ny].into_iter().enumerate() {
            let center = (view.center[axis] / size).floor() as i32;
            lo[axis] = (center - n as i32 / 2).clamp(lo[axis], hi[axis] - n as i32 + 1);
            hi[axis] = lo[axis] + n as i32 - 1;
        }
    }
    let mut tiles: Vec<_> = (lo[1]..=hi[1])
        .flat_map(|z| (lo[0]..=hi[0]).map(move |x| (x, z)))
        .collect();
    let distance = |at| {
        let v = tile_view(at);
        (v.center[0] - view.center[0]).hypot(v.center[1] - view.center[1])
    };
    tiles.sort_by(|a, b| distance(*a).total_cmp(&distance(*b)).then(a.cmp(b)));
    Some(SearchArea { tiles, limited })
}
#[derive(Default)]
pub struct StructureTiles {
    entries: std::collections::BTreeMap<(usize, i32, i32), Vec<Mark>>,
    order: std::collections::VecDeque<(usize, i32, i32)>,
}
impl StructureTiles {
    pub fn contains(&self, dimension: usize, at: (i32, i32)) -> bool {
        self.entries.contains_key(&(dimension, at.0, at.1))
    }
    pub fn insert(
        &mut self,
        dimension: usize,
        at: (i32, i32),
        marks: Vec<Mark>,
        keep: &[(i32, i32)],
    ) -> Result<(), String> {
        let key = (dimension, at.0, at.1);
        if !self.entries.contains_key(&key) {
            while self.entries.len() >= MAX_SEARCH_TILES {
                let index = self
                    .order
                    .iter()
                    .position(|&(d, x, z)| d != dimension || !keep.contains(&(x, z)))
                    .ok_or("seed-map tile working set exceeds cache")?;
                let old = self
                    .order
                    .remove(index)
                    .ok_or("seed-map cache order mismatch")?;
                self.entries.remove(&old);
            }
            self.order.push_back(key);
        }
        self.entries.insert(key, marks);
        Ok(())
    }
    pub fn marks(&self, dimension: usize, view: View, aspect: f32, p: &Preferences) -> Vec<Mark> {
        self.entries
            .iter()
            .filter(|((d, _, _), _)| *d == dimension)
            .flat_map(|(_, marks)| marks)
            .filter(|m| p.shows(&m.id))
            .filter(|m| {
                let xy = view.pixel_aspect(m.position, 128.0, aspect);
                xy.iter().all(|v| *v >= -2.0 && *v <= 130.0)
            })
            .cloned()
            .collect()
    }
}

/// Hard bound on placement checks within one search tile, including dense sets.
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
    fn two_kilometre_view_is_fully_searchable_in_bounded_tiles() {
        let view = View {
            center: [-1928.0, 56.0],
            blocks_per_pixel: 64.0,
        };
        let aspect = 2.5;
        assert_eq!(
            super::super::claim_borders_map::scale_bar(128.0 * 64.0 * 2.5, 1000.0, 160.0)
                .unwrap()
                .0,
            2000.0
        );
        let area = search_tiles(view, aspect).unwrap();
        assert!(!area.limited);
        assert_eq!(area.tiles.len(), 21 * 9);
        let tiles: std::collections::BTreeSet<_> = area.tiles.iter().copied().collect();
        for z in -4..=4 {
            for x in -12..=8 {
                assert!(tiles.contains(&(x, z)));
                assert!(region_bounds(tile_view((x, z)), 1.0, 1).is_some());
            }
        }
        let far = search_tiles(
            View {
                blocks_per_pixel: 524288.0,
                ..view
            },
            aspect,
        )
        .unwrap();
        assert!(far.limited);
        assert!(far.tiles.len() <= MAX_SEARCH_TILES);
        assert!(search_tiles(view, f32::NAN).is_none());
    }
    #[test]
    fn cached_pois_survive_zooming_out_without_a_global_512_marker_cutoff() {
        let mut cache = StructureTiles::default();
        let view = View {
            center: [0.0, 0.0],
            blocks_per_pixel: 64.0,
        };
        let keep = [(0, 0), (-1, 0)];
        let dense: Vec<_> = (0..600)
            .map(|x| Mark {
                id: "mineshaft".into(),
                position: [x as f64, 8.0],
            })
            .collect();
        cache.insert(0, (0, 0), dense, &keep).unwrap();
        cache
            .insert(
                0,
                (-1, 0),
                vec![Mark {
                    id: "village_plains".into(),
                    position: [-8.0, 8.0],
                }],
                &keep,
            )
            .unwrap();
        let marks = cache.marks(0, view, 2.5, &Preferences::default());
        assert_eq!(marks.len(), 601);
        assert!(marks.iter().any(|m| m.id == "village_plains"));
        let zoomed = cache.marks(
            0,
            View {
                blocks_per_pixel: 128.0,
                ..view
            },
            2.5,
            &Preferences::default(),
        );
        assert_eq!(marks, zoomed);
        assert!(
            cache
                .marks(1, view, 2.5, &Preferences::default())
                .is_empty()
        );
        for x in 1..=MAX_SEARCH_TILES as i32 {
            cache.insert(0, (x, 1), Vec::new(), &keep).unwrap();
        }
        assert!(cache.contains(0, (0, 0)) && cache.contains(0, (-1, 0)));
        assert!(cache.entries.len() <= MAX_SEARCH_TILES);
    }
    #[test]
    fn english_labels_have_no_russian_text() {
        let cyrillic = |t: &str| t.chars().any(|c| ('\u{400}'..='\u{4ff}').contains(&c));
        for s in STRUCTURES {
            assert!(!cyrillic(s.label_in(Language::English)));
        }
        for g in GROUPS {
            assert!(!cyrillic(g.label_in(Language::English)));
        }
    }
    #[test]
    fn all_vanilla_variants_and_old_filters_are_preserved() {
        let mut names = std::collections::BTreeSet::new();
        for s in STRUCTURES {
            assert!(names.insert(s.id));
            assert!(s.item.starts_with("minecraft:"));
            assert!(Preferences::default().shows(s.id));
        }
        assert_eq!(names.len(), 35);
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
    fn end_cities_have_independent_ship_filters_and_migrate_old_choice() {
        let mut p = Preferences::default();
        p.toggle(structure_index("end_city").unwrap());
        assert!(!p.shows("end_city"));
        assert!(p.shows("end_city_ship"));
        assert_eq!(Preferences::decode(&p.encode()).unwrap(), p);
        let old = Preferences::decode(r#"{"schema":2,"structures":131072}"#).unwrap();
        assert!(old.shows("end_city"));
        assert!(old.shows("end_city_ship"));
        assert!(
            !Preferences::decode(r#"{"schema":2,"structures":0}"#)
                .unwrap()
                .shows("end_city_ship")
        );
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
    /// Minecraft's default font: 5 px glyphs plus 1 px spacing, with the narrow exceptions.
    fn pixels(text: &str) -> f32 {
        text.chars()
            .map(|c| match c {
                'i' | '!' | '.' | ',' | ':' | ';' | '|' | '\'' | '·' | '…' => 2.0,
                'l' => 3.0,
                't' | 'I' | '[' | ']' | ' ' => 4.0,
                'f' | 'k' | '<' | '>' | '(' | ')' | '{' | '}' | '*' | '"' | '×' => 5.0,
                _ => 6.0,
            })
            .sum()
    }
    #[test]
    fn wrapping_breaks_at_spaces_and_keeps_every_word() {
        let lines = wrap_words("Pillager Outpost", 70.0, 3, &pixels);
        assert_eq!(lines, ["Pillager", "Outpost"]);
        let lines = wrap_words("End City · ship with elytra", 64.0, 3, &pixels);
        assert!(lines.len() <= 3 && lines.iter().all(|l| pixels(l) <= 64.0));
        assert_eq!(lines.join(" "), "End City · ship with elytra");
        assert_eq!(wrap_words("Nether", 64.0, 3, &pixels), ["Nether"]);
        assert!(wrap_words("", 64.0, 3, &pixels).is_empty());
        // A word wider than the row and a text longer than the rows are marked, not overdrawn.
        let lines = wrap_words("Supercalifragilistic word", 40.0, 3, &pixels);
        assert!(lines[0].ends_with('…') && pixels(&lines[0]) <= 40.0);
        let lines = wrap_words("aaa bbb ccc ddd eee", 20.0, 2, &pixels);
        assert_eq!(lines.len(), 2);
        assert!(lines[1].ends_with('…') && lines.iter().all(|l| pixels(l) <= 20.0));
    }
    #[test]
    fn every_structure_label_fits_its_chip_in_both_languages() {
        // 1080p and 1440p at GUI scales 2 to 4, plus the smallest windows.
        for size in [
            [960.0, 540.0],
            [1280.0, 720.0],
            [640.0, 360.0],
            [853.0, 480.0],
            [480.0, 270.0],
            [426.0, 240.0],
            [320.0, 240.0],
        ] {
            let width = filter_chip_rect(size, 0)[2] - 4.0;
            for language in [Language::Russian, Language::English] {
                for def in STRUCTURES.iter() {
                    let label = def.label_in(language);
                    let lines = wrap_words(label, width, CHIP_LINES, &pixels);
                    assert!(
                        lines.iter().all(|l| !l.contains('…') && pixels(l) <= width),
                        "{label:?} does not fit {width}px at {size:?}: {lines:?}"
                    );
                    assert_eq!(lines.join(" "), label);
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
    #[test]
    fn waypoint_hit_test_uses_visible_dimension_and_screen_radius() {
        let marks = [
            (1, "minecraft:the_end", [0.0, 64.0, 0.0]),
            (2, "minecraft:overworld", [1.0, 64.0, 0.0]),
            (3, "minecraft:overworld", [3.0, 64.0, 0.0]),
        ];
        let rect = [0.0, 0.0, 128.0, 128.0];
        let view = View {
            center: [0.0, 0.0],
            blocks_per_pixel: 1.0,
        };
        assert_eq!(
            waypoint_at(
                view,
                rect,
                [64.0, 64.0],
                "minecraft:overworld",
                marks.into_iter()
            ),
            Some(2)
        );
        assert_eq!(
            waypoint_at(
                view,
                rect,
                [100.0, 100.0],
                "minecraft:overworld",
                marks.into_iter()
            ),
            None
        );
        assert_eq!(
            waypoint_at(
                view,
                rect,
                [-1.0, 64.0],
                "minecraft:overworld",
                marks.into_iter()
            ),
            None
        );
        let zoomed = View {
            blocks_per_pixel: 0.25,
            ..view
        };
        assert_eq!(
            waypoint_at(
                zoomed,
                rect,
                [64.0, 64.0],
                "minecraft:overworld",
                marks[2..].iter().copied()
            ),
            None
        );
    }
}
