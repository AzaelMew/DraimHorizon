// Schematic ghosts: every block a shown preview still needs is drawn where it goes as its own
// block model, translucent and facing the way it is to be placed, and a block standing there
// wrongly as a red box. Built on the game thread one 16x16x16 section at a time, nearest first,
// within a time budget per tick; a section is built again only when its world section or the
// previews change. The render side (`ghost_render.rs`) draws the quads after the world.
//
// Building ghosts from the game's block models and hiding the faces between two ghost cubes
// follows Horizonmatica by keksi2k (MIT License), https://github.com/keksi2k/horizonmatica; its
// license is in ghost_render.rs and notices/LICENSE-horizonmatica.
use crate::game::{
    Game, claim_borders_schematic as model, claim_borders_schematic_places as places,
    claim_borders_schematic_view as view,
};
use horizon_client::mesher::Vertex;
use horizon_client_render::gfx::native_render::claim_borders_ghosts as gpu;
use horizon_world::registry::BlockStateId;
use std::{
    collections::{HashMap, HashSet},
    sync::{Arc, Mutex},
    time::{Duration, Instant},
};
/// How far around the player ghosts are built, in blocks.
const RADIUS: i32 = 64;
/// Time the ghosts may take per tick.
const BUDGET: Duration = Duration::from_millis(3);
/// The ghosts' opacity.
const ALPHA: u32 = 0xa0;
/// A block standing where the preview wants another: red, half seen through.
const WRONG: u32 = 0x90ff_4040;
/// Fully lit: ghosts show their textures' own colours.
const FULL_BRIGHT: u32 = 240 | (240 << 8);
type Section = Option<Arc<horizon_client::chunk::Section>>;
/// One shown preview: where it stands and the game's state for each of its distinct states,
/// turned as it is placed.
struct Shown {
    placement: places::Placement,
    shape: Arc<view::Shape>,
    ids: Vec<Option<BlockStateId>>,
    lo: places::Pos,
    hi: places::Pos,
    /// The layers drawn (the player's choice, counted from this preview's bottom).
    layers: (places::LayerMode, i32),
}
#[derive(Default)]
struct State {
    /// What the sections were built for: the previews (file, place, turn, which file read).
    key: Option<u64>,
    shown: Vec<Shown>,
    anchor: [i32; 3],
    /// The world section each built section was built against.
    built: HashMap<[i32; 3], Section>,
    /// Blocks still to place and blocks standing wrongly, per built section.
    counts: HashMap<[i32; 3], (u32, u32)>,
}
/// Blocks still to place and blocks standing wrongly within the ghosts' reach (64 blocks
/// around the player), for the preview panel: a file that already matches the world shows no
/// ghosts at all, and this says so.
pub(super) fn nearby_counts() -> (u32, u32) {
    let guard = STATE.lock().unwrap_or_else(|e| e.into_inner());
    guard.as_ref().map_or((0, 0), |s| {
        s.counts
            .values()
            .fold((0, 0), |(a, b), (c, d)| (a + c, b + d))
    })
}
static STATE: Mutex<Option<State>> = Mutex::new(None);
fn key_of(game: &Game<'_>, list: &[(places::Placement, Arc<view::Shape>, bool)]) -> u64 {
    use std::hash::{Hash, Hasher};
    let mut h = std::collections::hash_map::DefaultHasher::new();
    crate::game::claim_borders_navigation_view::scope(game).hash(&mut h);
    // The player's choice of layers: another choice builds the ghosts again.
    format!("{:?}", view::layers(game)).hash(&mut h);
    for (pl, shape, _) in list {
        (
            &pl.file,
            pl.origin,
            pl.rotation,
            Arc::as_ptr(shape) as usize,
        )
            .hash(&mut h);
    }
    h.finish()
}
/// The game's id for a schematic state; properties the game does not know are left out.
fn resolve(game: &Game<'_>, s: &model::BlockState) -> Option<BlockStateId> {
    let registry = &game.pack.assets.registry;
    let wanted = horizon_world::chunk::BlockState {
        name: s.name.clone(),
        properties: s.properties.clone().into(),
    };
    if let Ok(id) = registry.resolve(&wanted) {
        return Some(id);
    }
    let mut best = registry.default_state(&s.name)?;
    for (k, v) in &s.properties {
        let mut probe = registry.state(best)?.clone();
        if probe.properties.contains_key(k) {
            probe.properties.insert(k.clone(), v.clone());
            if let Some(id) = registry.id(&probe) {
                best = id;
            }
        }
    }
    Some(best)
}
fn world_section(game: &Game<'_>, k: [i32; 3]) -> Section {
    let chunk = game.world.chunks.get(&(k[0], k[2]))?;
    let i = chunk.section_index(k[1] * 16)?;
    chunk.sections.get(i).cloned()
}
fn same(a: &Section, b: &Section) -> bool {
    match (a, b) {
        (Some(a), Some(b)) => Arc::ptr_eq(a, b),
        (None, None) => true,
        _ => false,
    }
}
fn loaded(game: &Game<'_>, w: [i32; 3]) -> bool {
    let shape = &game.world.shape;
    w[1] >= shape.min_y
        && w[1] < shape.min_y + shape.height
        && game.world.chunks.contains_key(&(w[0] >> 4, w[2] >> 4))
}
/// What the previews want at `w`: the game's state (None: a state the game does not know).
/// A layer the player chose not to see wants nothing.
fn wanted(shown: &[Shown], w: [i32; 3]) -> Option<Option<BlockStateId>> {
    shown.iter().find_map(|s| {
        if (0..3).any(|i| w[i] < s.lo[i] || w[i] > s.hi[i])
            || !s.layers.0.shows(s.layers.1, w[1] - s.lo[1])
        {
            return None;
        }
        let rel = places::unplace(w, s.placement.origin, s.placement.rotation);
        let i = *s.shape.cells.get(&rel)?;
        Some(s.ids.get(i as usize).copied().flatten())
    })
}
/// Whether the block at `w` leaves room for what is to be placed there (air, grass, water...).
fn empty(game: &Game<'_>, w: [i32; 3]) -> bool {
    let name = game.native_block(w);
    matches!(
        name.as_str(),
        "minecraft:air" | "minecraft:cave_air" | "minecraft:void_air"
    ) || crate::game::claim_borders_navigation::passable(&name)
}
enum Verdict {
    Ghost(BlockStateId),
    Wrong,
}
/// What to draw at `w`: nothing where the right block stands (by name, as the builder checks it).
fn verdict(game: &Game<'_>, shown: &[Shown], w: [i32; 3]) -> Option<Verdict> {
    let id = wanted(shown, w)?;
    let facts = game.res.facts();
    let live = game.world.block(w[0], w[1], w[2]);
    if let Some(id) = id
        && (live == id || facts.get(live).name == facts.get(id).name)
    {
        return None;
    }
    if empty(game, w) {
        id.map(Verdict::Ghost)
    } else {
        Some(Verdict::Wrong)
    }
}
/// The colour with vanilla's directional shade (up 1, down 0.5, north/south 0.8, east/west 0.6).
fn shade(color: u32, normal: [i8; 4]) -> u32 {
    let [nx, ny, nz, _] = normal.map(i32::from);
    let shade = if ny > 64 {
        255
    } else if ny < -64 {
        127
    } else if nz.abs() >= nx.abs() {
        204
    } else {
        153
    };
    let [r, g, b, _] = color.to_le_bytes();
    let f = |c: u8| (u32::from(c) * shade / 255) as u8;
    u32::from_le_bytes([f(r), f(g), f(b), ALPHA as u8])
}
/// A box's six faces with outward normals, on the atlas's white texel.
fn push_box(out: &mut Vec<Vertex>, white: [f32; 2], min: [f32; 3], max: [f32; 3], argb: u32) {
    let [x0, y0, z0] = min;
    let [x1, y1, z1] = max;
    let faces: [([[f32; 3]; 4], [i8; 4]); 6] = [
        (
            [[x0, y1, z0], [x0, y1, z1], [x1, y1, z1], [x1, y1, z0]],
            [0, 127, 0, 127],
        ),
        (
            [[x0, y0, z0], [x1, y0, z0], [x1, y0, z1], [x0, y0, z1]],
            [0, -127, 0, 127],
        ),
        (
            [[x0, y0, z0], [x0, y1, z0], [x1, y1, z0], [x1, y0, z0]],
            [0, 0, -127, 127],
        ),
        (
            [[x0, y0, z1], [x1, y0, z1], [x1, y1, z1], [x0, y1, z1]],
            [0, 0, 127, 127],
        ),
        (
            [[x0, y0, z0], [x0, y0, z1], [x0, y1, z1], [x0, y1, z0]],
            [-127, 0, 0, 127],
        ),
        (
            [[x1, y0, z0], [x1, y1, z0], [x1, y1, z1], [x1, y0, z1]],
            [127, 0, 0, 127],
        ),
    ];
    let [a, r, g, b] = argb.to_be_bytes();
    for (corners, normal) in faces {
        let base = u32::from_le_bytes([r, g, b, 255]);
        let color = (shade(base, normal) & 0x00ff_ffff) | u32::from(a) << 24;
        for pos in corners {
            out.push(Vertex {
                pos,
                uv: white,
                layer: 0,
                color,
                light: FULL_BRIGHT,
                normal,
                glint_uv: [0.0, 0.0],
            });
        }
    }
}
/// The quads of section `k`.
fn build(
    game: &Game<'_>,
    shown: &[Shown],
    anchor: [i32; 3],
    k: [i32; 3],
    counts: &mut (u32, u32),
) -> Vec<Vertex> {
    let mut quads = Vec::new();
    let white = horizon_client::gfx::lightning::white_uv(&game.gui.atlas.font.white());
    let facts = game.res.facts();
    let mut cubes: HashMap<[i32; 3], bool> = HashMap::new();
    let mut ghost_cube = |w: [i32; 3]| -> bool {
        *cubes.entry(w).or_insert_with(|| {
            loaded(game, w)
                && matches!(verdict(game, shown, w), Some(Verdict::Ghost(id)) if facts.get(id).solid_render)
        })
    };
    let mut drawn = HashSet::new();
    for s in shown {
        let lo = [0, 1, 2].map(|i| (k[i] * 16).max(s.lo[i]));
        let hi = [0, 1, 2].map(|i| (k[i] * 16 + 15).min(s.hi[i]));
        for y in lo[1]..=hi[1] {
            for z in lo[2]..=hi[2] {
                for x in lo[0]..=hi[0] {
                    let w = [x, y, z];
                    // Overlapping previews: the block is drawn once.
                    if !loaded(game, w)
                        || !s.shape.cells.contains_key(&places::unplace(
                            w,
                            s.placement.origin,
                            s.placement.rotation,
                        ))
                        || !drawn.insert(w)
                    {
                        continue;
                    }
                    let rel = [0, 1, 2].map(|i| (w[i] - anchor[i]) as f32);
                    match verdict(game, shown, w) {
                        None => {}
                        Some(Verdict::Wrong) => {
                            counts.1 += 1;
                            let min = rel.map(|v| v - 0.004);
                            push_box(&mut quads, white, min, min.map(|v| v + 1.008), WRONG);
                        }
                        Some(Verdict::Ghost(id)) => {
                            counts.0 += 1;
                            let start = quads.len();
                            let at = glam::Mat4::from_translation(glam::Vec3::from_array(rel));
                            game.block_model_vertices(id, 0, at, FULL_BRIGHT, true, &mut quads);
                            game.special_block_model(id, at, 0xff, &mut quads);
                            let cube = facts.get(id).solid_render;
                            let centre = glam::Vec3::from_array(rel) + glam::Vec3::splat(0.5);
                            let mut kept = Vec::with_capacity(quads.len() - start);
                            for q in quads[start..].chunks_exact(4) {
                                let given = q[0].normal;
                                // Two-sided faces (w = -127) and faces without a normal are
                                // always drawn.
                                let normal = if given[3] == -127 || given[..3] == [0, 0, 0] {
                                    [0; 4]
                                } else {
                                    [given[0], given[1], given[2], 127]
                                };
                                if cube && normal[3] == 127 {
                                    let p = q.iter().fold(glam::Vec3::ZERO, |a, v| {
                                        a + glam::Vec3::from_array(v.pos)
                                    }) * 0.25;
                                    let out = p - centre;
                                    let axis = (0..3).find(|&i| normal[i].unsigned_abs() > 120);
                                    if let Some(i) = axis
                                        && (out[i].abs() - 0.5).abs() < 1e-3
                                    {
                                        let mut n = w;
                                        n[i] += i32::from(normal[i].signum());
                                        if ghost_cube(n) {
                                            continue;
                                        }
                                    }
                                }
                                for v in q {
                                    let mut v = *v;
                                    v.color = shade(v.color, given);
                                    v.normal = normal;
                                    kept.push(v);
                                }
                            }
                            quads.truncate(start);
                            quads.extend(kept);
                        }
                    }
                }
            }
        }
    }
    quads
}
/// Every tick: keeps the ghosts of the shown previews of this world up to date.
pub(super) fn tick(game: &Game<'_>) {
    let mut guard = STATE.lock().unwrap_or_else(|e| e.into_inner());
    let state = guard.get_or_insert_with(State::default);
    // Nothing while the mod is switched off (as the rest of its drawing), on the title screen
    // or with the interface hidden (F1).
    let list = if !game.native_on(crate::game::claim_borders::ID)
        || game.title.is_some()
        || game.ui.hide_gui
    {
        Vec::new()
    } else {
        view::shown(game)
    };
    let me = game.native_player().map(|p| [p.x, p.y, p.z]);
    let (Some(me), false) = (me, list.is_empty()) else {
        if state.key.take().is_some() || !state.built.is_empty() {
            state.built.clear();
            state.counts.clear();
            state.shown.clear();
            gpu::clear();
        }
        return;
    };
    let key = key_of(game, &list);
    if state.key != Some(key) {
        state.key = Some(key);
        let layers = view::layers(game);
        state.shown = list
            .into_iter()
            .map(|(placement, shape, _)| {
                let ids = shape
                    .states
                    .iter()
                    .map(|s| resolve(game, &s.rotated(placement.rotation)))
                    .collect();
                let (lo, hi) =
                    places::bounds(shape.min, shape.max, placement.origin, placement.rotation);
                Shown {
                    placement,
                    shape,
                    ids,
                    lo,
                    hi,
                    layers,
                }
            })
            .collect();
        state.anchor = state.shown.first().map_or([0; 3], |s| s.lo);
        state.built.clear();
        state.counts.clear();
        gpu::clear();
        gpu::set_anchor(state.anchor);
    }
    if gpu::take_lost() {
        state.built.clear();
        state.counts.clear();
    }
    let feet = me.map(|v| v.floor() as i32);
    let mut wanted: Vec<[i32; 3]> = Vec::new();
    let mut seen = HashSet::new();
    for s in &state.shown {
        let lo = [0, 1, 2].map(|i| s.lo[i].max(feet[i] - RADIUS) >> 4);
        let hi = [0, 1, 2].map(|i| s.hi[i].min(feet[i] + RADIUS) >> 4);
        for y in lo[1]..=hi[1] {
            for z in lo[2]..=hi[2] {
                for x in lo[0]..=hi[0] {
                    if seen.insert([x, y, z]) {
                        wanted.push([x, y, z]);
                    }
                }
            }
        }
    }
    let gone: Vec<[i32; 3]> = state
        .built
        .keys()
        .filter(|k| !seen.contains(*k))
        .copied()
        .collect();
    for k in gone {
        state.built.remove(&k);
        state.counts.remove(&k);
        gpu::put_section(k, Vec::new());
    }
    let distance = |k: &[i32; 3]| -> i64 {
        (0..3)
            .map(|i| i64::from(k[i] * 16 + 8 - feet[i]).pow(2))
            .sum()
    };
    wanted.sort_by_key(distance);
    let start = Instant::now();
    for k in wanted {
        let now = world_section(game, k);
        if state.built.get(&k).is_some_and(|old| same(old, &now)) {
            continue;
        }
        if start.elapsed() > BUDGET {
            break;
        }
        let mut counts = (0, 0);
        let quads = build(game, &state.shown, state.anchor, k, &mut counts);
        gpu::put_section(k, quads);
        state.built.insert(k, now);
        state.counts.insert(k, counts);
    }
}
