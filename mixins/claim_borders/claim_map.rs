// Claim cartography and preferences; no game access or server mutations.
use serde_json::{Value, json};
use std::collections::BTreeMap;
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Layer {
    #[default]
    Auto,
    Surface,
    Cave,
}
impl Layer {
    pub fn next(self) -> Self {
        match self {
            Self::Auto => Self::Surface,
            Self::Surface => Self::Cave,
            Self::Cave => Self::Auto,
        }
    }
    pub fn label(self) -> &'static str {
        match self {
            Self::Auto => "Авто",
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
}
impl Default for Preferences {
    fn default() -> Self {
        Self {
            minimap: true,
            corner: 1,
            size: 112,
            zoom: 2.0,
            grid: true,
            coordinates: true,
            layer: Layer::Auto,
            marks: true,
            own: true,
            friends: true,
            fill: 0.25,
            outline: 1,
            labels: true,
            sort: Sort::Distance,
            all_dimensions: false,
            grid_step: 8,
        }
    }
}
impl Preferences {
    pub fn normalized(mut self) -> Self {
        self.corner %= 4;
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
        json!({"schema":1,"minimap":p.minimap,"corner":p.corner,"size":p.size,"zoom":p.zoom,"grid":p.grid,"coordinates":p.coordinates,"layer":p.layer as u8,"marks":p.marks,"own":p.own,"friends":p.friends,"fill":p.fill,"outline":p.outline,"labels":p.labels,"sort":p.sort as u8,"all_dimensions":p.all_dimensions,"grid_step":p.grid_step}).to_string()
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
            grid: flag("grid", true),
            coordinates: flag("coordinates", true),
            layer: match v["layer"].as_u64() {
                Some(1) => Layer::Surface,
                Some(2) => Layer::Cave,
                _ => Layer::Auto,
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
        }
        .normalized())
    }
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Cell {
    pub dimension: String,
    pub x: i32,
    pub z: i32,
    pub owner: String,
    pub own: bool,
}
#[derive(Clone, Debug)]
pub struct Mark {
    pub dimension: String,
    pub position: [f64; 2],
    pub label: String,
    pub color: u32,
}
#[derive(Clone, Debug, Default)]
pub struct Snapshot {
    pub scope: String,
    pub dimension: String,
    pub player: [f64; 3],
    pub cells: Vec<Cell>,
    pub marks: Vec<Mark>,
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
pub fn paint(
    image: &mut [u8],
    size: usize,
    view: View,
    dimension: &str,
    snapshot: &Snapshot,
    p: Preferences,
    full: bool,
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
            cells.insert(key, c.own);
        }
    }
    for y in 0..size {
        for x in 0..size {
            let world = view.world([x as f32 + 0.5, y as f32 + 0.5], size as f32);
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
            if let Some(own) = cells.get(&(bx, bz)) {
                let color = if *own {
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
        let xy = view.pixel(position, size as f32);
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
                if c.own {
                    snapshot.own_color
                } else {
                    snapshot.friend_color
                },
                1,
            );
        }
    }
    if p.marks {
        for mark in snapshot.marks.iter().filter(|m| m.dimension == dimension) {
            dot(mark.position, mark.color, 1);
        }
    }
    if full && snapshot.dimension == dimension {
        dot([snapshot.player[0], snapshot.player[2]], 0xffffff, 2);
    }
}
#[cfg(test)]
mod tests {
    use super::*;
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
                },
                Cell {
                    dimension: "minecraft:overworld".into(),
                    x: -16,
                    z: 0,
                    owner: "Друг".into(),
                    own: false,
                },
                Cell {
                    dimension: "minecraft:the_nether".into(),
                    x: 0,
                    z: 0,
                    owner: "Вы".into(),
                    own: true,
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
        let s = Snapshot {
            own_color: 0x00ff00,
            cells: vec![Cell {
                dimension: "minecraft:overworld".into(),
                x: 0,
                z: 0,
                owner: "Вы".into(),
                own: true,
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
