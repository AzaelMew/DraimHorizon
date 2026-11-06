// Claim cartography and preferences; no game access or server mutations.
use serde_json::{Value, json};
use std::collections::BTreeMap;
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Layer {
    // Legacy value 0 is retained for old settings, but no longer auto-selects caves.
    Auto,
    #[default]
    Surface,
    Cave,
}
impl Layer {
    pub fn next(self) -> Self {
        match self {
            Self::Auto | Self::Cave => Self::Surface,
            Self::Surface => Self::Cave,
        }
    }
    /// A surface scan never depends on player Y. Explicit caves share one canonical
    /// ceiling per saved 16-block band, so single-block movement cannot invalidate tiles.
    pub fn scan_ceiling(self, head: i32) -> Option<i32> {
        match self {
            Self::Auto | Self::Surface => None,
            Self::Cave => Some(head.div_euclid(16) * 16 + 15),
        }
    }
    pub fn label(self) -> &'static str {
        match self {
            Self::Auto => "Поверхность",
            Self::Surface => "Поверхность",
            Self::Cave => "Пещеры",
        }
    }
}
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Sort {
    #[default]
    Distance,
    Owner,
    Dimension,
    Coordinates,
}
impl Sort {
    pub fn next(self) -> Self {
        match self {
            Self::Distance => Self::Owner,
            Self::Owner => Self::Dimension,
            Self::Dimension => Self::Coordinates,
            Self::Coordinates => Self::Distance,
        }
    }
    pub fn label(self) -> &'static str {
        match self {
            Self::Distance => "Ближе",
            Self::Owner => "Владелец",
            Self::Dimension => "Измерение",
            Self::Coordinates => "Координаты",
        }
    }
}
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Preferences {
    pub minimap: bool,
    pub corner: u8,
    pub size: u16,
    pub zoom: f32,
    pub grid: bool,
    pub coordinates: bool,
    pub layer: Layer,
    pub marks: bool,
    pub own: bool,
    pub friends: bool,
    pub fill: f32,
    pub outline: u8,
    pub labels: bool,
    pub sort: Sort,
    pub all_dimensions: bool,
    pub grid_step: u8,
    pub remember: bool,
    pub shading: bool,
    pub profile: u8,
    pub mobs: u8,
    pub mob_hidden: [u64; 2],
}
impl Default for Preferences {
    fn default() -> Self {
        Self {
            minimap: true,
            corner: 1,
            size: 112,
            zoom: 2.0,
            grid: false,
            coordinates: true,
            layer: Layer::Surface,
            marks: true,
            own: true,
            friends: true,
            fill: 0.25,
            outline: 1,
            labels: true,
            sort: Sort::Distance,
            all_dimensions: false,
            grid_step: 8,
            remember: true,
            shading: true,
            profile: 0,
            mobs: 1,
            mob_hidden: [0; 2],
        }
    }
}
impl Preferences {
    pub fn normalized(mut self) -> Self {
        if self.layer == Layer::Auto {
            self.layer = Layer::Surface;
        }
        self.corner %= 4;
        self.profile %= 8;
        self.mobs %= 3;
        self.size = self.size.clamp(64, 192);
        self.zoom = if self.zoom.is_finite() {
            self.zoom.clamp(0.5, 4.0)
        } else {
            2.0
        };
        self.fill = if self.fill.is_finite() {
            self.fill.clamp(0.0, 0.8)
        } else {
            0.25
        };
        self.outline = self.outline.clamp(1, 3);
        if ![0, 8, 16, 32].contains(&self.grid_step) {
            self.grid_step = 8;
        }
        self
    }
    pub fn encode(self) -> String {
        let p = self.normalized();
        json!({"schema":1,"minimap":p.minimap,"corner":p.corner,"size":p.size,"zoom":p.zoom,"grid":p.grid,"coordinates":p.coordinates,"layer":p.layer as u8,"marks":p.marks,"own":p.own,"friends":p.friends,"fill":p.fill,"outline":p.outline,"labels":p.labels,"sort":p.sort as u8,"all_dimensions":p.all_dimensions,"grid_step":p.grid_step,"remember":p.remember,"shading":p.shading,"profile":p.profile,"mobs":p.mobs,"mob_hidden":p.mob_hidden}).to_string()
    }
    pub fn decode(raw: &str) -> Result<Self, String> {
        if raw.len() > 4096 {
            return Err("map settings too large".into());
        }
        let v: Value = serde_json::from_str(raw).map_err(|_| "invalid map settings")?;
        if v["schema"] != 1 {
            return Err("unsupported map settings".into());
        }
        let d = Self::default();
        let flag = |key: &str, default| v[key].as_bool().unwrap_or(default);
        Ok(Self {
            minimap: flag("minimap", d.minimap),
            corner: v["corner"].as_u64().unwrap_or(1).min(3) as u8,
            size: v["size"].as_u64().unwrap_or(112).min(192) as u16,
            zoom: v["zoom"].as_f64().unwrap_or(2.0) as f32,
            grid: flag("grid", d.grid),
            coordinates: flag("coordinates", true),
            layer: match v["layer"].as_u64() {
                Some(1) => Layer::Surface,
                Some(2) => Layer::Cave,
                _ => Layer::Surface,
            },
            marks: flag("marks", true),
            own: flag("own", true),
            friends: flag("friends", true),
            fill: v["fill"].as_f64().unwrap_or(0.25) as f32,
            outline: v["outline"].as_u64().unwrap_or(1).min(3) as u8,
            labels: flag("labels", true),
            sort: match v["sort"].as_u64() {
                Some(1) => Sort::Owner,
                Some(2) => Sort::Dimension,
                Some(3) => Sort::Coordinates,
                _ => Sort::Distance,
            },
            all_dimensions: flag("all_dimensions", false),
            grid_step: v["grid_step"].as_u64().unwrap_or(8).min(32) as u8,
            remember: flag("remember", true),
            shading: flag("shading", true),
            profile: v["profile"].as_u64().unwrap_or(0).min(7) as u8,
            mobs: v["mobs"].as_u64().unwrap_or(1).min(2) as u8,
            mob_hidden: [
                v["mob_hidden"][0].as_u64().unwrap_or(0),
                v["mob_hidden"][1].as_u64().unwrap_or(0),
            ],
        }
        .normalized())
    }
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Cell {
    pub foreign: bool,
    pub dimension: String,
    pub x: i32,
    pub z: i32,
    pub owner: String,
    pub own: bool,
}
#[derive(Clone, Debug)]
pub struct Mark {
    pub player: Option<u128>,
    pub friend: bool,
    pub skin: String,
    pub dimension: String,
    pub position: [f64; 2],
    pub label: String,
    pub color: u32,
}
#[derive(Clone, Debug)]
pub struct MobMark {
    pub position: [f64; 3],
    pub label: String,
    pub type_id: String,
    pub hostile: bool,
}
// Stable indices are persisted; append new types without reordering existing entries.
pub const MOB_TYPES: &[&str] = &[
    "zombie",
    "skeleton",
    "creeper",
    "spider",
    "enderman",
    "witch",
    "slime",
    "cave_spider",
    "husk",
    "drowned",
    "stray",
    "bogged",
    "phantom",
    "silverfish",
    "endermite",
    "warden",
    "pillager",
    "vindicator",
    "evoker",
    "ravager",
    "vex",
    "breeze",
    "blaze",
    "ghast",
    "magma_cube",
    "wither_skeleton",
    "piglin",
    "piglin_brute",
    "zombified_piglin",
    "hoglin",
    "zoglin",
    "shulker",
    "ender_dragon",
    "wither",
    "guardian",
    "elder_guardian",
    "cow",
    "pig",
    "sheep",
    "chicken",
    "wolf",
    "cat",
    "ocelot",
    "fox",
    "rabbit",
    "horse",
    "donkey",
    "mule",
    "llama",
    "trader_llama",
    "camel",
    "goat",
    "panda",
    "polar_bear",
    "bee",
    "bat",
    "allay",
    "armadillo",
    "sniffer",
    "frog",
    "tadpole",
    "axolotl",
    "turtle",
    "dolphin",
    "squid",
    "glow_squid",
    "cod",
    "salmon",
    "tropical_fish",
    "pufferfish",
    "strider",
    "mooshroom",
    "villager",
    "wandering_trader",
    "iron_golem",
    "snow_golem",
    "zombie_villager",
    "zombie_horse",
    "skeleton_horse",
    "parrot",
    "happy_ghast",
    "copper_golem",
    "nautilus",
    "zombie_nautilus",
];
impl Preferences {
    pub fn mob_selected(self, index: usize) -> bool {
        index < MOB_TYPES.len() && self.mob_hidden[index / 64] & (1 << (index % 64)) == 0
    }
    pub fn toggle_mob(&mut self, index: usize) {
        if index < MOB_TYPES.len() {
            self.mob_hidden[index / 64] ^= 1 << (index % 64);
        }
    }
    pub fn mob_visible(self, id: &str, hostile: bool) -> bool {
        self.mobs > 0
            && (self.mobs != 2 || hostile)
            && MOB_TYPES
                .iter()
                .position(|name| *name == id.strip_prefix("minecraft:").unwrap_or(id))
                .is_none_or(|index| self.mob_selected(index))
    }
}
pub fn mob_group(id: &str, category: &str) -> Option<bool> {
    if id == "minecraft:player" {
        return None;
    }
    match category {
        "monster" => Some(true),
        "creature"
        | "ambient"
        | "axolotls"
        | "underground_water_creature"
        | "water_creature"
        | "water_ambient" => Some(false),
        _ => None,
    }
}
#[derive(Clone, Debug, Default)]
pub struct Snapshot {
    pub scope: String,
    pub dimension: String,
    pub player: [f64; 3],
    pub player_uuid: u128,
    pub player_skin: String,
    pub yaw: f32,
    pub cells: Vec<Cell>,
    pub marks: Vec<Mark>,
    pub mobs: Vec<MobMark>,
    pub own_known: bool,
    pub own_color: u32,
    pub friend_color: u32,
}
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct View {
    pub center: [f64; 2],
    pub blocks_per_pixel: f32,
}
impl View {
    pub fn world(self, pixel: [f32; 2], size: f32) -> [f64; 2] {
        [
            self.center[0] + f64::from((pixel[0] - size * 0.5) * self.blocks_per_pixel),
            self.center[1] + f64::from((pixel[1] - size * 0.5) * self.blocks_per_pixel),
        ]
    }
    pub fn world_aspect(self, pixel: [f32; 2], size: f32, aspect: f32) -> [f64; 2] {
        self.world(
            [(pixel[0] - size * 0.5) * aspect + size * 0.5, pixel[1]],
            size,
        )
    }
    pub fn pixel_aspect(self, world: [f64; 2], size: f32, aspect: f32) -> [f32; 2] {
        let xy = self.pixel(world, size);
        [(xy[0] - size * 0.5) / aspect + size * 0.5, xy[1]]
    }
    pub fn pixel(self, world: [f64; 2], size: f32) -> [f32; 2] {
        [
            (world[0] - self.center[0]) as f32 / self.blocks_per_pixel + size * 0.5,
            (world[1] - self.center[1]) as f32 / self.blocks_per_pixel + size * 0.5,
        ]
    }
    pub fn zoom_at(&mut self, pixel: [f32; 2], size: f32, factor: f32) {
        let world = self.world(pixel, size);
        self.blocks_per_pixel = (self.blocks_per_pixel * factor).clamp(0.25, 524288.0);
        self.center = [
            world[0] - f64::from((pixel[0] - size * 0.5) * self.blocks_per_pixel),
            world[1] - f64::from((pixel[1] - size * 0.5) * self.blocks_per_pixel),
        ];
        self.constrain();
    }
    pub fn constrain(&mut self) {
        for v in &mut self.center {
            *v = if v.is_finite() {
                v.clamp(-29_999_984.0, 29_999_984.0)
            } else {
                0.0
            };
        }
    }
}
pub fn scale_bar(span: f64, pixels: f32, max_pixels: f32) -> Option<(f64, f32)> {
    if !span.is_finite() || span <= 0.0 || !pixels.is_finite() || pixels <= 0.0 || max_pixels < 1.0
    {
        return None;
    }
    let max = span * f64::from(max_pixels / pixels);
    let unit = 10f64.powf(max.log10().floor());
    let blocks = [5.0, 2.0, 1.0]
        .into_iter()
        .map(|m| m * unit)
        .find(|n| *n <= max)?;
    Some((blocks, (blocks / span * pixels as f64) as f32))
}
pub fn origin(value: f64) -> i32 {
    ((value.floor() as i32).div_euclid(16)) * 16
}
pub fn dimension_label(value: &str) -> &str {
    match value {
        "minecraft:overworld" => "Обычный мир",
        "minecraft:the_nether" => "Незер",
        "minecraft:the_end" => "Край",
        _ => value,
    }
}
pub fn sorted_cells(snapshot: &Snapshot, p: Preferences) -> Vec<Cell> {
    let mut cells: Vec<_> = snapshot
        .cells
        .iter()
        .filter(|c| {
            (p.all_dimensions || c.dimension == snapshot.dimension)
                && ((c.own && p.own) || (!c.own && p.friends))
        })
        .cloned()
        .collect();
    let distance = |c: &Cell| {
        (c.x as f64 + 8.0 - snapshot.player[0]).hypot(c.z as f64 + 8.0 - snapshot.player[2])
    };
    cells.sort_by(|a, b| {
        let dimension = if p.sort == Sort::Dimension {
            a.dimension.cmp(&b.dimension)
        } else {
            (a.dimension != snapshot.dimension)
                .cmp(&(b.dimension != snapshot.dimension))
                .then_with(|| {
                    if a.dimension != snapshot.dimension && b.dimension != snapshot.dimension {
                        a.dimension.cmp(&b.dimension)
                    } else {
                        std::cmp::Ordering::Equal
                    }
                })
        };
        dimension
            .then_with(|| match p.sort {
                Sort::Owner => a.owner.cmp(&b.owner),
                Sort::Coordinates => a.x.cmp(&b.x).then(a.z.cmp(&b.z)),
                _ => distance(a).total_cmp(&distance(b)),
            })
            .then(a.owner.cmp(&b.owner))
            .then(a.dimension.cmp(&b.dimension))
            .then(a.x.cmp(&b.x))
            .then(a.z.cmp(&b.z))
    });
    cells.dedup();
    cells
}
fn blend(pixel: &mut [u8], color: u32, alpha: f32) {
    let [_, r, g, b] = color.to_be_bytes();
    for (i, c) in [r, g, b].into_iter().enumerate() {
        pixel[i] = (f32::from(pixel[i]) * (1.0 - alpha) + f32::from(c) * alpha).round() as u8;
    }
    pixel[3] = 255;
}
#[cfg(test)]
pub fn paint(
    image: &mut [u8],
    size: usize,
    view: View,
    dimension: &str,
    snapshot: &Snapshot,
    p: Preferences,
    _full: bool,
) {
    paint_aspect(image, size, view, dimension, snapshot, p, 1.0);
}
pub fn paint_aspect(
    image: &mut [u8],
    size: usize,
    view: View,
    dimension: &str,
    snapshot: &Snapshot,
    p: Preferences,
    aspect: f32,
) {
    if image.len() != size * size * 4
        || !view.blocks_per_pixel.is_finite()
        || view.blocks_per_pixel <= 0.0
    {
        return;
    }
    let mut cells = BTreeMap::new();
    for c in snapshot
        .cells
        .iter()
        .filter(|c| c.dimension == dimension && ((c.own && p.own) || (!c.own && p.friends)))
    {
        let key = (c.x, c.z);
        if c.own || !cells.contains_key(&key) {
            cells.insert(key, (c.own, c.foreign));
        }
    }
    for y in 0..size {
        for x in 0..size {
            let world = view.world_aspect([x as f32 + 0.5, y as f32 + 0.5], size as f32, aspect);
            let bx = origin(world[0]);
            let bz = origin(world[1]);
            let lx = world[0] - bx as f64;
            let lz = world[1] - bz as f64;
            let edge = lx.min(lz).min(16.0 - lx).min(16.0 - lz);
            let pixel = &mut image[(y * size + x) * 4..(y * size + x + 1) * 4];
            if p.grid
                && view.blocks_per_pixel <= 8.0
                && edge < f64::from(view.blocks_per_pixel) * 0.45
            {
                blend(pixel, 0xc0c9d5, 0.18);
            }
            if let Some((own, observed)) = cells.get(&(bx, bz)) {
                let color = if *observed {
                    0xe45b65
                } else if *own {
                    snapshot.own_color
                } else {
                    snapshot.friend_color
                };
                blend(pixel, color, p.fill);
                if edge < f64::from(view.blocks_per_pixel) * f64::from(p.outline) {
                    blend(pixel, color, 0.95);
                }
            }
        }
    }
    let mut dot = |position: [f64; 2], color: u32, radius: i32| {
        let xy = view.pixel_aspect(position, size as f32, aspect);
        if !xy.iter().all(|v| v.is_finite()) {
            return;
        }
        let (cx, cy) = (xy[0].round() as i32, xy[1].round() as i32);
        for y in cy - radius..=cy + radius {
            for x in cx - radius..=cx + radius {
                if x >= 1 && y >= 1 && (x as usize) < size - 1 && (y as usize) < size - 1 {
                    let i = (y as usize * size + x as usize) * 4;
                    blend(&mut image[i..i + 4], color, 1.0);
                }
            }
        }
    };
    if view.blocks_per_pixel > 8.0 {
        for c in snapshot
            .cells
            .iter()
            .filter(|c| c.dimension == dimension && ((c.own && p.own) || (!c.own && p.friends)))
        {
            dot(
                [c.x as f64 + 8.0, c.z as f64 + 8.0],
                if c.foreign {
                    0xe45b65
                } else if c.own {
                    snapshot.own_color
                } else {
                    snapshot.friend_color
                },
                1,
            );
        }
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn surface_maps_and_legacy_auto_settings_are_independent_of_player_height() {
        assert_eq!(Preferences::default().layer, Layer::Surface);
        for raw in [
            r#"{"schema":1}"#,
            r#"{"schema":1,"layer":0}"#,
            r#"{"schema":1,"layer":1}"#,
        ] {
            let p = Preferences::decode(raw).unwrap();
            assert_eq!(p.layer, Layer::Surface);
            for head in -64..=320 {
                assert_eq!(p.layer.scan_ceiling(head), None);
            }
        }
        assert_eq!(
            Preferences {
                layer: Layer::Auto,
                ..Preferences::default()
            }
            .normalized()
            .layer,
            Layer::Surface
        );
        assert_eq!(Layer::Auto.scan_ceiling(12), None);
        assert_eq!(Layer::Surface.next(), Layer::Cave);
        assert_eq!(Layer::Cave.next(), Layer::Surface);
    }
    #[test]
    fn explicit_cave_scans_are_stable_within_positive_and_negative_height_bands() {
        let p = Preferences::decode(r#"{"schema":1,"layer":2}"#).unwrap();
        assert_eq!(p.layer, Layer::Cave);
        assert_eq!(Preferences::decode(&p.encode()).unwrap().layer, Layer::Cave);
        for band in -4..20 {
            for head in band * 16..band * 16 + 16 {
                assert_eq!(p.layer.scan_ceiling(head), Some(band * 16 + 15));
            }
        }
    }
    #[test]
    fn radar_includes_living_categories_and_excludes_players_and_items() {
        assert_eq!(mob_group("minecraft:zombie", "monster"), Some(true));
        assert_eq!(mob_group("minecraft:cow", "creature"), Some(false));
        assert_eq!(mob_group("minecraft:squid", "water_creature"), Some(false));
        assert_eq!(mob_group("minecraft:player", "creature"), None);
        assert_eq!(mob_group("minecraft:item", "misc"), None);
    }
    #[test]
    fn rectangular_view_projection_and_scale_remain_geographic() {
        let view = View {
            center: [-150.0, 88.0],
            blocks_per_pixel: 2.0,
        };
        for aspect in [0.5, 1.0, 2.5] {
            let point = [30.0, 190.0];
            let px = view.pixel_aspect(point, 128.0, aspect);
            let back = view.world_aspect(px, 128.0, aspect);
            assert!((point[0] - back[0]).abs() < 0.001);
            assert!((point[1] - back[1]).abs() < 0.001);
        }
        let (blocks, pixels) = scale_bar(512.0, 256.0, 64.0).unwrap();
        assert_eq!(blocks, 100.0);
        assert_eq!(pixels, 50.0);
    }
    #[test]
    fn negative_chunks_and_mouse_anchored_zoom() {
        assert_eq!(origin(-0.1), -16);
        assert_eq!(origin(-16.0), -16);
        let mut v = View {
            center: [-1900.0, 48.0],
            blocks_per_pixel: 2.0,
        };
        let before = v.world([90.0, 70.0], 128.0);
        v.zoom_at([90.0, 70.0], 128.0, 0.5);
        assert_eq!(v.world([90.0, 70.0], 128.0), before);
        assert_eq!(v.pixel(before, 128.0), [90.0, 70.0]);
    }
    #[test]
    fn sorting_is_stable_and_dimensions_do_not_mix() {
        let s = Snapshot {
            dimension: "minecraft:overworld".into(),
            cells: vec![
                Cell {
                    dimension: "minecraft:overworld".into(),
                    x: 32,
                    z: 0,
                    owner: "Вы".into(),
                    own: true,
                    foreign: false,
                },
                Cell {
                    dimension: "minecraft:overworld".into(),
                    x: -16,
                    z: 0,
                    owner: "Друг".into(),
                    own: false,
                    foreign: false,
                },
                Cell {
                    dimension: "minecraft:the_nether".into(),
                    x: 0,
                    z: 0,
                    owner: "Вы".into(),
                    own: true,
                    foreign: false,
                },
            ],
            ..Snapshot::default()
        };
        let cells = sorted_cells(&s, Preferences::default());
        assert_eq!(cells.len(), 2);
        assert_eq!(cells[0].x, -16);
        let all = sorted_cells(
            &s,
            Preferences {
                all_dimensions: true,
                ..Preferences::default()
            },
        );
        assert_eq!(all.len(), 3);
        assert_eq!(all[2].dimension, "minecraft:the_nether");
    }
    #[test]
    fn claim_overlay_respects_layers_and_round_trips_preferences() {
        let p = Preferences::default();
        assert_eq!(Preferences::decode(&p.encode()).unwrap(), p);
        let custom = Preferences {
            remember: false,
            shading: false,
            profile: 7,
            ..p
        };
        assert_eq!(Preferences::decode(&custom.encode()).unwrap(), custom);
        let legacy = Preferences::decode(r#"{"schema":1,"grid":true}"#).unwrap();
        assert!(legacy.remember && legacy.shading && legacy.grid);
        assert_eq!(legacy.profile, 0);
        let s = Snapshot {
            own_color: 0x00ff00,
            cells: vec![Cell {
                dimension: "minecraft:overworld".into(),
                x: 0,
                z: 0,
                owner: "Вы".into(),
                own: true,
                foreign: false,
            }],
            ..Snapshot::default()
        };
        let mut image = vec![0; 32 * 32 * 4];
        paint(
            &mut image,
            32,
            View {
                center: [8.0, 8.0],
                blocks_per_pixel: 1.0,
            },
            "minecraft:overworld",
            &s,
            Preferences { grid: false, ..p },
            true,
        );
        let i = (16 * 32 + 18) * 4;
        assert!(image[i + 1] > 0);
        let mut other = vec![0; 32 * 32 * 4];
        paint(
            &mut other,
            32,
            View {
                center: [8.0, 8.0],
                blocks_per_pixel: 1.0,
            },
            "minecraft:the_nether",
            &s,
            Preferences { grid: false, ..p },
            true,
        );
        assert!(other.iter().all(|v| *v == 0));
    }
}

#[cfg(test)]
mod observed_tests {
    use super::*;
    #[test]
    fn observed_foreign_claims_are_red_and_follow_foreign_visibility() {
        let snapshot = Snapshot {
            cells: vec![Cell {
                foreign: true,
                dimension: "minecraft:overworld".into(),
                x: 0,
                z: 0,
                owner: "Alex (наблюдение)".into(),
                own: false,
            }],
            own_color: 0x00ff00,
            friend_color: 0x00ff00,
            ..Snapshot::default()
        };
        let mut image = vec![0u8; 16 * 16 * 4];
        let view = View {
            center: [8., 8.],
            blocks_per_pixel: 1.,
        };
        let p = Preferences {
            grid: false,
            fill: 1.0,
            ..Preferences::default()
        };
        paint(
            &mut image,
            16,
            view,
            "minecraft:overworld",
            &snapshot,
            p,
            true,
        );
        let center = (8 * 16 + 8) * 4;
        assert!(image[center] > image[center + 1]);
        image.fill(0);
        paint(
            &mut image,
            16,
            view,
            "minecraft:overworld",
            &snapshot,
            Preferences {
                friends: false,
                ..p
            },
            true,
        );
        assert_eq!(image[center], 0);
    }
}

#[cfg(test)]
mod mob_filter_tests {
    use super::*;
    #[test]
    fn specific_filters_persist_and_combine_with_hostiles_only() {
        let mut p = Preferences::default();
        let zombie = MOB_TYPES.iter().position(|id| *id == "zombie").unwrap();
        p.toggle_mob(zombie);
        assert!(!p.mob_visible("minecraft:zombie", true));
        assert!(p.mob_visible("minecraft:cow", false));
        p = Preferences::decode(&p.encode()).unwrap();
        assert!(!p.mob_visible("minecraft:zombie", true));
        p.mobs = 2;
        assert!(!p.mob_visible("minecraft:cow", false));
        assert!(p.mob_visible("minecraft:skeleton", true));
        p.mobs = 0;
        assert!(!p.mob_visible("minecraft:skeleton", true));
    }
}
