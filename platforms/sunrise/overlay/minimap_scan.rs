// Terrain tiles for the live minimap and the explored map.
//
// The 2026-11-19 Sunrise Nightly removed its engine minimap: the `minimap` hook and its
// `Minimap`/`MinimapEvent` types, the render crate's terrain scanner and its drawing. A mod that
// wants a map now keeps its own picture (`DynamicTextures`) and draws it in the GUI.
//
// This module is the scanner the mod needs, kept here instead of the render crate: one 16x16 tile
// per loaded chunk holding each column's vanilla map colour (`MapItem.update`'s choice), its height
// and its water depth. Tiles are filled nearest first within a time budget and rescanned after a
// block change. Nothing here draws or composes; `claim_borders_map_backend` composites the image
// and `claim_borders_map_images` uploads it.
use std::{
    collections::BTreeMap,
    time::{Duration, Instant},
};
use sunrise_client::blocks::{BlockFacts, Fluid};
use sunrise_client_render::{chunk::Chunk, world::World};

/// The image's side in texels (the mod composes and draws it; the engine used to fix this at 128).
pub const SIZE: usize = 128;
/// Marks a column the map cuts through (a wall at the player's height).
const WALL: u8 = 0x80;
/// Water's and lava's map colours (`MapColor.WATER`, `MapColor.FIRE`).
const WATER: u8 = 12;
const LAVA: u8 = 4;
/// A tile whose chunk changed is rescanned at most this often.
const RESCAN: Duration = Duration::from_secs(1);

/// What the map shows of each column.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Layer {
    /// The top block, as map items show it.
    Surface,
    /// Underground, in the Nether or under a roof: the floor below this height (the player's
    /// head), and walls cut at it.
    Below(i32),
}

/// What the scan needs of each block state, built once per facts set.
pub struct Palette {
    /// Map colour id, 0 for none.
    color: Vec<u8>,
    /// 0 none, 1 water, 2 lava; | 4 for air; | 8 when sturdy on top.
    flags: Vec<u8>,
}
const FLUID: u8 = 3;
const AIR: u8 = 4;
const STURDY_TOP: u8 = 8;
impl Palette {
    pub fn new(facts: &BlockFacts) -> Self {
        let mut color = Vec::with_capacity(facts.states.len());
        let mut flags = Vec::with_capacity(facts.states.len());
        for (i, f) in facts.states.iter().enumerate() {
            color.push(f.map_color);
            let mut b = match f.fluid {
                // A mod's fluid shows its block's map colour, as any block does.
                Fluid::None | Fluid::Modded => 0,
                Fluid::Water => 1,
                Fluid::Lava => 2,
            };
            // A state's id is its index here; `is_air` keeps the engine's own air list, so a new
            // air name upstream does not leave those columns as unknown terrain.
            if facts.is_air(sunrise_world::registry::BlockStateId(i as u32)) {
                b |= AIR;
            }
            // isFaceSturdy(UP, FULL): Direction.UP's ordinal is 1.
            if f.sturdy_full >> 1 & 1 != 0 {
                b |= STURDY_TOP;
            }
            flags.push(b);
        }
        Self { color, flags }
    }
    /// A palette from (colour, flags) per state id (tests).
    #[cfg(test)]
    fn from_parts(parts: &[(u8, u8)]) -> Self {
        Self {
            color: parts.iter().map(|p| p.0).collect(),
            flags: parts.iter().map(|p| p.1).collect(),
        }
    }
    #[inline]
    fn get(&self, id: u32) -> (u8, u8) {
        let i = id as usize;
        (
            self.color.get(i).copied().unwrap_or(0),
            self.flags.get(i).copied().unwrap_or(AIR),
        )
    }
}

#[derive(Clone)]
pub struct Tile {
    /// Map colour id per column (`z * 16 + x`), `| WALL` for cut walls.
    pub color: [u8; 256],
    pub height: [i16; 256],
    /// Water depth below the surface, for the map's water shading.
    pub depth: [u8; 256],
    revision: u64,
    layer: Layer,
    scanned: Instant,
}
/// `MapItem.update` for one column: the first block with a map colour going down, the fluid's
/// colour over a fluid unless the block is sturdy on top (`getCorrectStateForFluidBlock`), and the
/// fluid's depth.
fn scan_column(
    chunk: &Chunk,
    palette: &Palette,
    x: i32,
    z: i32,
    top: i32,
    layer: Layer,
) -> (u8, i16, u8) {
    let mut y = top;
    if let Layer::Below(head) = layer {
        let (color, flags) = palette.get(chunk.get(x, head, z));
        if flags & AIR == 0 && color != 0 && flags & FLUID == 0 {
            return (color | WALL, head as i16, 0);
        }
    }
    let bottom = chunk.min_y();
    while y > bottom {
        let (mut color, flags) = palette.get(chunk.get(x, y, z));
        if color == 0 {
            y -= 1;
            continue;
        }
        let mut depth = 0u8;
        let fluid = flags & FLUID;
        if fluid != 0 {
            let mut below = y - 1;
            while below > bottom && palette.get(chunk.get(x, below, z)).1 & FLUID != 0 {
                depth = depth.saturating_add(1);
                below -= 1;
            }
            depth = depth.saturating_add(1);
            if flags & STURDY_TOP == 0 {
                color = if fluid == 2 { LAVA } else { WATER };
            }
        }
        return (color, y as i16, depth);
    }
    (0, bottom as i16, 0)
}
/// The highest block of a chunk worth scanning from.
fn top_of(chunk: &Chunk, air: u32, layer: Layer) -> i32 {
    let surface = (0..chunk.sections.len())
        .rev()
        .find(|&s| {
            chunk
                .sections
                .get(s)
                .is_some_and(|sec| !sec.is_uniform(air))
        })
        .map_or(chunk.min_y(), |s| chunk.min_y() + s as i32 * 16 + 15);
    match layer {
        Layer::Surface => surface,
        Layer::Below(head) => surface.min(head),
    }
}
fn scan_tile(chunk: &Chunk, palette: &Palette, air: u32, layer: Layer, revision: u64) -> Tile {
    let top = top_of(chunk, air, layer);
    let mut tile = Tile {
        color: [0; 256],
        height: [chunk.min_y() as i16; 256],
        depth: [0; 256],
        revision,
        layer,
        scanned: Instant::now(),
    };
    for z in 0..16 {
        for x in 0..16 {
            let (c, h, d) = scan_column(chunk, palette, x, z, top, layer);
            let i = (z * 16 + x) as usize;
            tile.color[i] = c;
            tile.height[i] = h;
            tile.depth[i] = d;
        }
    }
    tile
}
/// Chunk offsets at Chebyshev distance `ring`.
fn ring_offsets(ring: i32) -> impl Iterator<Item = (i32, i32)> {
    (-ring..=ring).flat_map(move |dz| {
        (-ring..=ring)
            .filter(move |&dx| dx.abs() == ring || dz.abs() == ring)
            .map(move |dx| (dx, dz))
    })
}

/// The tiles around the player, filled nearest first.
#[derive(Default)]
pub struct Scanner {
    tiles: BTreeMap<(i32, i32), Tile>,
    palette: Option<Palette>,
}
impl Scanner {
    /// Forget everything (a new world or dimension, or another scan layer).
    pub fn clear(&mut self) {
        *self = Self::default();
    }
    /// Fill and refresh tiles around chunk `center` within `radius` chunks, nearest first, until
    /// `budget` is spent. Returns the tiles scanned.
    pub fn update(
        &mut self,
        world: &World,
        facts: &BlockFacts,
        center: (i32, i32),
        radius: i32,
        layer: Layer,
        budget: Duration,
    ) -> usize {
        let palette = self.palette.get_or_insert_with(|| Palette::new(facts));
        let started = Instant::now();
        let air = world.air.0;
        // Tiles of unloaded or distant chunks go.
        self.tiles.retain(|k, _| {
            (k.0 - center.0).abs() <= radius + 2
                && (k.1 - center.1).abs() <= radius + 2
                && world.chunks.contains_key(k)
        });
        let mut scanned = 0;
        for ring in 0..=radius {
            for (dx, dz) in ring_offsets(ring) {
                let key = (center.0 + dx, center.1 + dz);
                let Some(chunk) = world.chunks.get(&key) else {
                    continue;
                };
                let stale = match self.tiles.get(&key) {
                    None => true,
                    Some(t) if t.layer != layer => true,
                    Some(t) => t.revision != world.revision && t.scanned.elapsed() >= RESCAN,
                };
                if !stale {
                    continue;
                }
                let tile = scan_tile(chunk, palette, air, layer, world.revision);
                self.tiles.insert(key, tile);
                scanned += 1;
                if started.elapsed() >= budget {
                    return scanned;
                }
            }
        }
        scanned
    }
    /// The tiles scanned at or after `since`, as the map's tiles.
    pub fn tiles_since(
        &self,
        since: Instant,
    ) -> impl Iterator<Item = ((i32, i32), &[u8; 256], &[i16; 256], &[u8; 256])> {
        self.tiles
            .iter()
            .filter(move |(_, tile)| tile.scanned >= since)
            .map(|(key, tile)| (*key, &tile.color, &tile.height, &tile.depth))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // One state per id: air, grass (colour 1), stone (11), water, glass (no colour),
    // waterlogged slab (not sturdy on top), lava.
    fn palette() -> Palette {
        Palette::from_parts(&[
            (0, AIR),
            (1, STURDY_TOP),
            (11, STURDY_TOP),
            (WATER, 1),
            (0, 0),
            (11, 1),
            (LAVA, 2),
        ])
    }
    fn chunk(columns: &[(i32, i32, &[(i32, u32)])]) -> Chunk {
        let mut c = Chunk::empty(0, 0);
        for &(x, z, blocks) in columns {
            for &(y, id) in blocks {
                c.set(x, y, z, id);
            }
        }
        c
    }

    #[test]
    fn columns_take_the_first_coloured_block_and_fluids_over_it() {
        let p = palette();
        let c = chunk(&[
            (0, 0, &[(60, 2), (61, 1)]),
            // Glass has no map colour: the grass under it shows.
            (1, 0, &[(61, 1), (62, 4)]),
            // Three water over stone.
            (2, 0, &[(60, 2), (61, 3), (62, 3), (63, 3)]),
            (3, 0, &[(70, 6)]),
        ]);
        let top = top_of(&c, 0, Layer::Surface);
        assert_eq!(top, 79);
        assert_eq!(scan_column(&c, &p, 0, 0, top, Layer::Surface), (1, 61, 0));
        assert_eq!(scan_column(&c, &p, 1, 0, top, Layer::Surface), (1, 61, 0));
        assert_eq!(
            scan_column(&c, &p, 2, 0, top, Layer::Surface),
            (WATER, 63, 3)
        );
        assert_eq!(
            scan_column(&c, &p, 3, 0, top, Layer::Surface),
            (LAVA, 70, 1)
        );
        assert_eq!(scan_column(&c, &p, 4, 0, top, Layer::Surface).0, 0);
    }

    #[test]
    fn a_waterlogged_block_not_sturdy_on_top_shows_water() {
        let p = palette();
        let c = chunk(&[(0, 0, &[(60, 2), (61, 5)])]);
        assert_eq!(
            scan_column(&c, &p, 0, 0, 70, Layer::Surface),
            (WATER, 61, 1)
        );
    }

    #[test]
    fn below_a_height_shows_the_floor_and_cuts_walls() {
        let p = palette();
        // A cave: stone everywhere from 0 to 20, air at 10 and 11 in column 0.
        let mut blocks: Vec<(i32, u32)> = (0..=20).map(|y| (y, 2)).collect();
        let solid = blocks.clone();
        blocks.retain(|&(y, _)| y != 10 && y != 11);
        let c = chunk(&[(0, 0, &blocks), (1, 0, &solid)]);
        let layer = Layer::Below(11);
        let top = top_of(&c, 0, layer);
        assert_eq!(top, 11);
        assert_eq!(scan_column(&c, &p, 0, 0, top, layer), (11, 9, 0));
        assert_eq!(scan_column(&c, &p, 1, 0, top, layer), (11 | WALL, 11, 0));
        // The surface view sees the roof.
        assert_eq!(scan_column(&c, &p, 0, 0, 31, Layer::Surface), (11, 20, 0));
    }

    #[test]
    fn rings_cover_each_offset_once() {
        let mut seen = std::collections::HashSet::new();
        for r in 0..=3 {
            for o in ring_offsets(r) {
                assert!(seen.insert(o));
            }
        }
        assert_eq!(seen.len(), 49);
    }
}
