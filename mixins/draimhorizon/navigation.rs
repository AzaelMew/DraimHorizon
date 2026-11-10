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
/// for the head) and is held up, by a floor or by the ladder it is in, with no hazard beside.
pub fn standable(at: Pos, cell: &mut impl FnMut(Pos) -> Cell) -> bool {
    let feet = cell(at);
    open(feet)
        && open(cell([at[0], at[1] + 1, at[2]]))
        && (feet == Cell::Climb
            || matches!(cell([at[0], at[1] - 1, at[2]]), Cell::Solid | Cell::Climb))
        && [[1, 0], [-1, 0], [0, 1], [0, -1]]
            .into_iter()
            .all(|[x, z]| (-1..=1).all(|y| cell([at[0] + x, at[1] + y, at[2] + z]) != Cell::Hazard))
}

/// How far a leg of a far walk goes, and the shorter reach tried when the long one has no way.
pub const LEG_REACH: [f64; 2] = [24.0, 14.0];
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
/// The longest drop at all, taken only when there is no other way (off a roof, down a cliff to
/// the farm): six blocks cost three health points, which eating makes good again.
pub const FAR_DROP: i32 = 6;
/// What each block of a drop beyond `MAX_DROP` costs a route: a long detour is taken first.
const FAR_DROP_COST: i32 = 40;
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
            (-FAR_DROP..=1).contains(&dy)
                // A door is passed on the level, never with a step.
                && (dy == 0 || !(door(cell(from)) || door(cell(to))))
                && (dy <= 0 || open(cell([from[0], from[1] + 2, from[2]])))
                && column_open(cell)
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
                Cell::Hazard | Cell::Unknown
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
                        Cell::Hazard | Cell::Unknown
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
const MOVES: [([i32; 3], i32); 50] = {
    let mut moves = [([0, 0, 0], 0); 50];
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
    moves
};
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
    search_near_drop(start, goal, radius, MAX_DROP, cell, canceled)
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
    if !standable(start, &mut cell) {
        return None;
    }
    let began = Instant::now();
    let mut open = BinaryHeap::from([Reverse((heuristic(start, goal), 0, start))]);
    let mut costs = HashMap::from([(start, 0)]);
    let mut parents = HashMap::new();
    let mut best = start;
    let mut expanded = 0;
    while let Some(Reverse((_, cost, at))) = open.pop() {
        if canceled() {
            return None;
        }
        if cost != costs[&at] {
            continue;
        }
        if heuristic(at, goal) < heuristic(best, goal) {
            best = at;
        }
        if reached(at) {
            best = at;
            break;
        }
        expanded += 1;
        if expanded >= MAX_NODES || began.elapsed() > Duration::from_millis(80) {
            break;
        }
        for ([dx, dy, dz], base) in MOVES {
            let next = [at[0] + dx, at[1] + dy, at[2] + dz];
            if dy < -max_drop
                || (next[0] - start[0]).abs() > RADIUS
                || (next[2] - start[2]).abs() > RADIUS
                || (next[1] - start[1]).abs() > 16
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
                + (-dy - MAX_DROP).max(0) * FAR_DROP_COST;
            if costs.get(&next).is_none_or(|old| next_cost < *old) {
                costs.insert(next, next_cost);
                parents.insert(next, at);
                open.push(Reverse((
                    next_cost + heuristic(next, goal),
                    next_cost,
                    next,
                )));
            }
        }
    }
    if canceled() || (best == start && !reached(start)) {
        return None;
    }
    let mut nodes = vec![best];
    while let Some(parent) = parents.get(nodes.last().unwrap()) {
        nodes.push(*parent);
        if nodes.len() > MAX_NODES {
            return None;
        }
    }
    nodes.reverse();
    Some(Path {
        nodes,
        complete: reached(best),
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
    pub follow: Option<Follow>,
    pub status: Status,
    pub goal: Option<Pos>,
    pub path: Vec<Pos>,
    pub next: usize,
    pub complete: bool,
    pub running: bool,
    progress: Option<[f64; 3]>,
    stalled: u16,
    recoveries: u8,
    recovery_origin: Option<[f64; 3]>,
}
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Input {
    pub forward: f32,
    pub strafe: f32,
    pub jump: bool,
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
        while self.next < self.path.len()
            && distance(position, center(self.path[self.next]))
                < if self.precise { 0.12 } else { 0.26 }
        {
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
        let off = distance(position, target);
        if off > 2.6 {
            // Shoved off the route (a player walking into the walker): plan again from where
            // it stands. Only a long way off (a teleport) is a pause.
            if off > 8.0 {
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
        if self.climbing && dx.hypot(dz) < 0.2 {
            // Straight up or down a ladder: climb with the jump key, or let go and slide.
            return Input {
                forward: 0.0,
                strafe: 0.0,
                jump: target[1] > position[1] + 0.1,
            };
        }
        let yaw = f64::from(yaw).to_radians();
        let forward = -dx * yaw.sin() + dz * yaw.cos();
        let left = dx * yaw.cos() + dz * yaw.sin();
        let sector = (left.atan2(forward) / (std::f64::consts::PI / 4.0)).round() as i32;
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
        }
    }
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
        assert!((straight[0] - 24.0).abs() < 1e-9 && straight[2].abs() < 1e-9);
        // Turned 35 degrees to either side, at the same reach.
        let left = leg_target(from, goal, 1).unwrap();
        let right = leg_target(from, goal, 2).unwrap();
        assert!(left[2] > 1.0 && right[2] < -1.0);
        assert!((nav_len(left) - 24.0).abs() < 1e-9 && (nav_len(right) - 24.0).abs() < 1e-9);
        // The shorter reach comes after all the turns at the long one.
        let shorter = leg_target(from, goal, 7).unwrap();
        assert!((shorter[0] - 14.0).abs() < 1e-9);
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
        assert!(!traversable([0, 0, 0], [1, -7, 0], &mut |p| if p[1] < -7 {
            Cell::Solid
        } else {
            Cell::Air
        }));
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
        // With stairs down at hand, the stairs are taken instead of the long drop.
        let stairs = |p: Pos| {
            if p[2] == 3 && p[0] >= 3 && p[1] < 6 - (p[0] - 2) {
                Cell::Solid
            } else {
                roof(p)
            }
        };
        let path = search_near_drop([0, 6, 0], [6, 0, 0], 0, FAR_DROP, stairs, || false).unwrap();
        assert!(path.complete);
        assert!(
            path.nodes
                .windows(2)
                .all(|w| w[1][1] - w[0][1] >= -MAX_DROP)
        );
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
