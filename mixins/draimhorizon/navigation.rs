// Conservative walking paths over actual loaded blocks. No mining, placing or flight.
use std::cmp::Reverse;
use std::collections::{BinaryHeap, HashMap};
use std::time::{Duration, Instant};

pub type Pos = [i32; 3];

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Hotkey {
    Preview,
    Start,
    Pause,
    Stop,
}
impl Hotkey {
    pub const COUNT: usize = 4;
    pub const ALL: [Self; Self::COUNT] = [Self::Preview, Self::Start, Self::Pause, Self::Stop];
    /// Marks an unassigned binding; such a key never triggers anything.
    pub const DEFAULT: &str = "key.keyboard.unknown";
    /// The numeric keypad is unused by the game, so the keys work out of the box without
    /// taking a gameplay key. They can be changed in Options > Mods.
    pub fn preset(self) -> &'static str {
        match self {
            Self::Preview => "key.keyboard.keypad.1",
            Self::Start => "key.keyboard.keypad.2",
            Self::Pause => "key.keyboard.keypad.3",
            Self::Stop => "key.keyboard.keypad.4",
        }
    }
    pub fn id(self) -> &'static str {
        match self {
            Self::Preview => "navigation_preview",
            Self::Start => "navigation_start",
            Self::Pause => "navigation_pause",
            Self::Stop => "navigation_stop",
        }
    }
    pub fn label(
        self,
        language: claim_borders_relay_client::translation::Language,
    ) -> &'static str {
        match self {
            Self::Preview => language.text("Baritone: просмотр", "Baritone: preview"),
            Self::Start => {
                language.text("Baritone: старт / продолжить", "Baritone: start / resume")
            }
            Self::Pause => language.text("Baritone: пауза", "Baritone: pause"),
            Self::Stop => language.text("Baritone: остановить", "Baritone: stop"),
        }
    }
}

pub fn hotkey(
    key: &str,
    pressed: bool,
    eligible: bool,
    conflict: bool,
    bindings: [&str; Hotkey::COUNT],
) -> Option<Hotkey> {
    if !pressed || !eligible || conflict || key == Hotkey::DEFAULT {
        return None;
    }
    let mut matches = bindings
        .into_iter()
        .zip(Hotkey::ALL)
        .filter_map(|(binding, action)| (binding == key).then_some(action));
    let action = matches.next()?;
    matches.next().is_none().then_some(action)
}
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Goal {
    Coordinates(Pos),
    Waypoint(u64),
    Follow(u128),
    /// `ore` is a target index, or `mining::ANY` for every bit of `targets`.
    Mine {
        ore: u8,
        targets: u32,
        limit: u16,
    },
    Construct(super::claim_borders_construction::Config),
    Farm(super::claim_borders_farming::Config),
    Drain(super::claim_borders_draining::Config),
}
pub const RADIUS: i32 = 32;
const MAX_NODES: usize = 8192;
/// How far a walking search looks: around the start, with a node and a time budget. Near is the
/// everyday search; wide is for a goal far off or at another height (a farm under a second
/// floor whose only way down is a slope forty blocks away), run on the planner thread.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Reach {
    pub radius: i32,
    pub nodes: usize,
    pub millis: u64,
}
pub const NEAR: Reach = Reach {
    radius: RADIUS,
    nodes: MAX_NODES,
    millis: 80,
};
pub const WIDE: Reach = Reach {
    radius: 160,
    nodes: 400_000,
    millis: 1500,
};
/// Whether a walk from `start` to `goal` needs the wide search: far off, or at another height.
pub fn wide_needed(start: Pos, goal: Pos) -> bool {
    (start[0] - goal[0]).abs().max((start[2] - goal[2]).abs()) > 24
        || (start[1] - goal[1]).abs() > 3
}
thread_local! {
    static REACH: std::cell::Cell<Reach> = const { std::cell::Cell::new(NEAR) };
}
/// Sets how far the walking searches on this thread look (the planner thread sets it from each
/// request).
pub fn set_reach(reach: Reach) {
    REACH.with(|r| r.set(reach));
}
fn current_reach() -> Reach {
    REACH.with(|r| r.get())
}
/// Remembered routes: a walk that arrived is kept as the cells it went through, and a later
/// walk to either end, starting near the route, follows it instead of searching again. The
/// returned points are every `every`-th cell from the one nearest to `from` toward the end near
/// `goal`, that end included; `None` when the route does not serve this walk.
pub fn route_via(points: &[Pos], from: Pos, goal: Pos, every: usize) -> Option<Vec<Pos>> {
    let near = |a: Pos, b: Pos, h: i32, v: i32| {
        (a[0] - b[0]).abs().max((a[2] - b[2]).abs()) <= h && (a[1] - b[1]).abs() <= v
    };
    let (first, last) = (*points.first()?, *points.last()?);
    let forward: Vec<Pos> = if near(last, goal, 4, 2) {
        points.to_vec()
    } else if near(first, goal, 4, 2) {
        points.iter().rev().copied().collect()
    } else {
        return None;
    };
    let start = forward
        .iter()
        .enumerate()
        .filter(|(_, p)| near(**p, from, 5, 1))
        .min_by_key(|(_, p)| (p[0] - from[0]).abs() + (p[2] - from[2]).abs())?
        .0;
    let end = forward.len() - 1;
    if start >= end {
        return None;
    }
    let mut via: Vec<Pos> = (start + every..end)
        .step_by(every.max(1))
        .map(|i| forward[i])
        .collect();
    via.push(forward[end]);
    Some(via)
}
/// Whether a far walk goes back and forth: the next leg ends where the leg before the last one
/// ended (within four blocks), so the walk would turn round again.
pub fn ping_pong(ends: &[Pos], next: Pos) -> bool {
    let near = |a: Pos, b: Pos| {
        (a[0] - b[0]).abs().max((a[2] - b[2]).abs()) <= 4 && (a[1] - b[1]).abs() <= 2
    };
    ends.len() >= 2 && near(ends[ends.len() - 2], next) && !near(ends[ends.len() - 1], next)
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Cell {
    Unknown,
    Air,
    Solid,
    Blocked,
    Hazard,
    /// A door or a fence gate that can be opened by hand: walked through once opened (the
    /// walker opens it on the way). Iron doors are not opened by hand and are `Blocked`.
    Door,
    /// A ladder or vines: the body fits in it and climbs up or down without a floor.
    Climb,
    /// Water (or a waterlogged block): swum in, not walked; harmless beside a place to stand
    /// (unlike lava, a `Hazard`).
    Water,
}
/// Whether the body fits in the cell: open air, a plant, a door to open, a ladder.
pub fn open(cell: Cell) -> bool {
    matches!(cell, Cell::Air | Cell::Door | Cell::Climb)
}
/// Doors and fence gates a hand opens (not iron doors): the planner's `Door` cells.
pub fn hand_door(name: &str) -> bool {
    let name = name.strip_prefix("minecraft:").unwrap_or(name);
    (name.ends_with("_door") && name != "iron_door") || name.ends_with("_fence_gate")
}
/// Trapdoors a hand opens (not iron ones).
pub fn hand_trapdoor(name: &str) -> bool {
    let name = name.strip_prefix("minecraft:").unwrap_or(name);
    name.ends_with("_trapdoor") && name != "iron_trapdoor"
}
/// Everything the walker opens by hand on its way: doors, fence gates and trapdoors.
pub fn openable(name: &str) -> bool {
    hand_door(name) || hand_trapdoor(name)
}
/// Blocks a body climbs in (the `climbable` tag, scaffolding aside).
pub fn climbable(name: &str) -> bool {
    let name = name.strip_prefix("minecraft:").unwrap_or(name);
    matches!(
        name,
        "ladder"
            | "vine"
            | "weeping_vines"
            | "weeping_vines_plant"
            | "twisting_vines"
            | "twisting_vines_plant"
            | "cave_vines"
            | "cave_vines_plant"
    )
}
/// Blocks that are taller than they look (their collision is a block and a half): never
/// stood on and never walked through, whatever their outline says.
fn tall_collision(name: &str) -> bool {
    name.ends_with("_fence") || name.ends_with("_wall") || name.ends_with("_fence_gate")
}

/// Unknown/partial collision shapes are obstacles in this first walking implementation.
pub fn classify(name: &str, full_collision: bool, fluid: bool) -> Cell {
    let name = name.strip_prefix("minecraft:").unwrap_or(name);
    if (fluid && name != "lava") || matches!(name, "water" | "bubble_column") {
        return Cell::Water;
    }
    if fluid
        || matches!(
            name,
            "lava"
                | "water"
                | "fire"
                | "soul_fire"
                | "magma_block"
                | "cactus"
                | "sweet_berry_bush"
                | "wither_rose"
                | "powder_snow"
                | "campfire"
                | "soul_campfire"
                | "nether_portal"
                | "end_portal"
                | "end_gateway"
                | "cobweb"
                | "slime_block"
                | "honey_block"
        )
    {
        Cell::Hazard
    } else if matches!(name, "air" | "cave_air" | "void_air") {
        Cell::Air
    } else if hand_door(name) {
        Cell::Door
    } else if climbable(name) {
        Cell::Climb
    } else if full_collision || matches!(name, "farmland" | "dirt_path") {
        Cell::Solid
    } else if passable(name) {
        Cell::Air
    } else {
        Cell::Blocked
    }
}

/// How high a player steps up without jumping (`Player.maxUpStep`).
pub const STEP_HEIGHT: f64 = 0.6;
/// Whether a block's shape (boxes in block space, `[min x, y, z, max x, y, z]`) is low enough to
/// step onto without jumping: a bottom slab, a carpet, a snow layer, a pressure plate, a bed, a
/// stonecutter. Such a block is walked over like the floor it stands on.
pub fn low_shape(outline: &[[f64; 6]]) -> bool {
    !outline.is_empty() && outline.iter().all(|b| b[4] <= STEP_HEIGHT + 1e-6)
}
/// Whether a shape is a block to stand on although it is not a full cube: its top is at most a
/// block high and it covers nearly the whole cell (a chest, a composter, a cauldron, a hopper,
/// an enchanting table, a top slab, a closed top trapdoor). A fence with four arms covers the
/// cell too, so the tall blocks are left out by name.
pub fn platform_shape(outline: &[[f64; 6]]) -> bool {
    if outline.is_empty() || outline.iter().any(|b| b[4] > 1.0 + 1e-6) {
        return false;
    }
    let (mut x0, mut z0, mut x1, mut z1) = (1.0f64, 1.0f64, 0.0f64, 0.0f64);
    for b in outline {
        x0 = x0.min(b[0]);
        z0 = z0.min(b[2]);
        x1 = x1.max(b[3]);
        z1 = z1.max(b[5]);
    }
    x1 - x0 >= 0.8 && z1 - z0 >= 0.8
}
/// `classify`, with the shapes it cannot judge by name looked up in the block's outline: stairs
/// and top slabs are blocks to stand on, and a low shape is `low` (open to walk through for the
/// walking planner, solid for the passages that dig and build).
pub fn classify_shape(
    name: &str,
    full_collision: bool,
    fluid: bool,
    outline: &[[f64; 6]],
    low: Cell,
) -> Cell {
    let cell = classify(name, full_collision, fluid);
    if cell != Cell::Blocked {
        return cell;
    }
    let short = name.strip_prefix("minecraft:").unwrap_or(name);
    if short == "snow" {
        // Snow layers: the body stands on one layer less than they look (eight layers are a full
        // block). Up to five layers are walked over, six and seven stood on like a block.
        let top = outline.iter().map(|b| b[4]).fold(0.0f64, f64::max);
        return if top - 0.125 <= STEP_HEIGHT + 1e-6 {
            low
        } else {
            Cell::Solid
        };
    }
    if hand_trapdoor(short) {
        // A wooden trapdoor: shut on the floor it is walked over; shut at the top of its cell
        // (a hatch at head height, over a ladder) it is opened on the way; open, it stands up
        // against one side and the body passes it.
        return if low_shape(outline) {
            low
        } else if platform_shape(outline) {
            Cell::Door
        } else {
            Cell::Air
        };
    }
    if tall_collision(short) || matches!(short, "iron_bars" | "chain") || short.ends_with("_pane") {
        Cell::Blocked
    } else if short.ends_with("_stairs") {
        Cell::Solid
    } else if low_shape(outline) {
        low
    } else if short.ends_with("_slab") || platform_shape(outline) {
        // A top slab, a chest, a composter: a floor a block high.
        Cell::Solid
    } else {
        Cell::Blocked
    }
}

/// Blocks with no collision shape that a player walks straight through: grass, ferns, flowers,
/// crops, saplings and similar plants, torches, rails and redstone parts. Treated as open air
/// (they are replaced when something is placed there, or broken in an instant).
pub fn passable(name: &str) -> bool {
    let name = name.strip_prefix("minecraft:").unwrap_or(name);
    name.ends_with("grass")
        || name.ends_with("fern")
        || name.ends_with("sapling")
        || name.ends_with("_roots")
        || name.ends_with("torch")
        || name.ends_with("_button")
        || name.ends_with("rail")
        || name.ends_with("tulip")
        || name.ends_with("_stem")
        || matches!(
            name,
            "dead_bush"
                | "poppy"
                | "dandelion"
                | "blue_orchid"
                | "allium"
                | "azure_bluet"
                | "oxeye_daisy"
                | "cornflower"
                | "lily_of_the_valley"
                | "torchflower"
                | "sunflower"
                | "lilac"
                | "rose_bush"
                | "peony"
                | "pitcher_plant"
                | "pink_petals"
                | "wildflowers"
                | "leaf_litter"
                | "red_mushroom"
                | "brown_mushroom"
                | "sugar_cane"
                | "wheat"
                | "carrots"
                | "potatoes"
                | "beetroots"
                | "nether_wart"
                | "nether_sprouts"
                | "glow_lichen"
                | "lever"
                | "string"
                | "tripwire"
                | "redstone_wire"
                | "snow_layer_zero"
        )
}

/// Whether the body can be at `at`: it fits there (air, a plant, a door, a ladder, with room
/// for the head) and is held up, by a floor or by the ladder it is in, with no hazard beside,
/// diagonally either (a corner of lava is slipped into as easily as its side).
pub fn standable(at: Pos, cell: &mut impl FnMut(Pos) -> Cell) -> bool {
    let feet = cell(at);
    open(feet)
        && open(cell([at[0], at[1] + 1, at[2]]))
        && (feet == Cell::Climb
            || matches!(cell([at[0], at[1] - 1, at[2]]), Cell::Solid | Cell::Climb))
        && [
            [1, 0],
            [-1, 0],
            [0, 1],
            [0, -1],
            [1, 1],
            [1, -1],
            [-1, 1],
            [-1, -1],
        ]
        .into_iter()
        .all(|[x, z]| (-1..=1).all(|y| cell([at[0] + x, at[1] + y, at[2] + z]) != Cell::Hazard))
}

/// How far a leg of a far walk goes, and the shorter reach tried when the long one has no way.
pub const LEG_REACH: [f64; 2] = [140.0, 60.0];
/// How many directions a leg is tried in: straight at the goal, then turned to either side.
const LEG_TURNS: [f64; 7] = [0.0, 35.0, -35.0, 70.0, -70.0, 110.0, -110.0];
/// How many different ends of a leg there are to try.
pub const LEG_TRIES: usize = LEG_TURNS.len() * LEG_REACH.len();
/// The `attempt`-th point to end a leg of a walk from `from` toward `goal`: straight on at the
/// full reach first, then turned aside (a cliff, a river, a wall in the way), then the same at
/// the shorter reach. Only the horizontal position matters; the caller finds ground at it.
pub fn leg_target(from: [f64; 3], goal: [f64; 3], attempt: usize) -> Option<[f64; 3]> {
    if attempt >= LEG_TRIES {
        return None;
    }
    let reach = LEG_REACH[attempt / LEG_TURNS.len()];
    let turn = LEG_TURNS[attempt % LEG_TURNS.len()].to_radians();
    let (dx, dz) = (goal[0] - from[0], goal[2] - from[2]);
    let length = dx.hypot(dz);
    if length < 1e-6 {
        return None;
    }
    let (ux, uz) = (dx / length, dz / length);
    let (sin, cos) = turn.sin_cos();
    let (vx, vz) = (ux * cos - uz * sin, ux * sin + uz * cos);
    Some([from[0] + vx * reach, from[1], from[2] + vz * reach])
}
/// The longest drop a walk steps off as a matter of course: three blocks cost no health, and
/// dunes and pits are full of them.
pub const MAX_DROP: i32 = 3;
/// The longest drop the planner ever considers (with fall protection worn); how far a walk may
/// really drop is `drop_limit`, the same rule for every task.
pub const FAR_DROP: i32 = 12;
/// Fall damage of a drop of `drop` blocks, in health points (two to a heart): a point a block
/// beyond three, less 4 % per point of enchantment protection against falls (`fall_protection`),
/// at most 80 % less, as `CombatRules.getDamageAfterMagicAbsorb` takes it.
pub fn fall_damage(drop: i32, protection: u32) -> f32 {
    (drop - MAX_DROP).max(0) as f32 * (1.0 - protection.min(20) as f32 * 0.04)
}
/// Enchantment protection against falls from what is worn: Feather Falling gives three a level
/// (boots), Protection one a level (every piece). `levels` holds each piece's two levels.
pub fn fall_protection(levels: &[(u32, u32)]) -> u32 {
    levels
        .iter()
        .map(|(feather, protection)| feather * 3 + protection)
        .sum::<u32>()
        .min(20)
}
/// How far a walk may drop, for every task alike: three blocks always (no damage); more only
/// when there is no other way down, with health above ten, and only as far as costs at most
/// three points and leaves ten. Boots with Feather Falling make the same health go further.
pub fn drop_limit(health: f32, protection: u32) -> i32 {
    let budget = (health - 10.0).clamp(0.0, 3.0);
    (MAX_DROP..=FAR_DROP)
        .rev()
        .find(|d| fall_damage(*d, protection) <= budget + 1e-3)
        .unwrap_or(MAX_DROP)
}
thread_local! {
    /// The drop limit of the walk being planned on this thread (`set_drop_limit`).
    static DROP_LIMIT: std::cell::Cell<i32> = const { std::cell::Cell::new(MAX_DROP) };
}
/// Sets the drop limit the planners on this thread use (the planner thread sets it from each
/// request), so walking, following, mining, farm, drain and schematics all go by the same rule.
pub fn set_drop_limit(limit: i32) {
    DROP_LIMIT.with(|d| d.set(limit.clamp(MAX_DROP, ELYTRA_DROP)));
}
/// The drop limit with elytra worn: any height of the world (they are glided down).
pub const ELYTRA_DROP: i32 = 384;
fn current_drop_limit() -> i32 {
    DROP_LIMIT.with(|d| d.get())
}
/// What each block of a drop beyond `MAX_DROP` costs a route: a step and a half, so a drop (within
/// the safe height) beats a way round that is longer, and stairs on the way beat the drop.
const FAR_DROP_COST: i32 = 15;
/// The place to walk to for a goal given as a block: the goal itself when it can be stood in,
/// otherwise the nearest place to stand in its column, up to three blocks up or down (a goal
/// typed as the block under the feet, a waypoint saved on the ground). `None` when there is none.
pub fn standing_goal(goal: Pos, cell: &mut impl FnMut(Pos) -> Cell) -> Option<Pos> {
    [0, 1, -1, 2, -2, 3, -3]
        .into_iter()
        .map(|d| [goal[0], goal[1] + d, goal[2]])
        .find(|at| standable(*at, cell))
}
/// Where to plan from when the player does not stand on firm ground by the planner's rule: on
/// an edge with the middle over a gap, on a low block, in the air after a jump or a fall. The
/// cell itself, one up, then down to the drop limit, then the neighbours; `None` when there is
/// nowhere to stand around.
pub fn nearest_standable(start: Pos, cell: &mut impl FnMut(Pos) -> Cell) -> Option<Pos> {
    let mut offsets = vec![[0, 0, 0], [0, 1, 0]];
    offsets.extend((1..=MAX_DROP).map(|d| [0, -d, 0]));
    for [dx, dz] in [[1, 0], [-1, 0], [0, 1], [0, -1]] {
        for dy in [0, 1, -1, -2] {
            offsets.push([dx, dy, dz]);
        }
    }
    offsets
        .into_iter()
        .map(|d| [start[0] + d[0], start[1] + d[1], start[2] + d[2]])
        .find(|at| standable(*at, cell))
}
/// One move of a walk: a step to a side (up one, level, or down to `MAX_DROP`), a step across a
/// corner (level or down, both sides of the corner open so the body does not catch on them), or
/// a climb up or down a ladder.
pub fn traversable(from: Pos, to: Pos, cell: &mut impl FnMut(Pos) -> Cell) -> bool {
    let (dx, dy, dz) = (to[0] - from[0], to[1] - from[1], to[2] - from[2]);
    if !standable(to, cell) {
        return false;
    }
    let door = |c: Cell| c == Cell::Door;
    // Hanging vines (a climbable cell with nothing firm under it) are climbed up or down, not
    // walked along from one to the next over a drop.
    let hanging = |at: Pos, cell: &mut dyn FnMut(Pos) -> Cell| {
        cell(at) == Cell::Climb && cell([at[0], at[1] - 1, at[2]]) != Cell::Solid
    };
    if (dx != 0 || dz != 0) && hanging(from, cell) && hanging(to, cell) {
        return false;
    }
    let column_open = |cell: &mut dyn FnMut(Pos) -> Cell| {
        // Stepping off an edge: the whole column down to the landing is open.
        dy >= 0 || (to[1] + 1..=from[1] + 1).all(|y| open(cell([to[0], y, to[2]])))
    };
    match (dx.abs(), dz.abs()) {
        // Up or down a ladder (or out of the top of one onto what it leans on: a side step).
        (0, 0) => {
            dy.abs() == 1
                && if dy > 0 {
                    cell(from) == Cell::Climb
                } else {
                    cell(to) == Cell::Climb
                }
        }
        (1, 0) | (0, 1) => {
            (-FAR_DROP.max(current_drop_limit())..=1).contains(&dy)
                // A door is passed on the level, never with a step.
                && (dy == 0 || !(door(cell(from)) || door(cell(to))))
                && (dy <= 0 || open(cell([from[0], from[1] + 2, from[2]])))
                && column_open(cell)
        }
        // A running jump across a gap: two to four blocks straight on, level or one down, the
        // air above the gap clear for the arc, at least one cell of it without a floor.
        (2..=4, 0) | (0, 2..=4) if long_jumps() => {
            let length = dx.abs().max(dz.abs());
            let (sx, sz) = (dx.signum(), dz.signum());
            (-1..=0).contains(&dy)
                && cell(from) == Cell::Air
                && cell([from[0], from[1] + 2, from[2]]) == Cell::Air
                && cell([to[0], to[1] + 2, to[2]]) == Cell::Air
                && (1..length).all(|k| {
                    let (x, z) = (from[0] + sx * k, from[2] + sz * k);
                    (0..=2).all(|y| cell([x, from[1] + y, z]) == Cell::Air)
                })
                && (1..length)
                    .any(|k| cell([from[0] + sx * k, from[1] - 1, from[2] + sz * k]) != Cell::Solid)
        }
        (1, 1) => {
            (-MAX_DROP..=0).contains(&dy)
                && [cell(from), cell(to)].iter().all(|c| *c == Cell::Air)
                && [[to[0], from[2]], [from[0], to[2]]]
                    .iter()
                    .all(|[x, z]| (0..=1).all(|y| cell([*x, from[1] + y, *z]) == Cell::Air))
                && column_open(cell)
        }
        _ => false,
    }
}

/// One move of a route that may break and place blocks: what to remove and what to
/// build before stepping onto `to`. The native executor revalidates all of it live.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DigStep {
    pub to: Pos,
    pub dig: Vec<Pos>,
    pub place: Option<Pos>,
}
#[derive(Clone, Debug)]
pub struct DigPath {
    pub start: Pos,
    pub steps: Vec<DigStep>,
    pub complete: bool,
}
const DIG_COST: i32 = 45;
const PLACE_COST: i32 = 35;

/// A single horizontal move that may climb or drop one block. Blocks that stand in the way
/// are dug when `diggable` allows it; a missing floor is bridged when `can_place`. Fluids,
/// unknown chunks and partial blocks next to the route make the move invalid.
pub fn dig_step(
    from: Pos,
    to: Pos,
    can_place: bool,
    cell: &mut impl FnMut(Pos) -> Cell,
    diggable: &mut impl FnMut(Pos) -> bool,
) -> Option<DigStep> {
    let dy = to[1] - from[1];
    if (to[0] - from[0]).abs() + (to[2] - from[2]).abs() != 1 || dy.abs() > 1 {
        return None;
    }
    for [dx, dz] in [[0, 0], [1, 0], [-1, 0], [0, 1], [0, -1]] {
        for y in -1..=2 {
            if matches!(
                cell([to[0] + dx, to[1] + y, to[2] + dz]),
                Cell::Hazard | Cell::Unknown | Cell::Water
            ) {
                return None;
            }
        }
    }
    let mut needed = vec![to, [to[0], to[1] + 1, to[2]]];
    if dy > 0 {
        needed.push([from[0], from[1] + 2, from[2]]);
    }
    if dy < 0 {
        needed.push([to[0], to[1] + 2, to[2]]);
    }
    let mut dig = Vec::new();
    for at in needed {
        match cell(at) {
            c if open(c) => {}
            Cell::Solid if diggable(at) => {
                for delta in [
                    [1, 0, 0],
                    [-1, 0, 0],
                    [0, 1, 0],
                    [0, -1, 0],
                    [0, 0, 1],
                    [0, 0, -1],
                ] {
                    if matches!(
                        cell([at[0] + delta[0], at[1] + delta[1], at[2] + delta[2]]),
                        Cell::Hazard | Cell::Unknown | Cell::Water
                    ) {
                        return None;
                    }
                }
                dig.push(at);
            }
            _ => return None,
        }
    }
    // Break from the top down so nothing above settles into the cleared space.
    dig.sort_by_key(|at| Reverse(at[1]));
    let support = [to[0], to[1] - 1, to[2]];
    let place = match cell(support) {
        Cell::Solid => None,
        Cell::Air if can_place && dy == 0 => Some(support),
        _ => return None,
    };
    Some(DigStep { to, dig, place })
}

/// Bounded A* that may dig through rock and bridge gaps. With `adjacent`, standing next to
/// `goal` (its column, at feet or head height) counts as arrival, which is what mining an
/// ore needs. The result is a plan: every action is checked again against the live world.
pub fn search_dig(
    start: Pos,
    goal: Pos,
    adjacent: bool,
    can_place: bool,
    mut cell: impl FnMut(Pos) -> Cell,
    mut diggable: impl FnMut(Pos) -> bool,
    canceled: impl Fn() -> bool,
) -> Option<DigPath> {
    let reached = |at: Pos| {
        at == goal
            || (adjacent
                && (at[0] - goal[0]).abs() + (at[2] - goal[2]).abs() == 1
                && (0..=1).contains(&(goal[1] - at[1])))
    };
    let estimate = |at: Pos| {
        let horizontal = (at[0] - goal[0]).abs() + (at[2] - goal[2]).abs();
        let vertical = (at[1] - goal[1]).abs();
        let horizontal = if adjacent {
            (horizontal - 1).max(0)
        } else {
            horizontal
        };
        horizontal.max(vertical) * 10
    };
    if !standable(start, &mut cell) {
        return None;
    }
    let began = Instant::now();
    let mut open = BinaryHeap::from([Reverse((estimate(start), 0, start))]);
    let mut costs = HashMap::from([(start, 0)]);
    let mut parents: HashMap<Pos, (Pos, DigStep)> = HashMap::new();
    let mut best = start;
    let mut expanded = 0;
    while let Some(Reverse((_, cost, at))) = open.pop() {
        if canceled() {
            return None;
        }
        if cost != costs[&at] {
            continue;
        }
        if estimate(at) < estimate(best) {
            best = at;
        }
        if reached(at) {
            best = at;
            break;
        }
        expanded += 1;
        if expanded >= MAX_NODES || began.elapsed() > Duration::from_millis(150) {
            break;
        }
        for [dx, dz] in [[1, 0], [-1, 0], [0, 1], [0, -1]] {
            for dy in [0, 1, -1] {
                let next = [at[0] + dx, at[1] + dy, at[2] + dz];
                if (next[0] - start[0]).abs() > RADIUS
                    || (next[2] - start[2]).abs() > RADIUS
                    || (next[1] - start[1]).abs() > 48
                {
                    continue;
                }
                let Some(step) = dig_step(at, next, can_place, &mut cell, &mut diggable) else {
                    continue;
                };
                let next_cost = cost
                    + 10
                    + if dy > 0 { 4 } else { dy.abs() * 2 }
                    + step.dig.len() as i32 * DIG_COST
                    + if step.place.is_some() { PLACE_COST } else { 0 };
                if costs.get(&next).is_none_or(|old| next_cost < *old) {
                    costs.insert(next, next_cost);
                    parents.insert(next, (at, step));
                    open.push(Reverse((next_cost + estimate(next), next_cost, next)));
                }
            }
        }
    }
    if canceled() || (best == start && !reached(start)) {
        return None;
    }
    let mut steps = Vec::new();
    let mut at = best;
    while let Some((parent, step)) = parents.get(&at) {
        steps.push(step.clone());
        at = *parent;
        if steps.len() > MAX_NODES {
            return None;
        }
    }
    steps.reverse();
    Some(DigPath {
        start,
        steps,
        complete: reached(best),
    })
}

/// Octile distance: a step to a side costs 10, one across a corner 14.
fn heuristic(a: Pos, b: Pos) -> i32 {
    let (x, z) = ((a[0] - b[0]).abs(), (a[2] - b[2]).abs());
    // Five a level: a drop of three costs 16, so the estimate stays below the real cost, and a
    // partial route ends nearer the goal's height (not on a roof straight above it).
    x.max(z) * 10 + x.min(z) * 4 + (a[1] - b[1]).abs() * 5
}
/// Every move tried from a cell, with its base cost: the four sides (up one, level, down to the
/// drop limit), the four corners (level or down) and a ladder up or down.
const MOVES: [([i32; 3], i32); 98] = {
    let mut moves = [([0, 0, 0], 0); 98];
    let sides = [[1, 0], [-1, 0], [0, 1], [0, -1]];
    let corners = [[1, 1], [1, -1], [-1, 1], [-1, -1]];
    let mut i = 0;
    let mut s = 0;
    while s < 4 {
        let mut dy = 1;
        while dy >= -FAR_DROP {
            moves[i] = ([sides[s][0], dy, sides[s][1]], 10);
            i += 1;
            dy -= 1;
        }
        s += 1;
    }
    let mut c = 0;
    while c < 4 {
        let mut dy = 0;
        while dy >= -MAX_DROP {
            moves[i] = ([corners[c][0], dy, corners[c][1]], 14);
            i += 1;
            dy -= 1;
        }
        c += 1;
    }
    moves[i] = ([0, 1, 0], 12);
    moves[i + 1] = ([0, -1, 0], 12);
    i += 2;
    // Running jumps across gaps: two to four blocks, level or one down; dearer than walking.
    let mut s = 0;
    while s < 4 {
        let mut length = 2;
        while length <= 4 {
            let mut dy = 0;
            while dy >= -1 {
                moves[i] = (
                    [sides[s][0] * length, dy, sides[s][1] * length],
                    10 * length + 6,
                );
                i += 1;
                dy -= 1;
            }
            length += 1;
        }
        s += 1;
    }
    moves
};
thread_local! {
    static LONG_JUMPS: std::cell::Cell<bool> = const { std::cell::Cell::new(true) };
}
/// Whether routes on this thread may jump across gaps (not on a farm's level routes, which
/// never jump).
pub fn set_long_jumps(on: bool) {
    LONG_JUMPS.with(|l| l.set(on));
}
fn long_jumps() -> bool {
    LONG_JUMPS.with(|l| l.get())
}
#[derive(Clone, Debug)]
pub struct Path {
    pub nodes: Vec<Pos>,
    pub complete: bool,
}

/// Bounded A*, computed on a worker. A partial result must improve distance to the goal.
pub fn search(
    start: Pos,
    goal: Pos,
    cell: impl FnMut(Pos) -> Cell,
    canceled: impl Fn() -> bool,
) -> Option<Path> {
    search_near(start, goal, 0, cell, canceled)
}

pub fn search_near(
    start: Pos,
    goal: Pos,
    radius: u8,
    cell: impl FnMut(Pos) -> Cell,
    canceled: impl Fn() -> bool,
) -> Option<Path> {
    search_near_drop(start, goal, radius, current_drop_limit(), cell, canceled)
}
/// A fast hasher for block positions (the multiply-rotate of FxHash): the search keeps a few
/// hundred thousand of them, and the standard hasher was a good part of its time.
#[derive(Default, Clone, Copy)]
pub struct PosHasher(u64);
impl std::hash::Hasher for PosHasher {
    fn finish(&self) -> u64 {
        self.0
    }
    fn write(&mut self, bytes: &[u8]) {
        // A position comes as one write of its twelve bytes: taken eight at a time.
        let (words, rest) = bytes.as_chunks::<8>();
        for word in words {
            self.write_u64(u64::from_le_bytes(*word));
        }
        if !rest.is_empty() {
            let mut word = [0; 8];
            word[..rest.len()].copy_from_slice(rest);
            self.write_u64(u64::from_le_bytes(word));
        }
    }
    fn write_u64(&mut self, v: u64) {
        self.0 = (self.0.rotate_left(5) ^ v).wrapping_mul(0x51_7c_c1_b7_27_22_0a_95);
    }
    fn write_i32(&mut self, v: i32) {
        self.write_u64(u64::from(v as u32));
    }
    fn write_u32(&mut self, v: u32) {
        self.write_u64(u64::from(v));
    }
    fn write_usize(&mut self, v: usize) {
        self.write_u64(v as u64);
    }
}
pub type PosMap<V> = HashMap<Pos, V, std::hash::BuildHasherDefault<PosHasher>>;
/// The cells a search looks at, each read from the world once: a box around the start, filled
/// as the search goes (a cell is read again and again, by every move that passes it); anything
/// outside the box is read directly.
pub struct CellCache<F: FnMut(Pos) -> Cell> {
    lo: Pos,
    size: [i32; 3],
    data: Vec<u8>,
    read: F,
}
const CELLS: [Cell; 8] = [
    Cell::Unknown,
    Cell::Air,
    Cell::Solid,
    Cell::Blocked,
    Cell::Hazard,
    Cell::Door,
    Cell::Climb,
    Cell::Water,
];
impl<F: FnMut(Pos) -> Cell> CellCache<F> {
    /// The box from `lo` to `hi` (inclusive); a box too large to keep is not kept.
    pub fn new(lo: Pos, hi: Pos, read: F) -> Self {
        let size = [0, 1, 2].map(|i| (hi[i] - lo[i] + 1).max(0));
        let volume = size.iter().map(|v| *v as usize).product::<usize>();
        let (size, data) = if volume <= 24_000_000 {
            (size, vec![u8::MAX; volume])
        } else {
            ([0; 3], Vec::new())
        };
        Self {
            lo,
            size,
            data,
            read,
        }
    }
    pub fn get(&mut self, at: Pos) -> Cell {
        let l = [at[0] - self.lo[0], at[1] - self.lo[1], at[2] - self.lo[2]];
        if (0..3).any(|i| l[i] < 0 || l[i] >= self.size[i]) {
            return (self.read)(at);
        }
        let i = ((l[1] * self.size[2] + l[2]) * self.size[0] + l[0]) as usize;
        match self.data[i] {
            u8::MAX => {
                let cell = (self.read)(at);
                self.data[i] = CELLS.iter().position(|c| *c == cell).unwrap_or(0) as u8;
                cell
            }
            v => CELLS[usize::from(v)],
        }
    }
}
/// `search_near`, stepping off drops of up to `max_drop` blocks (beyond `MAX_DROP` only when it
/// saves a long way: each block more costs as much as four steps).
pub fn search_near_drop(
    start: Pos,
    goal: Pos,
    radius: u8,
    max_drop: i32,
    mut cell: impl FnMut(Pos) -> Cell,
    canceled: impl Fn() -> bool,
) -> Option<Path> {
    let reached = |at: Pos| {
        let dx = i64::from(at[0]) - i64::from(goal[0]);
        let dz = i64::from(at[2]) - i64::from(goal[2]);
        dx * dx + dz * dz <= i64::from(radius).pow(2)
            && (at[1] - goal[1]).abs() <= i32::from(radius > 0)
    };
    // How far up or down the search goes: sixteen blocks around a goal on the same level; for a
    // goal farther up or down (a cave far below, a tower) the whole height between the two and
    // more, with no fixed limit (the world's height bounds it).
    let vertical = ((goal[1] - start[1]).abs() + 16).max(16);
    let explored = explore_with(
        start,
        max_drop,
        current_reach(),
        vertical,
        |at| heuristic(at, goal),
        reached,
        &mut cell,
        &canceled,
    )?;
    if canceled() || (explored.best == start && !reached(start)) {
        return None;
    }
    let complete = reached(explored.best);
    let mut path = explored.path_to(explored.best)?;
    path.complete = complete;
    Some(path)
}
/// What a search went through: the cost of the best way found to each cell and the cell it came
/// from, and the cell it ended at (the goal, or the nearest to it it got).
pub struct Explored {
    pub costs: PosMap<i32>,
    parents: PosMap<Pos>,
    pub best: Pos,
    limit: usize,
}
impl Explored {
    /// The way from the start to `at`, when the search got there.
    pub fn path_to(&self, at: Pos) -> Option<Path> {
        self.costs.get(&at)?;
        let mut nodes = vec![at];
        while let Some(parent) = self.parents.get(nodes.last()?) {
            nodes.push(*parent);
            if nodes.len() > self.limit {
                return None;
            }
        }
        nodes.reverse();
        Some(Path {
            nodes,
            complete: true,
        })
    }
}
/// Every place that can be walked to from `start` within the near reach (the cheapest way to
/// each), for a caller that has many places to choose from: one search instead of one for each.
pub fn explore(
    start: Pos,
    mut cell: impl FnMut(Pos) -> Cell,
    canceled: impl Fn() -> bool,
) -> Option<Explored> {
    let reach = Reach {
        radius: RADIUS,
        nodes: 60_000,
        millis: 300,
    };
    explore_with(
        start,
        current_drop_limit(),
        reach,
        // A miner's places to stand lie up to twelve blocks up or down, and five more under a
        // trunk.
        24,
        |_| 0,
        |_| false,
        &mut cell,
        &canceled,
    )
}
/// The bounded A* shared by the searches (`estimate` 0 makes it a plain cheapest-first
/// exploration). None when `start` is no place to start from.
#[allow(clippy::too_many_arguments)]
fn explore_with(
    start: Pos,
    max_drop: i32,
    reach: Reach,
    vertical: i32,
    estimate: impl Fn(Pos) -> i32,
    reached: impl Fn(Pos) -> bool,
    cell: &mut impl FnMut(Pos) -> Cell,
    canceled: &impl Fn() -> bool,
) -> Option<Explored> {
    // In the water the walk starts from where the body swims (it climbs out on the first step).
    if !standable(start, cell) && cell(start) != Cell::Water {
        return None;
    }
    // Every cell read once (moves reach four blocks out, drops twelve down, a body two up).
    let margin = 6;
    let below = vertical.max(max_drop.min(FAR_DROP)) + margin;
    let mut cache = CellCache::new(
        [
            start[0] - reach.radius - margin,
            start[1] - below,
            start[2] - reach.radius - margin,
        ],
        [
            start[0] + reach.radius + margin,
            start[1] + vertical + margin,
            start[2] + reach.radius + margin,
        ],
        cell,
    );
    let mut cell = |at: Pos| cache.get(at);
    // Long drops cost as much as a long way round; glided ones (elytra) much less.
    let drop_cost = if max_drop > FAR_DROP {
        6
    } else {
        FAR_DROP_COST
    };
    let began = Instant::now();
    // Ties go to the deeper cell (the larger cost so far): straight at the goal, fewer cells.
    let mut open = BinaryHeap::from([Reverse((estimate(start), 0, start))]);
    let mut costs = PosMap::default();
    costs.insert(start, 0);
    let mut parents = PosMap::default();
    let mut best = start;
    let mut expanded = 0;
    while let Some(Reverse((_, depth, at))) = open.pop() {
        let cost = -depth;
        if canceled() {
            return None;
        }
        if cost != costs[&at] {
            continue;
        }
        if estimate(at) < estimate(best) {
            best = at;
        }
        if reached(at) {
            best = at;
            break;
        }
        expanded += 1;
        if expanded >= reach.nodes || began.elapsed() > Duration::from_millis(reach.millis) {
            break;
        }
        let mut push = |next: Pos, next_cost: i32| {
            if costs.get(&next).is_none_or(|old| next_cost < *old) {
                costs.insert(next, next_cost);
                parents.insert(next, at);
                open.push(Reverse((next_cost + estimate(next), -next_cost, next)));
            }
        };
        for ([dx, dy, dz], base) in MOVES {
            let next = [at[0] + dx, at[1] + dy, at[2] + dz];
            if dy < -max_drop
                || (next[0] - start[0]).abs() > reach.radius
                || (next[2] - start[2]).abs() > reach.radius
                || next[1] - start[1] > vertical
                || start[1] - next[1] > vertical.max(max_drop)
                || !traversable(at, next, &mut cell)
            {
                continue;
            }
            // A door costs the click that opens it; a ladder is slower than a floor.
            let door = (0..=1).any(|y| cell([next[0], next[1] + y, next[2]]) == Cell::Door);
            let next_cost = cost
                + base
                + if dy > 0 { 4 } else { dy.abs() * 2 }
                + if door { 10 } else { 0 }
                + (-dy - MAX_DROP).max(0) * drop_cost;
            push(next, next_cost);
        }
        // With elytra: off an edge to wherever the ground is, however far down (glided).
        if max_drop > FAR_DROP {
            for [dx, dz] in [[1, 0], [-1, 0], [0, 1], [0, -1]] {
                let column = [at[0] + dx, at[2] + dz];
                if (column[0] - start[0]).abs() > reach.radius
                    || (column[1] - start[2]).abs() > reach.radius
                {
                    continue;
                }
                let lowest = (at[1] - max_drop).max(start[1] - max_drop);
                let mut y = at[1] - FAR_DROP - 1;
                let landing = loop {
                    if y < lowest {
                        break None;
                    }
                    let here = [column[0], y, column[1]];
                    if standable(here, &mut cell) {
                        break Some(here);
                    }
                    if !self::open(cell(here)) {
                        break None;
                    }
                    y -= 1;
                };
                let Some(next) = landing.filter(|n| traversable(at, *n, &mut cell)) else {
                    continue;
                };
                let fall = at[1] - next[1];
                push(next, cost + 10 + fall * 2 + (fall - MAX_DROP) * drop_cost);
            }
        }
    }
    Some(Explored {
        costs,
        parents,
        best,
        limit: reach.nodes,
    })
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Status {
    #[default]
    Idle,
    Planning,
    Preview,
    Walking,
    Paused,
    Arrived,
    NoPath,
    Stuck,
    Waiting,
    Holding,
    TargetMissing,
    TargetLost,
    Mining,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum FollowDecision {
    Hold(Pos),
    Pursue(Pos),
    Missing,
    Lost,
}

#[derive(Clone, Debug)]
pub struct Follow {
    pub uuid: u128,
    last_seen_ms: u64,
    holding: bool,
}
impl Follow {
    pub fn new(uuid: u128, now_ms: u64) -> Self {
        Self {
            uuid,
            last_seen_ms: now_ms,
            holding: false,
        }
    }
    pub fn update(
        &mut self,
        position: [f64; 3],
        target: Option<[f64; 3]>,
        now_ms: u64,
    ) -> FollowDecision {
        let Some(target) = target.filter(|p| p.iter().all(|v| v.is_finite())) else {
            self.holding = false;
            return if now_ms.saturating_sub(self.last_seen_ms) >= 10_000 {
                FollowDecision::Lost
            } else {
                FollowDecision::Missing
            };
        };
        self.last_seen_ms = now_ms;
        let range = if self.holding { 3.5 } else { 2.5 };
        self.holding = (position[0] - target[0]).hypot(position[2] - target[2]) <= range
            && (position[1] - target[1]).abs() <= 2.25;
        if self.holding {
            FollowDecision::Hold(block(target))
        } else {
            FollowDecision::Pursue(block(target))
        }
    }
}

#[derive(Clone, Debug, Default)]
pub struct Navigator {
    pub precise: bool,
    /// The body is in a ladder or vines (set by the caller each tick): a node above is reached
    /// by climbing (the jump key), one below by letting go.
    pub climbing: bool,
    /// Walk as a person does: the caller turns the head toward `look_point`, and a target
    /// behind is turned to first instead of being walked to backwards or sideways.
    pub human: bool,
    pub follow: Option<Follow>,
    pub status: Status,
    pub goal: Option<Pos>,
    pub path: Vec<Pos>,
    pub next: usize,
    pub complete: bool,
    pub running: bool,
    progress: Option<[f64; 3]>,
    /// The step (index of `path`) whose run-up for a jump is done.
    leap_backed: Option<usize>,
    stalled: u16,
    recoveries: u8,
    recovery_origin: Option<[f64; 3]>,
}
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Input {
    pub forward: f32,
    pub strafe: f32,
    pub jump: bool,
    /// Run (the sprint key): a jump across a gap needs the speed.
    pub sprint: bool,
}
/// Keys toward `target` for any task that moves on its own (tunnel, bridge, drain, swimming),
/// the way the walker moves: forward, with a side step while turning, and a target behind is
/// turned to first (the head turns, see the human look) instead of walked to backwards. Close by
/// (fine positioning on an edge) every direction is allowed.
/// The movement keys (forward, strafe) toward a direction `dx, dz` for a head turned to `yaw`
/// (radians): the nearest of the eight ways.
fn keys_toward(dx: f64, dz: f64, yaw: f64) -> (f32, f32) {
    if dx.hypot(dz) < 1e-6 {
        return (0.0, 0.0);
    }
    let forward = -dx * yaw.sin() + dz * yaw.cos();
    let left = dx * yaw.cos() + dz * yaw.sin();
    let sector = (left.atan2(forward) / (std::f64::consts::PI / 4.0)).round() as i32;
    match sector.rem_euclid(8) {
        0 => (1., 0.),
        1 => (1., 1.),
        2 => (0., 1.),
        3 => (-1., 1.),
        4 => (-1., 0.),
        5 => (-1., -1.),
        6 => (0., -1.),
        _ => (1., -1.),
    }
}
pub fn human_steer(position: [f64; 3], target: [f64; 3], yaw: f32) -> Input {
    let dx = target[0] - position[0];
    let dz = target[2] - position[2];
    let distance = dx.hypot(dz);
    if distance < 0.06 {
        return Input::default();
    }
    let yaw = f64::from(yaw).to_radians();
    let forward = -dx * yaw.sin() + dz * yaw.cos();
    let left = dx * yaw.cos() + dz * yaw.sin();
    let sector = (left.atan2(forward) / (std::f64::consts::PI / 4.0)).round() as i32;
    let sector = sector.rem_euclid(8);
    if distance > 0.4 && (3..=5).contains(&sector) {
        return Input::default();
    }
    let (forward, strafe) = match sector {
        0 => (1., 0.),
        1 => (1., 1.),
        2 => (0., 1.),
        3 => (-1., 1.),
        4 => (-1., 0.),
        5 => (-1., -1.),
        6 => (0., -1.),
        _ => (1., -1.),
    };
    Input {
        forward,
        strafe,
        jump: false,
        sprint: false,
    }
}
/// How a glide on elytra is flown, tick by tick: the yaw and pitch to hold. Toward the landing,
/// turned aside from what is in the way (`clear` says whether a heading is open for a few blocks
/// ahead); high over the landing it circles to lose height instead of diving; on the way it
/// holds a descent that brings it down at the landing; low over it, it levels out so as to touch
/// down softly (a slow descent takes no fall damage).
pub fn glide(position: [f64; 3], landing: [f64; 3], clear: impl Fn(f32) -> bool) -> (f32, f32) {
    let above = position[1] - landing[1];
    let across = (landing[0] - position[0]).hypot(landing[2] - position[2]);
    let toward = yaw_to(position, landing);
    // High and close: circle round the landing.
    let wanted = if across < 4.0 && above > 6.0 {
        toward + 90.0
    } else {
        toward
    };
    let yaw = [
        0.0f32, 30.0, -30.0, 60.0, -60.0, 90.0, -90.0, 135.0, -135.0, 180.0,
    ]
    .into_iter()
    .map(|turn| wanted + turn)
    .find(|y| clear(*y))
    .unwrap_or(wanted);
    let pitch = if above < 5.0 {
        // Level out for the touch-down.
        -5.0
    } else if across < 4.0 {
        20.0
    } else {
        // The slope that ends at the landing, a little steeper than needed, kept gentle.
        ((above / across).atan().to_degrees() as f32 + 5.0).clamp(8.0, 35.0)
    };
    (yaw, pitch)
}
/// The yaw (degrees, the game's convention) that faces from `from` toward `to`.
pub fn yaw_to(from: [f64; 3], to: [f64; 3]) -> f32 {
    (-(to[0] - from[0])).atan2(to[2] - from[2]).to_degrees() as f32
}
/// The pitch (degrees, down positive) that looks from the eye at `eye` toward `to`.
pub fn pitch_to(eye: [f64; 3], to: [f64; 3]) -> f32 {
    let (dx, dy, dz) = (to[0] - eye[0], to[1] - eye[1], to[2] - eye[2]);
    (-dy).atan2(dx.hypot(dz)).to_degrees() as f32
}
/// The signed smallest turn from `from` to `to`, in degrees (-180..180).
pub fn angle_between(from: f32, to: f32) -> f32 {
    (to - from + 540.0).rem_euclid(360.0) - 180.0
}
/// One tick of a head turn as a person makes it: fast while far off, easing in as it comes
/// round (a share of what is left, at least a little, at most `max_step`).
pub fn turn_toward(current: f32, wanted: f32, max_step: f32) -> f32 {
    let left = angle_between(current, wanted);
    let step = (left * 0.4).abs().clamp(left.abs().min(1.5), max_step);
    current + step.copysign(left)
}
pub fn block(position: [f64; 3]) -> Pos {
    position.map(|x| x.floor() as i32)
}
pub fn standing_block(position: [f64; 3], inside: &str) -> Pos {
    let mut at = block(position);
    if matches!(inside, "minecraft:farmland" | "minecraft:dirt_path")
        && position[1] - f64::from(at[1]) > 0.8
    {
        at[1] += 1;
    }
    at
}
pub fn center(at: Pos) -> [f64; 3] {
    [
        f64::from(at[0]) + 0.5,
        f64::from(at[1]),
        f64::from(at[2]) + 0.5,
    ]
}
pub fn distance(a: [f64; 3], b: [f64; 3]) -> f64 {
    a.into_iter()
        .zip(b)
        .map(|(a, b)| (a - b).powi(2))
        .sum::<f64>()
        .sqrt()
}
impl Navigator {
    pub fn begin(&mut self, goal: Pos, running: bool) {
        *self = Self {
            status: Status::Planning,
            goal: Some(goal),
            running,
            ..Self::default()
        };
    }
    pub fn accept(&mut self, path: Option<Path>) {
        self.path.clear();
        self.next = 1;
        // A run-up done belongs to the old route.
        self.leap_backed = None;
        self.progress = None;
        self.stalled = 0;
        if let Some(path) = path {
            self.path = path.nodes;
            self.complete = path.complete;
            self.status = if self.running {
                Status::Walking
            } else {
                Status::Preview
            };
        } else {
            self.running = false;
            self.status = Status::NoPath;
        }
    }
    pub fn pause(&mut self) {
        if self.running {
            self.running = false;
            self.status = Status::Paused;
        }
    }
    pub fn replan(&mut self) {
        self.path.clear();
        self.next = 0;
        self.progress = None;
        self.stalled = 0;
        self.status = Status::Waiting;
    }
    /// Where a person walking this route would look: a few blocks ahead along it (the next
    /// nodes within reach of the eye), so corners are taken in a curve and the head does not
    /// jerk at every cell. `None` when not walking.
    pub fn look_point(&self, position: [f64; 3]) -> Option<[f64; 3]> {
        if !self.running || self.status != Status::Walking {
            return None;
        }
        let mut point = center(*self.path.get(self.next)?);
        for node in self.path.iter().skip(self.next + 1).take(4) {
            let c = center(*node);
            if (c[0] - position[0]).hypot(c[2] - position[2]) > 3.0
                || (c[1] - position[1]).abs() > 1.5
            {
                break;
            }
            point = c;
        }
        Some(point)
    }
    /// Whether the walker is retrying after a failed plan at this spot (the budget of
    /// `blocked`): such retries keep a slower pace, the world may still be loading.
    pub fn recovering(&self) -> bool {
        self.recoveries > 0
    }
    /// How many ticks in a row the walker has made no progress.
    pub fn stalled(&self) -> u16 {
        self.stalled
    }
    /// The walker gave way to something in its path (a player): that is not being stuck.
    pub fn yielded(&mut self) {
        self.stalled = 0;
    }
    /// Bound automatic recovery attempts at one location; moving away resets the budget.
    pub fn blocked(&mut self, position: [f64; 3]) {
        if self
            .recovery_origin
            .is_none_or(|at| distance(at, position) >= 2.0)
        {
            self.recoveries = 0;
            self.recovery_origin = Some(position);
        }
        if self.recoveries >= 3 {
            self.running = false;
            self.status = Status::Stuck;
        } else {
            self.recoveries += 1;
            self.running = true;
            self.replan();
        }
    }
    pub fn input(&mut self, position: [f64; 3], yaw: f32, grounded: bool) -> Input {
        if !self.running || self.status != Status::Walking {
            return Input::default();
        }
        while self.next < self.path.len() && self.reached(position, self.path[self.next]) {
            self.next += 1;
            self.stalled = 0;
        }
        if self.next >= self.path.len() {
            self.status = if self.complete {
                if self.follow.is_some() {
                    Status::Holding
                } else {
                    self.running = false;
                    Status::Arrived
                }
            } else {
                Status::Waiting
            };
            return Input::default();
        }
        let target = center(self.path[self.next]);
        // Off the route is measured from the step under way (a jump across a gap starts four
        // blocks from its landing).
        let from = self
            .next
            .checked_sub(1)
            .and_then(|i| self.path.get(i))
            .map_or(target, |p| center(*p));
        let off = segment_distance(position, from, target);
        if off > 2.6 {
            // Shoved off the route (a player walking into the walker): plan again from where
            // it stands. Only a long way off (a teleport) is a pause.
            // Only a jump far away (a teleport, a portal) is a pause: a shove from a hit, a
            // glide that landed off the line or a fall are planned again from where it is.
            if off > 64.0 {
                self.pause();
            } else {
                self.replan();
            }
            return Input::default();
        }
        if let Some(last) = self.progress {
            if distance(position, last) < 0.04 {
                self.stalled += 1;
            } else {
                self.progress = Some(position);
                self.stalled = 0;
            }
        } else {
            self.progress = Some(position);
        }
        if self.stalled >= 60 {
            self.running = false;
            self.status = Status::Stuck;
            return Input::default();
        }
        let dx = target[0] - position[0];
        let dz = target[2] - position[2];
        if !self.climbing && dx.hypot(dz) < 0.25 && target[1] < position[1] - 0.5 {
            // Straight down (into a ladder shaft, through an opened trapdoor): let go and fall;
            // walking on would step off the hole.
            return Input::default();
        }
        if self.climbing && dx.hypot(dz) < 0.2 {
            // Straight up or down a ladder: climb with the jump key, or let go and slide.
            return Input {
                forward: 0.0,
                strafe: 0.0,
                jump: target[1] > position[1] + 0.1,
                sprint: false,
            };
        }
        let yaw = f64::from(yaw).to_radians();
        let forward = -dx * yaw.sin() + dz * yaw.cos();
        let left = dx * yaw.cos() + dz * yaw.sin();
        let sector = (left.atan2(forward) / (std::f64::consts::PI / 4.0)).round() as i32;
        // Close to the point (the last steps, fine positioning) every direction is taken: turning
        // round there made the walker overshoot and circle round its goal.
        let to_end = self.path.last().map_or(0.0, |end| {
            let e = center(*end);
            (e[0] - position[0]).hypot(e[2] - position[2])
        });
        // The way on is behind: turn round first (the caller turns the head), do not walk
        // backwards. Not for the last bit of a step: at a corner the head already looks round
        // it, along the way ahead, and the cell to finish lies a little behind; waiting for a
        // turn that never comes stood the walker there until it counted as stuck.
        if self.human
            && grounded
            && to_end > 1.5
            && dx.hypot(dz) > 0.9
            && matches!(sector.rem_euclid(8), 3..=5)
        {
            return Input::default();
        }
        let (forward, strafe) = match sector.rem_euclid(8) {
            0 => (1., 0.),
            1 => (1., 1.),
            2 => (0., 1.),
            3 => (-1., 1.),
            4 => (-1., 0.),
            5 => (-1., -1.),
            6 => (0., -1.),
            _ => (1., -1.),
        };
        // No progress for a while (a slab, a stair or an edge the planner took for flat, a body
        // in the way): jump now and then, then step aside, instead of pushing on in the same
        // way until the stall counts as stuck.
        // A jump across a gap: back up to the far side of the block it starts from (a run-up: a
        // three-block gap needs nearly full sprint speed), sprint at the gap and jump at the very
        // edge (the body stands on the block until its middle is 0.8 past the block's middle).
        // The keys follow the head as on any step, so the run goes the right way whatever it
        // faces. Jumping a quarter of a block in, from a standstill, fell short of the far side.
        let span = (target[0] - from[0]).hypot(target[2] - from[2]);
        if span > 1.5 {
            let dir = [(target[0] - from[0]) / span, (target[2] - from[2]) / span];
            let run = (position[0] - from[0]) * dir[0] + (position[2] - from[2]) * dir[1];
            if self.leap_backed != Some(self.next) && grounded && run > -0.25 && span > 2.5 {
                // Back to the rear of the block, facing the gap all the while.
                let back = [from[0] - dir[0] * 0.35, from[2] - dir[1] * 0.35];
                let (forward, strafe) =
                    keys_toward(back[0] - position[0], back[1] - position[2], yaw);
                return Input {
                    forward,
                    strafe,
                    jump: false,
                    sprint: false,
                };
            }
            self.leap_backed = Some(self.next);
            let (forward, strafe) =
                keys_toward(target[0] - position[0], target[2] - position[2], yaw);
            return Input {
                forward,
                strafe,
                jump: grounded && run >= 0.55,
                sprint: forward > 0.0,
            };
        }
        let unstick = grounded && self.stalled >= 8 && self.stalled % 10 == 8;
        let nudge = if self.stalled >= 20 {
            if (self.stalled / 10).is_multiple_of(2) {
                1.0
            } else {
                -1.0
            }
        } else {
            0.0
        };
        Input {
            forward,
            strafe: if strafe == 0.0 { nudge } else { strafe },
            // Up a step, or out of the top of a ladder onto what it leans on.
            jump: unstick
                || ((grounded || self.climbing)
                    && target[1] > position[1] + 0.5
                    && dx.hypot(dz) < 1.25),
            sprint: false,
        }
    }
    /// Whether the walker is at `node`: close across, and standing on its level. A partial
    /// block (a slab, a bed, a trapdoor) lifts the feet up to a block above the cell, which
    /// is still there; a ladder is followed exactly.
    fn reached(&self, position: [f64; 3], node: Pos) -> bool {
        let c = center(node);
        let near = if self.precise { 0.12 } else { 0.26 };
        if self.climbing {
            return distance(position, c) < near;
        }
        (position[0] - c[0]).hypot(position[2] - c[2]) < near
            && (-0.5..=0.95).contains(&(position[1] - c[1]))
    }
}
/// How far `p` is from the segment `a`–`b`.
pub fn segment_distance(p: [f64; 3], a: [f64; 3], b: [f64; 3]) -> f64 {
    let ab = [b[0] - a[0], b[1] - a[1], b[2] - a[2]];
    let ap = [p[0] - a[0], p[1] - a[1], p[2] - a[2]];
    let len = ab.iter().map(|v| v * v).sum::<f64>();
    let t = if len < 1e-9 {
        0.0
    } else {
        (ap.iter().zip(ab).map(|(x, y)| x * y).sum::<f64>() / len).clamp(0.0, 1.0)
    };
    distance(p, [a[0] + ab[0] * t, a[1] + ab[1] * t, a[2] + ab[2] * t])
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn global_hotkeys_require_a_unique_binding_and_unobstructed_gameplay() {
        let bindings = [
            "key.keyboard.f5",
            "key.keyboard.f6",
            "key.keyboard.f7",
            "key.keyboard.f8",
        ];
        for (key, action) in bindings.into_iter().zip(Hotkey::ALL) {
            assert_eq!(hotkey(key, true, true, false, bindings), Some(action));
            assert_eq!(hotkey(key, false, true, false, bindings), None);
            assert_eq!(hotkey(key, true, false, false, bindings), None);
            assert_eq!(hotkey(key, true, true, true, bindings), None);
        }
        assert_eq!(
            hotkey(
                "key.keyboard.f5",
                true,
                true,
                false,
                [bindings[0]; Hotkey::COUNT]
            ),
            None
        );
        assert_eq!(
            hotkey(
                Hotkey::DEFAULT,
                true,
                true,
                false,
                [Hotkey::DEFAULT; Hotkey::COUNT]
            ),
            None
        );
        assert_eq!(hotkey("key.keyboard.w", true, true, false, bindings), None);
        for language in [
            claim_borders_relay_client::translation::Language::English,
            claim_borders_relay_client::translation::Language::Russian,
        ] {
            for action in Hotkey::ALL {
                assert!(!action.label(language).is_empty());
                assert!(action.id().starts_with("navigation_"));
            }
        }
    }
    #[test]
    fn follow_uses_hysteresis_and_stops_when_the_target_disappears() {
        let mut f = Follow::new(42, 0);
        assert_eq!(
            f.update([0., 0., 0.], Some([2.4, 0., 0.]), 50),
            FollowDecision::Hold([2, 0, 0])
        );
        assert_eq!(
            f.update([0., 0., 0.], Some([3.2, 0., 0.]), 100),
            FollowDecision::Hold([3, 0, 0])
        );
        assert_eq!(
            f.update([0., 0., 0.], Some([3.6, 0., 0.]), 150),
            FollowDecision::Pursue([3, 0, 0])
        );
        assert_eq!(f.update([0., 0., 0.], None, 200), FollowDecision::Missing);
        assert_eq!(f.update([0., 0., 0.], None, 10_150), FollowDecision::Lost);
        assert_eq!(
            f.update([0., 0., 0.], Some([8., 0., 0.]), 10_200),
            FollowDecision::Pursue([8, 0, 0])
        );
        assert_eq!(
            f.update([0., 0., 0.], Some([f64::NAN, 0., 0.]), 10_250),
            FollowDecision::Missing
        );
    }
    #[test]
    fn follow_path_stops_near_the_player_and_manual_pause_stays_paused() {
        let path = search_near([0, 0, 0], [4, 0, 0], 2, flat, || false).unwrap();
        assert!(path.complete);
        assert_eq!(path.nodes.last(), Some(&[2, 0, 0]));
        let mut n = Navigator::default();
        n.begin([4, 0, 0], true);
        n.follow = Some(Follow::new(42, 0));
        n.accept(Some(path));
        n.next = n.path.len();
        assert_eq!(n.input([2.5, 0., 0.5], 0., true), Input::default());
        assert_eq!(n.status, Status::Holding);
        assert!(n.running);
        n.pause();
        assert_eq!(n.input([2.5, 0., 0.5], 0., true), Input::default());
        assert_eq!(n.status, Status::Paused);
        assert_eq!(n.follow.as_ref().unwrap().uuid, 42);
    }
    #[test]
    fn missing_route_retries_with_a_bounded_budget() {
        let mut n = Navigator::default();
        n.begin([12, 0, 0], true);
        for _ in 0..3 {
            n.blocked([0.5, 0., 0.5]);
            assert!(n.running);
            assert_eq!(n.status, Status::Waiting);
            assert!(n.path.is_empty());
        }
        n.blocked([0.5, 0., 0.5]);
        assert!(!n.running);
        assert_eq!(n.status, Status::Stuck);
        n.begin([12, 0, 0], false);
        n.accept(None);
        assert!(!n.running);
        assert_eq!(n.status, Status::NoPath);
    }
    #[test]
    fn changed_terrain_reroutes_and_recovery_has_a_budget() {
        let wall = |at: Pos| {
            if at[0] == 1 && at[2] == 0 && (0..=2).contains(&at[1]) {
                Cell::Solid
            } else {
                flat(at)
            }
        };
        let path = search([0, 0, 0], [4, 0, 0], wall, || false).unwrap();
        assert!(path.complete);
        assert!(path.nodes.iter().any(|at| at[2] != 0));
        let mut n = Navigator::default();
        n.begin([4, 0, 0], true);
        for _ in 0..3 {
            n.blocked([0.5, 0., 0.5]);
            assert!(n.running);
            assert_eq!(n.status, Status::Waiting);
        }
        n.blocked([0.5, 0., 0.5]);
        assert!(!n.running);
        assert_eq!(n.status, Status::Stuck);
        n.blocked([2.5, 0., 0.5]);
        assert!(n.running);
        assert_eq!(n.status, Status::Waiting);
    }
    fn flat(p: Pos) -> Cell {
        if p[1] < 0 { Cell::Solid } else { Cell::Air }
    }
    #[test]
    fn walks_around_a_two_block_wall() {
        let cell = |p: Pos| {
            if p[0] == 2 && p[2] == 0 && (0..=2).contains(&p[1]) {
                Cell::Solid
            } else {
                flat(p)
            }
        };
        let path = search([0, 0, 0], [4, 0, 0], cell, || false).unwrap();
        assert!(path.complete);
        assert!(path.nodes.iter().any(|p| p[2] != 0));
        for pair in path.nodes.windows(2) {
            assert!(traversable(pair[0], pair[1], &mut { cell }));
        }
    }
    #[test]
    fn climbs_one_block_but_needs_headroom() {
        let terrain = |p: Pos| {
            if p[0] >= 1 && p[1] == 0 {
                Cell::Solid
            } else {
                flat(p)
            }
        };
        assert!(
            search([0, 0, 0], [2, 1, 0], terrain, || false)
                .unwrap()
                .complete
        );
        assert!(!traversable(
            [0, 0, 0],
            [1, 1, 0],
            &mut |p| if p == [0, 2, 0] {
                Cell::Solid
            } else {
                terrain(p)
            }
        ));
    }
    #[test]
    fn planning_starts_from_the_nearest_place_to_stand() {
        let mut ground = |p: Pos| if p[1] < 0 { Cell::Solid } else { Cell::Air };
        assert_eq!(nearest_standable([0, 0, 0], &mut ground), Some([0, 0, 0]));
        // In the air after a jump: the landing below.
        assert_eq!(nearest_standable([0, 3, 0], &mut ground), Some([0, 0, 0]));
        assert_eq!(nearest_standable([0, 2, 0], &mut ground), Some([0, 0, 0]));
        // Higher than the drop limit there is nothing to plan from.
        assert_eq!(nearest_standable([0, 4, 0], &mut ground), None);
        // On the edge with the middle over a gap: the neighbouring cell that has a floor.
        let mut ledge = |p: Pos| {
            if p[1] < 0 && p[0] >= 1 {
                Cell::Solid
            } else {
                Cell::Air
            }
        };
        assert!(!standable([0, 0, 0], &mut ledge));
        assert_eq!(nearest_standable([0, 0, 0], &mut ledge), Some([1, 0, 0]));
        // Nothing firm anywhere around.
        assert_eq!(nearest_standable([0, 0, 0], &mut |_| Cell::Air), None);
    }
    #[test]
    fn a_shove_off_the_route_replans_and_a_teleport_pauses() {
        let path = search([0, 0, 0], [6, 0, 0], flat, || false).unwrap();
        let mut n = Navigator::default();
        n.begin([6, 0, 0], true);
        n.accept(Some(path));
        // Pushed four blocks off the path by a passer-by: plan again, do not stop.
        let _ = n.input([2.5, 0., 5.5], 0., true);
        assert_eq!(n.status, Status::Waiting);
        assert!(n.running);
        // A long way off (a teleport) is a pause.
        let path = search([0, 0, 0], [6, 0, 0], flat, || false).unwrap();
        n.accept(Some(path));
        let _ = n.input([2.5, 0., 40.5], 0., true);
        // Forty blocks off (a long glide, a hard shove) is planned again, not paused.
        assert_eq!(n.status, Status::Waiting);
        let path = search([0, 0, 0], [6, 0, 0], flat, || false).unwrap();
        n.begin([6, 0, 0], true);
        n.accept(Some(path));
        let _ = n.input([2.5, 0., 200.5], 0., true);
        assert_eq!(n.status, Status::Paused);
        // Giving way to someone in the path clears the stall: it is not being stuck.
        let path = search([0, 0, 0], [6, 0, 0], flat, || false).unwrap();
        n.begin([6, 0, 0], true);
        n.accept(Some(path));
        for _ in 0..5 {
            let _ = n.input([0.5, 0., 0.5], 0., true);
        }
        assert!(n.stalled() >= 4);
        n.yielded();
        assert_eq!(n.stalled(), 0);
    }
    #[test]
    fn a_leg_tries_the_straight_line_then_turns_aside_then_goes_shorter() {
        let (from, goal) = ([0.0, 64.0, 0.0], [800.0, 64.0, 0.0]);
        let straight = leg_target(from, goal, 0).unwrap();
        assert!((straight[0] - 140.0).abs() < 1e-9 && straight[2].abs() < 1e-9);
        // Turned 35 degrees to either side, at the same reach.
        let left = leg_target(from, goal, 1).unwrap();
        let right = leg_target(from, goal, 2).unwrap();
        assert!(left[2] > 1.0 && right[2] < -1.0);
        assert!((nav_len(left) - 140.0).abs() < 1e-9 && (nav_len(right) - 140.0).abs() < 1e-9);
        // The shorter reach comes after all the turns at the long one.
        let shorter = leg_target(from, goal, 7).unwrap();
        assert!((shorter[0] - 60.0).abs() < 1e-9);
        assert!(leg_target(from, goal, LEG_TRIES).is_none());
        // Standing on the goal there is no direction.
        assert!(leg_target(goal, goal, 0).is_none());
    }
    fn nav_len(p: [f64; 3]) -> f64 {
        p[0].hypot(p[2])
    }
    #[test]
    fn a_walker_that_makes_no_progress_jumps_then_steps_aside() {
        let path = search([0, 0, 0], [6, 0, 0], flat, || false).unwrap();
        let mut n = Navigator::default();
        n.begin([6, 0, 0], true);
        n.accept(Some(path));
        let mut jumped = false;
        let mut stepped_aside = false;
        for _ in 0..40 {
            let input = n.input([0.5, 0., 0.5], 0., true);
            jumped |= input.jump;
            stepped_aside |= input.strafe != 0.0 && n.stalled() >= 20;
        }
        assert!(jumped, "no jump after standing still");
        assert!(stepped_aside, "no step aside after standing still longer");
        // Not on the ground: no jump to give.
        let mut m = Navigator::default();
        m.begin([6, 0, 0], true);
        m.accept(Some(search([0, 0, 0], [6, 0, 0], flat, || false).unwrap()));
        assert!((0..40).all(|_| !m.input([0.5, 0., 0.5], 0., false).jump));
    }
    #[test]
    fn refuses_unknown_terrain_hazards_and_large_drops() {
        assert!(!standable([0, 0, 0], &mut |_| Cell::Unknown));
        assert!(!standable([0, 0, 0], &mut |p| if p == [1, 0, 0] {
            Cell::Hazard
        } else {
            flat(p)
        }));
        assert!(!traversable([0, 0, 0], [1, -2, 0], &mut flat));
        assert!(!traversable(
            [0, 0, 0],
            [1, -13, 0],
            &mut |p| if p[1] < -13 { Cell::Solid } else { Cell::Air }
        ));
        assert!(!traversable([0, 0, 0], [2, 0, 0], &mut flat));
        assert_eq!(classify("minecraft:magma_block", true, false), Cell::Hazard);
        assert_eq!(classify("minecraft:oak_slab", false, false), Cell::Blocked);
        // A fern is walked through, not an obstacle (the bridge used to stop in it).
        for plant in [
            "fern",
            "large_fern",
            "short_grass",
            "tall_grass",
            "poppy",
            "wall_torch",
        ] {
            assert_eq!(
                classify(&format!("minecraft:{plant}"), false, false),
                Cell::Air,
                "{plant}"
            );
        }
        assert_eq!(classify("minecraft:grass_block", true, false), Cell::Solid);
        // Half blocks are walked over, not an obstacle.
        let bottom = [[0.0, 0.0, 0.0, 1.0, 0.5, 1.0]];
        let top = [[0.0, 0.5, 0.0, 1.0, 1.0, 1.0]];
        let carpet = [[0.0, 0.0, 0.0, 1.0, 0.0625, 1.0]];
        let stair = [
            [0.0, 0.0, 0.0, 1.0, 0.5, 1.0],
            [0.0, 0.5, 0.0, 0.5, 1.0, 1.0],
        ];
        let fence = [[0.375, 0.0, 0.375, 0.625, 1.5, 0.625]];
        let shaped = |name: &str, outline: &[[f64; 6]], low| {
            classify_shape(&format!("minecraft:{name}"), false, false, outline, low)
        };
        assert_eq!(shaped("oak_slab", &bottom, Cell::Air), Cell::Air);
        assert_eq!(shaped("oak_slab", &bottom, Cell::Solid), Cell::Solid);
        assert_eq!(shaped("oak_slab", &top, Cell::Air), Cell::Solid);
        assert_eq!(shaped("white_carpet", &carpet, Cell::Air), Cell::Air);
        assert_eq!(shaped("oak_stairs", &stair, Cell::Air), Cell::Solid);
        assert_eq!(shaped("oak_fence", &fence, Cell::Air), Cell::Blocked);
        assert!(!low_shape(&[]));
        assert_eq!(
            classify("minecraft:oak_stairs", false, false),
            Cell::Blocked
        );
    }
    #[test]
    fn gaps_are_jumped_with_a_run_and_partial_blocks_count_as_reached() {
        // A three-block gap (x = 1..=3) in the ground; the landing at x = 4.
        let gap = |p: Pos| {
            if (p[1] < 0 && !(1..=3).contains(&p[0])) || p[1] < -20 {
                Cell::Solid
            } else {
                Cell::Air
            }
        };
        assert!(traversable([0, 0, 0], [4, 0, 0], &mut { gap }));
        let path = search([0, 0, 0], [6, 0, 0], gap, || false).unwrap();
        assert!(path.complete);
        assert!(
            path.nodes
                .windows(2)
                .any(|w| (w[1][0] - w[0][0]).abs() == 4)
        );
        // Not over a gap of five, not without room for the arc, and not when jumps are off.
        assert!(!traversable([0, 0, 0], [5, 0, 0], &mut { gap }));
        let low = |p: Pos| if p == [2, 2, 0] { Cell::Solid } else { gap(p) };
        assert!(!traversable([0, 0, 0], [4, 0, 0], &mut { low }));
        set_long_jumps(false);
        assert!(!traversable([0, 0, 0], [4, 0, 0], &mut { gap }));
        set_long_jumps(true);
        // The walker runs and jumps on such a step.
        let mut n = Navigator::default();
        n.begin([4, 0, 0], true);
        n.accept(Some(Path {
            nodes: vec![[0, 0, 0], [4, 0, 0]],
            complete: true,
        }));
        // At the middle of the block it starts from: back up first, facing the gap (yaw -90 is
        // east, +x), for a run-up.
        let back = n.input([0.5, 0.0, 0.5], -90.0, true);
        assert!(back.forward < 0.0 && !back.jump && !back.sprint, "{back:?}");
        // At the rear of the block: the run-up begins, sprinting at the gap.
        let run = n.input([0.2, 0.0, 0.5], -90.0, true);
        assert!(run.forward > 0.0 && run.sprint && !run.jump, "{run:?}");
        // Running past the middle: still no jump (it fell short when it jumped a quarter in).
        let mid = n.input([0.8, 0.0, 0.5], -90.0, true);
        assert!(mid.forward > 0.0 && mid.sprint && !mid.jump, "{mid:?}");
        // At the edge: jump.
        let edge = n.input([1.1, 0.0, 0.5], -90.0, true);
        assert!(edge.forward > 0.0 && edge.sprint && edge.jump, "{edge:?}");
        assert_eq!(n.status, Status::Walking);
        // Facing the other way (no human look): the keys still run toward the gap.
        let mut w = Navigator::default();
        w.begin([4, 0, 0], true);
        w.accept(Some(Path {
            nodes: vec![[0, 0, 0], [4, 0, 0]],
            complete: true,
        }));
        let _ = w.input([0.1, 0.0, 0.5], 90.0, true);
        let away = w.input([1.1, 0.0, 0.5], 90.0, true);
        assert!(away.forward < 0.0 && away.jump && !away.sprint, "{away:?}");
        // Standing on a slab (half a block up) is standing on its cell.
        let mut m = Navigator::default();
        m.begin([1, 0, 0], true);
        m.accept(Some(Path {
            nodes: vec![[0, 0, 0], [1, 0, 0]],
            complete: true,
        }));
        let _ = m.input([1.5, 0.5, 0.5], 0.0, true);
        assert_eq!(m.status, Status::Arrived);
    }
    #[test]
    fn corners_are_cut_only_when_both_sides_are_open() {
        // On open ground a straight diagonal is the route, and shorter than a staircase.
        assert!(traversable([0, 0, 0], [1, 0, 1], &mut flat));
        let path = search([0, 0, 0], [5, 0, 5], flat, || false).unwrap();
        assert!(path.complete);
        assert_eq!(path.nodes.len(), 6);
        // A block on one side of the corner: no cut (the body would catch on it).
        let post = |p: Pos| if p == [1, 1, 0] { Cell::Solid } else { flat(p) };
        assert!(!traversable([0, 0, 0], [1, 0, 1], &mut { post }));
        // Corners are never cut upward (that takes a jump).
        let step = |p: Pos| if p == [1, 0, 1] { Cell::Solid } else { flat(p) };
        assert!(!traversable([0, 0, 0], [1, 1, 1], &mut { step }));
    }
    #[test]
    fn ladders_are_climbed_and_doors_and_gates_are_passed() {
        // A ladder up the side of a wall four blocks high at x = 1.
        let world = |p: Pos| {
            if p[1] < 0 || (p[0] == 1 && p[1] <= 3) {
                Cell::Solid
            } else if p[0] == 0 && p[2] == 0 && (0..=3).contains(&p[1]) {
                Cell::Climb
            } else {
                Cell::Air
            }
        };
        assert!(standable([0, 2, 0], &mut { world }));
        assert!(traversable([0, 2, 0], [0, 3, 0], &mut { world }));
        assert!(traversable([0, 3, 0], [0, 2, 0], &mut { world }));
        // Not up through the open air beside the ladder.
        assert!(!traversable([0, 0, 1], [0, 1, 1], &mut { world }));
        let path = search([0, 0, 0], [1, 4, 0], world, || false).unwrap();
        assert!(path.complete, "{:?}", path.nodes);
        assert!(path.nodes.contains(&[0, 3, 0]));
        // A wall with a door in it: the door is the way through.
        let wall = |p: Pos| {
            if p[1] < 0 {
                Cell::Solid
            } else if p[0] == 2 && (-6..=6).contains(&p[2]) && p[1] <= 3 {
                if p[2] == 3 && p[1] <= 1 {
                    Cell::Door
                } else {
                    Cell::Solid
                }
            } else {
                Cell::Air
            }
        };
        let path = search([0, 0, 0], [4, 0, 0], wall, || false).unwrap();
        assert!(path.complete);
        assert!(path.nodes.contains(&[2, 0, 3]));
        // Never up or down a step in a doorway.
        assert!(!traversable(
            [1, 0, 3],
            [2, 1, 3],
            &mut |p| if p == [2, 0, 3] { Cell::Solid } else { wall(p) }
        ));
        assert_eq!(classify("minecraft:oak_door", false, false), Cell::Door);
        assert_eq!(
            classify("minecraft:birch_fence_gate", false, false),
            Cell::Door
        );
        assert_eq!(classify("minecraft:iron_door", false, false), Cell::Blocked);
        assert_eq!(classify("minecraft:ladder", false, false), Cell::Climb);
        assert_eq!(classify("minecraft:vine", false, false), Cell::Climb);
    }
    #[test]
    fn a_long_drop_is_taken_only_when_allowed_and_there_is_no_other_way() {
        // A roof six blocks over the ground, ending in a sheer edge at x = 2.
        let roof = |p: Pos| {
            if p[1] < 0 || (p[0] <= 2 && p[1] == 5) {
                Cell::Solid
            } else {
                Cell::Air
            }
        };
        let down = |max| {
            search_near_drop([0, 6, 0], [6, 0, 0], 0, max, roof, || false)
                .is_some_and(|p| p.complete)
        };
        assert!(!down(MAX_DROP));
        assert!(down(FAR_DROP));
        // Stairs three blocks aside: the shorter way down (the drop, within the safe height) is
        // taken rather than the way round.
        let aside = |p: Pos| {
            if p[2] == 3 && p[0] >= 3 && p[1] < 6 - (p[0] - 2) {
                Cell::Solid
            } else {
                roof(p)
            }
        };
        let path = search_near_drop([0, 6, 0], [6, 0, 0], 0, FAR_DROP, aside, || false).unwrap();
        assert!(path.complete);
        assert!(path.nodes.windows(2).any(|w| w[1][1] - w[0][1] < -MAX_DROP));
        // Not the long way down the whole stair: a short route with a drop on it.
        assert!(path.nodes.len() <= 8, "{:?}", path.nodes);
        // Stairs right on the way down: as short, and no fall, so the stairs are taken.
        let on_the_way = |p: Pos| {
            if p[2] == 0 && p[0] >= 3 && p[1] < 6 - (p[0] - 2) {
                Cell::Solid
            } else {
                roof(p)
            }
        };
        let path =
            search_near_drop([0, 6, 0], [9, 0, 0], 0, FAR_DROP, on_the_way, || false).unwrap();
        assert!(path.complete);
        assert!(
            path.nodes
                .windows(2)
                .all(|w| w[1][1] - w[0][1] >= -MAX_DROP),
            "{:?}",
            path.nodes
        );
    }
    #[test]
    fn checklist_lava_is_walked_round_never_beside() {
        // A lava pool across the way (x 3..=5, z -2..=2); the floor goes on all round it.
        let lava = |p: Pos| (3..=5).contains(&p[0]) && (-2..=2).contains(&p[2]);
        let cell = |p: Pos| {
            if p[1] == 0 && lava(p) {
                Cell::Hazard
            } else {
                flat(p)
            }
        };
        let path = search([0, 0, 0], [8, 0, 0], cell, || false).unwrap();
        assert!(path.complete);
        for n in &path.nodes {
            for dx in -1..=1 {
                for dz in -1..=1 {
                    assert!(!lava([n[0] + dx, 0, n[2] + dz]), "{n:?} stands by the lava");
                }
            }
        }
    }
    #[test]
    fn checklist_a_three_block_drop_is_taken_straight_off() {
        // A ledge three blocks over the ground; stairs down twenty blocks aside.
        let cell = |p: Pos| {
            let ledge = p[0] <= 2 && p[1] < 3;
            let stairs = p[2] == 20 && p[0] >= 3 && p[1] < 3 - (p[0] - 2);
            if p[1] < 0 || ledge || stairs {
                Cell::Solid
            } else {
                Cell::Air
            }
        };
        let path = search_near_drop([0, 3, 0], [6, 0, 0], 0, MAX_DROP, cell, || false).unwrap();
        assert!(path.complete);
        assert!(
            path.nodes.iter().all(|n| n[2].abs() <= 2),
            "{:?}",
            path.nodes
        );
        assert!(path.nodes.windows(2).any(|w| w[1][1] - w[0][1] == -3));
    }
    #[test]
    fn checklist_far_levels_up_and_down_are_found() {
        set_reach(WIDE);
        // A long stair up 32 blocks (one block per step) to a tower top, and back down.
        let stair = |p: Pos| {
            let top = p[0].clamp(0, 32);
            if p[1] < top && p[2].abs() <= 1 || p[1] < 0 {
                Cell::Solid
            } else {
                Cell::Air
            }
        };
        let up = search([-2, 0, 0], [36, 32, 0], stair, || false).unwrap();
        assert!(up.complete, "up: {:?}", up.nodes.last());
        let down = search([36, 32, 0], [-2, 0, 0], stair, || false).unwrap();
        assert!(down.complete, "down: {:?}", down.nodes.last());
        set_reach(NEAR);
    }
    #[test]
    fn checklist_a_far_goal_round_a_long_wall_is_found_in_one_search() {
        set_reach(WIDE);
        // A wall three high from z -60 to 60 between the walker and a goal 120 blocks away.
        let cell = |p: Pos| {
            if p[0] == 60 && (-60..=60).contains(&p[2]) && (0..3).contains(&p[1]) {
                Cell::Solid
            } else {
                flat(p)
            }
        };
        let start = Instant::now();
        let path = search([0, 0, 0], [120, 0, 0], cell, || false).unwrap();
        set_reach(NEAR);
        assert!(path.complete, "{:?}", path.nodes.last());
        assert!(start.elapsed() < Duration::from_secs(20));
    }
    #[test]
    fn checklist_iron_doors_are_not_opened() {
        assert!(!openable("minecraft:iron_door"));
        assert!(!openable("minecraft:iron_trapdoor"));
        assert!(openable("minecraft:oak_door"));
        assert!(openable("minecraft:spruce_fence_gate"));
        assert!(openable("minecraft:birch_trapdoor"));
        assert_ne!(classify("minecraft:iron_door", false, false), Cell::Door);
    }
    /// Walks a route tick by tick: the keys move the body as the game does (no momentum), and
    /// with `human` the head turns toward the look point as the human look turns it; without it
    /// the head stays where it is. Returns the ticks taken to arrive (None: never) and the
    /// ticks spent within a block of the end before arriving.
    fn simulate(
        world: impl Fn(Pos) -> Cell + Copy,
        start: Pos,
        goal: Pos,
        yaw0: f32,
        human: bool,
    ) -> (Option<u32>, u32) {
        let path = search(start, goal, world, || false).unwrap();
        assert!(path.complete);
        let mut n = Navigator::default();
        n.begin(goal, true);
        n.human = human;
        n.accept(Some(path));
        let mut position = center(start);
        let mut yaw = yaw0;
        let mut near = 0;
        for tick in 0..2000 {
            if human && let Some(look) = n.look_point(position) {
                let flat = (look[0] - position[0]).hypot(look[2] - position[2]);
                if flat > 0.3 {
                    yaw = turn_toward(yaw, yaw_to(position, look), 22.0);
                }
            }
            let input = n.input(position, yaw, true);
            if n.status == Status::Arrived {
                return (Some(tick), near);
            }
            if !n.running {
                return (None, near);
            }
            let r = f64::from(yaw).to_radians();
            let (f, l) = (f64::from(input.forward), f64::from(input.strafe));
            let length = f.hypot(l).max(1.0);
            let speed = 0.216 / length;
            let dx = (f * -r.sin() + l * r.cos()) * speed;
            let dz = (f * r.cos() + l * r.sin()) * speed;
            let next = [position[0] + dx, position[1], position[2] + dz];
            // Walls stop the body (each axis on its own, as the game slides along them).
            if open(world(block([next[0], position[1], position[2]]))) {
                position[0] = next[0];
            }
            if open(world(block([position[0], position[1], next[2]]))) {
                position[2] = next[2];
            }
            let end = center(goal);
            if (end[0] - position[0]).hypot(end[2] - position[2]) < 1.0 {
                near += 1;
            }
        }
        (None, near)
    }
    #[test]
    fn checklist_walking_arrives_without_circling_with_and_without_the_human_look() {
        // A wall between the walker and a goal just behind it: the way goes round its end.
        let wall = |p: Pos| {
            if p[2] == 2 && (-10..=3).contains(&p[0]) && (0..3).contains(&p[1]) {
                Cell::Solid
            } else {
                flat(p)
            }
        };
        let open_field = flat;
        for human in [true, false] {
            for yaw0 in [0.0, 90.0, 180.0, -90.0] {
                for (name, start, goal) in [
                    ("behind", [0, 0, 0], [0, 0, -6]),
                    ("diagonal", [0, 0, 0], [7, 0, 5]),
                ] {
                    let (ticks, near) = simulate(open_field, start, goal, yaw0, human);
                    assert!(
                        ticks.is_some(),
                        "{name} human={human} yaw={yaw0}: never arrived"
                    );
                    assert!(
                        near < 30,
                        "{name} human={human} yaw={yaw0}: circled {near} ticks"
                    );
                }
                let (ticks, near) = simulate(wall, [0, 0, 0], [0, 0, 4], yaw0, human);
                assert!(
                    ticks.is_some(),
                    "wall human={human} yaw={yaw0}: never arrived"
                );
                assert!(
                    near < 30,
                    "wall human={human} yaw={yaw0}: circled {near} ticks"
                );
            }
        }
    }
    #[test]
    fn checklist_a_narrow_zigzag_corridor_is_walked_with_and_without_the_human_look() {
        // A one-wide corridor: east 6, north 4, west 6, north 4, east 6 (walls everywhere else).
        let corridor: Vec<Pos> = {
            let mut c = Vec::new();
            let mut at = [0, 0, 0];
            c.push(at);
            for (dx, dz, n) in [(1, 0, 6), (0, 1, 4), (-1, 0, 6), (0, 1, 4), (1, 0, 6)] {
                for _ in 0..n {
                    at = [at[0] + dx, 0, at[2] + dz];
                    c.push(at);
                }
            }
            c
        };
        let goal = *corridor.last().unwrap();
        let world = move |p: Pos| {
            if p[1] < 0 {
                Cell::Solid
            } else if p[1] <= 1 && corridor_has(p) {
                Cell::Air
            } else if p[1] <= 2 {
                Cell::Solid
            } else {
                Cell::Air
            }
        };
        fn corridor_has(p: Pos) -> bool {
            let (x, z) = (p[0], p[2]);
            (z == 0 && (0..=6).contains(&x))
                || (x == 6 && (0..=4).contains(&z))
                || (z == 4 && (0..=6).contains(&x))
                || (x == 0 && (4..=8).contains(&z))
                || (z == 8 && (0..=6).contains(&x))
        }
        for human in [true, false] {
            for yaw0 in [0.0, 180.0, -90.0] {
                let (ticks, near) = simulate(world, [0, 0, 0], goal, yaw0, human);
                assert!(
                    ticks.is_some(),
                    "zigzag human={human} yaw={yaw0}: never arrived"
                );
                assert!(near < 30, "zigzag human={human} yaw={yaw0}: circled {near}");
            }
        }
    }
    #[test]
    fn the_cell_cache_answers_as_the_world_does_inside_and_outside_its_box() {
        let world = |p: Pos| match (p[0] + 3 * p[1] + 7 * p[2]).rem_euclid(9) {
            0 => Cell::Solid,
            1 => Cell::Water,
            2 => Cell::Hazard,
            3 => Cell::Door,
            4 => Cell::Climb,
            5 => Cell::Blocked,
            6 => Cell::Unknown,
            _ => Cell::Air,
        };
        let mut reads = 0;
        let mut cache = CellCache::new([-4, -4, -4], [4, 4, 4], |p| {
            reads += 1;
            world(p)
        });
        for _ in 0..2 {
            for x in -6..=6 {
                for y in -6..=6 {
                    for z in -6..=6 {
                        assert_eq!(cache.get([x, y, z]), world([x, y, z]));
                    }
                }
            }
        }
        // Inside the box each cell was read once; outside, every time.
        let inside = 9 * 9 * 9;
        let outside = 13 * 13 * 13 - inside;
        drop(cache);
        assert_eq!(reads, inside + 2 * outside);
    }
    #[test]
    fn exploring_reaches_exactly_the_places_a_search_reaches() {
        // Uneven ground with walls: the cost of the way to each place agrees.
        let world = |p: Pos| {
            let ground = ((p[0] * 7 + p[2] * 3).rem_euclid(5) == 0) as i32;
            let wall = p[0] == 4 && (-6..=5).contains(&p[2]) && p[1] <= 2;
            if p[1] < ground || wall {
                Cell::Solid
            } else {
                Cell::Air
            }
        };
        let start = [0, 0, 0];
        let start = if standable(start, &mut { world }) {
            start
        } else {
            [0, 1, 0]
        };
        let explored = explore(start, world, || false).unwrap();
        let mut compared = 0;
        for x in -10..=10 {
            for z in -10..=10 {
                for y in -1..=2 {
                    let goal = [x, y, z];
                    if !standable(goal, &mut { world }) {
                        continue;
                    }
                    let searched = search(start, goal, world, || false).is_some_and(|p| p.complete);
                    let found = explored.path_to(goal);
                    assert_eq!(found.is_some(), searched, "{goal:?}");
                    if let Some(found) = found {
                        assert_eq!(found.nodes.first(), Some(&start));
                        assert_eq!(found.nodes.last(), Some(&goal));
                        for w in found.nodes.windows(2) {
                            assert!(traversable(w[0], w[1], &mut { world }), "{w:?}");
                        }
                        compared += 1;
                    }
                }
            }
        }
        assert!(compared >= 200, "{compared}");
    }
    #[test]
    fn a_human_walker_looks_ahead_turns_smoothly_and_never_walks_backwards() {
        // Facing south (yaw 0) is +z; east (+x) is yaw -90.
        assert!((yaw_to([0.0; 3], [0.0, 0.0, 5.0])).abs() < 1e-4);
        assert!((yaw_to([0.0; 3], [5.0, 0.0, 0.0]) + 90.0).abs() < 1e-4);
        assert!((angle_between(170.0, -170.0) - 20.0).abs() < 1e-4);
        // A turn eases in: big steps first, never past the target, at most the limit.
        let mut yaw = 0.0;
        let mut steps = Vec::new();
        for _ in 0..30 {
            let next = turn_toward(yaw, 120.0, 25.0);
            steps.push(next - yaw);
            yaw = next;
        }
        assert!((yaw - 120.0).abs() < 1e-3);
        assert!(steps[0] <= 25.0 + 1e-4 && steps[0] > steps[8]);
        // The look point is a few blocks ahead on a straight route.
        let path = search([0, 0, 0], [10, 0, 0], flat, || false).unwrap();
        let mut n = Navigator::default();
        n.begin([10, 0, 0], true);
        n.human = true;
        n.accept(Some(path));
        let look = n.look_point([0.5, 0.0, 0.5]).unwrap();
        assert!(look[0] >= 2.5);
        // Facing away from the route (yaw 90 looks toward -x): stand and turn, no backwards.
        assert_eq!(n.input([0.5, 0.0, 0.5], 90.0, true), Input::default());
        // Facing along it: forward.
        assert_eq!(n.input([0.5, 0.0, 0.5], -90.0, true).forward, 1.0);
        // Within a step and a half of the end: no turning round, it steps back to its goal.
        let mut m = Navigator::default();
        m.begin([1, 0, 0], true);
        m.human = true;
        m.accept(Some(Path {
            nodes: vec![[0, 0, 0], [1, 0, 0]],
            complete: true,
        }));
        assert_eq!(m.input([2.2, 0.0, 0.5], -90.0, true).forward, -1.0);
    }
    #[test]
    fn a_second_floor_reaches_the_ground_by_a_far_slope_with_the_wide_search() {
        // A floor ten blocks up over the whole area; its only way down is a ramp at x = 40..50.
        let world = |p: Pos| {
            let ramp = (40..=50).contains(&p[0]) && p[2] == 0 && p[1] < 10 - (p[0] - 40);
            if p[1] < 0 || (p[1] == 9 && p[0] <= 40) || ramp {
                Cell::Solid
            } else {
                Cell::Air
            }
        };
        let (start, goal) = ([0, 10, 0], [0, 0, 3]);
        assert!(wide_needed(start, goal));
        set_reach(NEAR);
        assert!(search(start, goal, world, || false).is_none_or(|p| !p.complete));
        set_reach(WIDE);
        let path = search(start, goal, world, || false).unwrap();
        set_reach(NEAR);
        assert!(path.complete);
        assert!(path.nodes.iter().any(|p| p[0] >= 40));
    }
    #[test]
    fn remembered_routes_are_followed_both_ways_and_ping_pong_is_seen() {
        let route: Vec<Pos> = (0..=40).map(|x| [x, 64 - x / 10, 0]).collect();
        // From near the start to the far end: every tenth cell, the end last.
        let via = route_via(&route, [1, 64, 1], [40, 60, 0], 10).unwrap();
        assert_eq!(via.last(), Some(&[40, 60, 0]));
        assert_eq!(via[0], [11, 63, 0]);
        // The other way round, from the far end back to the start.
        let back = route_via(&route, [40, 60, 0], [0, 64, 0], 10).unwrap();
        assert_eq!(back.last(), Some(&[0, 64, 0]));
        // Not for a walk that starts far from it or goes elsewhere.
        assert!(route_via(&route, [20, 80, 30], [40, 60, 0], 10).is_none());
        assert!(route_via(&route, [1, 64, 1], [90, 64, 90], 10).is_none());
        let ends = vec![[0, 70, 0], [30, 70, 0]];
        assert!(ping_pong(&ends, [2, 70, 1]));
        assert!(!ping_pong(&ends, [60, 70, 0]));
    }
    #[test]
    fn the_drop_limit_follows_health_and_fall_protection() {
        // No protection, full health: six blocks (three points), never below three.
        assert_eq!(drop_limit(20.0, 0), 6);
        assert_eq!(drop_limit(12.0, 0), 5);
        assert_eq!(drop_limit(9.0, 0), MAX_DROP);
        // Feather Falling IV boots (12): about half the damage, so further.
        let boots = fall_protection(&[(4, 0)]);
        assert_eq!(boots, 12);
        assert_eq!(drop_limit(20.0, boots), 8);
        // Full protection is capped at 20 (80 % less), and the drop at the planner's maximum.
        let full = fall_protection(&[(4, 4), (0, 4), (0, 4), (0, 4)]);
        assert_eq!(full, 20);
        assert_eq!(drop_limit(20.0, full), FAR_DROP);
        assert!((fall_damage(7, 12) - 2.08).abs() < 1e-3);
        // The planners read the limit set for their thread.
        set_drop_limit(MAX_DROP);
        let roof = |p: Pos| {
            if p[1] < 0 || (p[0] <= 2 && p[1] == 5) {
                Cell::Solid
            } else {
                Cell::Air
            }
        };
        assert!(search([0, 6, 0], [6, 0, 0], roof, || false).is_none_or(|p| !p.complete));
        set_drop_limit(6);
        assert!(
            search([0, 6, 0], [6, 0, 0], roof, || false)
                .unwrap()
                .complete
        );
        set_drop_limit(MAX_DROP);
    }
    #[test]
    fn a_goal_typed_as_the_ground_block_is_stood_on_and_far_levels_are_searched() {
        // The block under the feet (y = -1) given as the goal: stand on it.
        assert_eq!(standing_goal([3, -1, 0], &mut flat), Some([3, 0, 0]));
        assert_eq!(standing_goal([3, 0, 0], &mut flat), Some([3, 0, 0]));
        // A head-high goal comes down to the feet.
        assert_eq!(standing_goal([3, 1, 0], &mut flat), Some([3, 0, 0]));
        assert_eq!(standing_goal([3, 40, 0], &mut flat), None);
        // A goal thirty blocks down a staircase is found (the search reaches that far down).
        let stairs = |p: Pos| {
            if p[1] < -p[0].clamp(0, 30) || p[1] < -40 {
                Cell::Solid
            } else {
                Cell::Air
            }
        };
        set_reach(WIDE);
        let path = search([0, 0, 0], [30, -30, 0], stairs, || false).unwrap();
        set_reach(NEAR);
        assert!(path.complete);
    }
    #[test]
    fn water_beside_is_harmless_and_a_walk_starts_from_the_water() {
        // A lake at x <= 0; the shore from x = 1 on.
        let lake = |p: Pos| {
            if p[1] < 0 && p[0] >= 1 {
                Cell::Solid
            } else if p[0] <= 0 && p[1] <= 0 {
                Cell::Water
            } else if p[1] < -10 {
                Cell::Solid
            } else {
                Cell::Air
            }
        };
        assert_eq!(classify("minecraft:water", false, true), Cell::Water);
        assert_eq!(classify("minecraft:seagrass", false, true), Cell::Water);
        assert_eq!(classify("minecraft:lava", false, true), Cell::Hazard);
        // The shore right by the water can be stood on (it could not before).
        assert!(standable([1, 0, 0], &mut { lake }));
        // Swimming in the lake, the walk climbs out and goes on.
        let path = search([0, 0, 0], [5, 0, 0], lake, || false).unwrap();
        assert!(path.complete);
        // Lava beside still keeps the walker away.
        let lava = |p: Pos| {
            if p == [0, 0, 0] {
                Cell::Hazard
            } else {
                lake(p)
            }
        };
        assert!(!standable([1, 0, 0], &mut { lava }));
    }
    #[test]
    fn a_glide_heads_for_the_landing_turns_from_walls_circles_and_levels_out() {
        let open = |_: f32| true;
        // Far off and high: straight at it, a moderate descent.
        let (yaw, pitch) = glide([0.0, 60.0, 0.0], [0.0, 40.0, 60.0], open);
        assert!(yaw.abs() < 1e-3, "{yaw}");
        assert!((8.0..=35.0).contains(&pitch));
        // A wall straight ahead: turned aside.
        let (yaw, _) = glide([0.0, 60.0, 0.0], [0.0, 40.0, 60.0], |y: f32| y.abs() > 10.0);
        assert!((yaw - 30.0).abs() < 1e-3);
        // High right over the landing: circling (a quarter off), not diving.
        let (yaw, pitch) = glide([0.0, 60.0, 0.0], [0.0, 30.0, 1.0], open);
        assert!((angle_between(0.0, yaw) - 90.0).abs() < 1e-3);
        assert!(pitch <= 20.0);
        // Low over it: level.
        let (_, pitch) = glide([0.0, 43.0, 0.0], [0.0, 40.0, 10.0], open);
        assert!(pitch < 0.0);
    }
    #[test]
    fn a_fence_gate_in_a_fenced_tunnel_is_the_way_through() {
        // A tunnel two blocks high (x from -6 to 6, z from -1 to 1), fenced across at x = 2 with a
        // gate in the middle.
        let tunnel = |p: Pos| {
            if p[1] < 0 || p[1] >= 2 || p[2].abs() > 1 {
                Cell::Solid
            } else if p[0] == 2 && p[1] == 0 {
                if p[2] == 0 { Cell::Door } else { Cell::Blocked }
            } else {
                Cell::Air
            }
        };
        let path = search([0, 0, 0], [5, 0, 1], tunnel, || false).unwrap();
        assert!(path.complete, "{:?}", path.nodes);
        assert!(path.nodes.contains(&[2, 0, 0]));
    }
    #[test]
    fn hanging_vines_are_climbed_not_walked_along() {
        // Vines hanging at x = 1..=3 over a gap, nothing under them.
        let jungle = |p: Pos| {
            if (1..=3).contains(&p[0]) && p[1] >= 0 && p[1] <= 4 && p[2] == 0 {
                Cell::Climb
            } else if p[1] < 0 && !(1..=3).contains(&p[0]) {
                Cell::Solid
            } else {
                Cell::Air
            }
        };
        assert!(!traversable([1, 2, 0], [2, 2, 0], &mut { jungle }));
        // Stepping into one from firm ground, or climbing it, is fine.
        assert!(traversable([0, 0, 0], [1, 0, 0], &mut { jungle }));
        assert!(traversable([1, 0, 0], [1, 1, 0], &mut { jungle }));
    }
    #[test]
    fn snow_layers_are_walked_over_or_stood_on_by_their_height() {
        let layers = |n: f64| [[0.0, 0.0, 0.0, 1.0, n / 8.0, 1.0]];
        let snow = |n: f64| classify_shape("minecraft:snow", false, false, &layers(n), Cell::Air);
        for n in 1..=5 {
            assert_eq!(snow(f64::from(n)), Cell::Air, "{n} layers");
        }
        assert_eq!(snow(6.0), Cell::Solid);
        assert_eq!(snow(7.0), Cell::Solid);
        assert_eq!(classify("minecraft:snow_block", true, false), Cell::Solid);
        assert_eq!(
            classify("minecraft:powder_snow", false, false),
            Cell::Hazard
        );
    }
    #[test]
    fn with_elytra_any_height_is_dropped() {
        // A cliff forty blocks high with no other way down.
        let cliff = |p: Pos| {
            if (p[0] <= 2 && p[1] == 39) || p[1] < 0 {
                Cell::Solid
            } else {
                Cell::Air
            }
        };
        let down = |max| {
            search_near_drop([0, 40, 0], [6, 0, 0], 0, max, cliff, || false)
                .is_some_and(|p| p.complete)
        };
        assert!(!down(FAR_DROP));
        set_drop_limit(ELYTRA_DROP);
        assert!(down(ELYTRA_DROP));
        // The walker's own check of the step agrees on this thread.
        assert!(traversable([2, 40, 0], [3, 0, 0], &mut { cliff }));
        set_drop_limit(MAX_DROP);
        assert!(!traversable([2, 40, 0], [3, 0, 0], &mut { cliff }));
    }
    #[test]
    fn trapdoors_are_walked_over_opened_or_passed_by_their_state() {
        let shaped = |name: &str, outline: &[[f64; 6]]| {
            classify_shape(
                &format!("minecraft:{name}"),
                false,
                false,
                outline,
                Cell::Air,
            )
        };
        let bottom = [[0.0, 0.0, 0.0, 1.0, 0.1875, 1.0]];
        let top = [[0.0, 0.8125, 0.0, 1.0, 1.0, 1.0]];
        let open = [[0.0, 0.0, 0.0, 1.0, 1.0, 0.1875]];
        assert_eq!(shaped("oak_trapdoor", &bottom), Cell::Air);
        assert_eq!(shaped("oak_trapdoor", &top), Cell::Door);
        assert_eq!(shaped("oak_trapdoor", &open), Cell::Air);
        assert_eq!(shaped("iron_trapdoor", &top), Cell::Solid);
        assert_eq!(shaped("iron_trapdoor", &open), Cell::Blocked);
        assert!(openable("minecraft:spruce_trapdoor") && !openable("minecraft:iron_trapdoor"));
        // Out of the top of a ladder through a hatch: the cell over the ladder holds the body.
        let shaft = |p: Pos| {
            if p[0] == 0 && p[2] == 0 && (0..=3).contains(&p[1]) {
                Cell::Climb
            } else if p == [0, 4, 0] {
                Cell::Door
            } else if p[1] < 0 || (p[0] == 1 && p[1] <= 3) {
                Cell::Solid
            } else {
                Cell::Air
            }
        };
        assert!(traversable([0, 3, 0], [0, 4, 0], &mut { shaft }));
    }
    #[test]
    fn partial_blocks_are_floors_steps_or_walls_by_their_shape() {
        let shaped = |name: &str, outline: &[[f64; 6]]| {
            classify_shape(
                &format!("minecraft:{name}"),
                false,
                false,
                outline,
                Cell::Air,
            )
        };
        // Low enough to step onto: walked over.
        let bed = [[0.0, 0.0, 0.0, 1.0, 0.5625, 1.0]];
        assert_eq!(shaped("red_bed", &bed), Cell::Air);
        // Nearly a block and nearly the whole cell: stood on (a jump up, like a block).
        let chest = [[0.0625, 0.0, 0.0625, 0.9375, 0.875, 0.9375]];
        assert_eq!(shaped("chest", &chest), Cell::Solid);
        let composter = [
            [0.0, 0.0, 0.0, 1.0, 0.125, 1.0],
            [0.0, 0.0, 0.0, 0.125, 1.0, 1.0],
            [0.875, 0.0, 0.0, 1.0, 1.0, 1.0],
        ];
        assert_eq!(shaped("composter", &composter), Cell::Solid);
        // A fence with four arms covers the cell but is a block and a half high.
        let cross = [
            [0.375, 0.0, 0.375, 0.625, 1.0, 0.625],
            [0.0, 0.0, 0.4375, 1.0, 1.0, 0.5625],
            [0.4375, 0.0, 0.0, 0.5625, 1.0, 1.0],
        ];
        assert_eq!(shaped("oak_fence", &cross), Cell::Blocked);
        assert_eq!(shaped("cobblestone_wall", &cross), Cell::Blocked);
        assert_eq!(shaped("glass_pane", &cross), Cell::Blocked);
        // A thin rod a block high is neither a floor nor a way through.
        let rod = [[0.4375, 0.0, 0.4375, 0.5625, 1.0, 0.5625]];
        assert_eq!(shaped("end_rod", &rod), Cell::Blocked);
    }
    #[test]
    fn distant_goal_is_segmented_and_cancel_is_honored() {
        let path = search([0, 0, 0], [10000, 0, 0], flat, || false).unwrap();
        assert!(!path.complete);
        assert!(path.nodes.len() <= 129);
        assert!(path.nodes.last().unwrap()[0] > 0);
        assert!(search([0, 0, 0], [5, 0, 0], flat, || true).is_none());
    }
    #[test]
    fn controller_arrives_pauses_and_detects_stalls() {
        let mut n = Navigator::default();
        n.begin([0, 0, 1], true);
        n.accept(Some(Path {
            nodes: vec![[0, 0, 0], [0, 0, 1]],
            complete: true,
        }));
        assert_eq!(n.input([0.5, 0., 0.5], 0., true).forward, 1.);
        assert_eq!(n.input([0.5, 0., 0.5], 90., true).strafe, 1.);
        n.pause();
        assert_eq!(n.input([0.5, 0., 0.5], 0., true), Input::default());
        n.begin([0, 0, 1], true);
        n.accept(Some(Path {
            nodes: vec![[0, 0, 0], [0, 0, 1]],
            complete: true,
        }));
        for _ in 0..65 {
            n.input([0.5, 0., 0.5], 0., true);
        }
        assert_eq!(n.status, Status::Stuck);
        n.begin([0, 0, 1], true);
        n.accept(Some(Path {
            nodes: vec![[0, 0, 0], [0, 0, 1]],
            complete: true,
        }));
        assert_eq!(n.input([0.5, 0., 1.5], 0., true), Input::default());
        assert_eq!(n.status, Status::Arrived);
    }
    #[test]
    fn preview_never_moves_and_negative_coordinates_floor() {
        let mut n = Navigator::default();
        n.begin([1, 0, 0], false);
        n.accept(Some(Path {
            nodes: vec![[0, 0, 0], [1, 0, 0]],
            complete: true,
        }));
        assert_eq!(n.input([0.5, 0., 0.5], 0., true), Input::default());
        assert_eq!(block([-0.1, 64., -16.1]), [-1, 64, -17]);
    }
}

#[cfg(test)]
mod dig_search_tests {
    use super::*;

    fn rock(p: Pos) -> Cell {
        if p[1] < 0 || (p[0] >= 1 && p[1] <= 1) {
            Cell::Solid
        } else {
            Cell::Air
        }
    }

    #[test]
    fn digs_a_straight_tunnel_through_rock_to_the_goal() {
        // Deep rock: climbing out would cost more than tunnelling straight through.
        let deep = |p: Pos| {
            if p[1] < 0 || (p[0] >= 1 && p[1] <= 30) {
                Cell::Solid
            } else {
                Cell::Air
            }
        };
        let path =
            search_dig([0, 0, 0], [4, 0, 0], false, false, deep, |_| true, || false).unwrap();
        assert!(path.complete);
        assert_eq!(path.steps.len(), 4);
        for step in &path.steps {
            // Feet and head are rock; the head goes first.
            assert_eq!(step.dig.len(), 2);
            assert!(step.dig[0][1] > step.dig[1][1]);
            assert_eq!(step.place, None);
        }
    }

    #[test]
    fn refuses_to_dig_what_is_not_diggable_and_prefers_walking() {
        assert!(
            search_dig(
                [0, 0, 0],
                [4, 0, 0],
                false,
                false,
                rock,
                |_| false,
                || false
            )
            .is_none_or(|p| !p.complete)
        );
        let open = |p: Pos| if p[1] < 0 { Cell::Solid } else { Cell::Air };
        let path = search_dig([0, 0, 0], [3, 0, 0], false, true, open, |_| true, || false).unwrap();
        assert!(
            path.steps
                .iter()
                .all(|s| s.dig.is_empty() && s.place.is_none())
        );
    }

    #[test]
    fn stairs_climb_one_block_per_step_and_stop_next_to_a_buried_ore() {
        // Solid ground at y < 0; everything above y = 0 is rock except the start column.
        let cell = |p: Pos| {
            if p[1] < 0 || p[0] != 0 && p[1] <= 6 {
                Cell::Solid
            } else {
                Cell::Air
            }
        };
        let ore = [4, 5, 0];
        let path = search_dig([0, 0, 0], ore, true, false, cell, |_| true, || false).unwrap();
        assert!(path.complete);
        let last = path.steps.last().unwrap().to;
        assert_eq!((last[0] - ore[0]).abs() + (last[2] - ore[2]).abs(), 1);
        assert!((0..=1).contains(&(ore[1] - last[1])));
        let mut at = path.start;
        for step in &path.steps {
            assert!((step.to[1] - at[1]).abs() <= 1);
            at = step.to;
        }
    }

    #[test]
    fn bridges_a_gap_and_never_touches_fluids_or_unknown_blocks() {
        let gap = |p: Pos| {
            if p[1] < 0 && !(1..=2).contains(&p[0]) {
                Cell::Solid
            } else {
                Cell::Air
            }
        };
        let path = search_dig([0, 0, 0], [3, 0, 0], false, true, gap, |_| true, || false).unwrap();
        assert!(path.complete);
        assert_eq!(
            path.steps
                .iter()
                .filter_map(|s| s.place)
                .collect::<Vec<_>>(),
            vec![[1, -1, 0], [2, -1, 0]]
        );
        assert!(
            search_dig([0, 0, 0], [3, 0, 0], false, false, gap, |_| true, || false)
                .is_none_or(|p| !p.complete)
        );
        let lava = |p: Pos| {
            if p == [1, 1, 0] {
                Cell::Hazard
            } else {
                rock(p)
            }
        };
        assert!(
            search_dig([0, 0, 0], [4, 0, 0], false, false, lava, |_| true, || false)
                .is_none_or(|p| !p.complete)
        );
        let unknown = |p: Pos| {
            if p == [3, 0, 0] {
                Cell::Unknown
            } else {
                rock(p)
            }
        };
        assert!(
            search_dig(
                [0, 0, 0],
                [4, 0, 0],
                false,
                false,
                unknown,
                |_| true,
                || false
            )
            .is_none_or(|p| !p.complete)
        );
    }

    #[test]
    fn dig_search_honors_cancellation_and_needs_a_standing_start() {
        assert!(search_dig([0, 0, 0], [4, 0, 0], false, false, rock, |_| true, || true).is_none());
        assert!(search_dig([0, 5, 0], [4, 5, 0], false, false, rock, |_| true, || false).is_none());
    }
}
