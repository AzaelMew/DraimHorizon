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
    } else if full_collision || matches!(name, "farmland" | "dirt_path") {
        Cell::Solid
    } else {
        Cell::Blocked
    }
}

pub fn standable(at: Pos, cell: &mut impl FnMut(Pos) -> Cell) -> bool {
    cell(at) == Cell::Air
        && cell([at[0], at[1] + 1, at[2]]) == Cell::Air
        && cell([at[0], at[1] - 1, at[2]]) == Cell::Solid
        && [[1, 0], [-1, 0], [0, 1], [0, -1]]
            .into_iter()
            .all(|[x, z]| (-1..=1).all(|y| cell([at[0] + x, at[1] + y, at[2] + z]) != Cell::Hazard))
}

pub fn traversable(from: Pos, to: Pos, cell: &mut impl FnMut(Pos) -> Cell) -> bool {
    let dy = to[1] - from[1];
    (to[0] - from[0]).abs() + (to[2] - from[2]).abs() == 1
        && dy.abs() <= 1
        && standable(to, cell)
        && (dy <= 0 || cell([from[0], from[1] + 2, from[2]]) == Cell::Air)
        && (dy >= 0 || cell([to[0], from[1] + 1, to[2]]) == Cell::Air)
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
            Cell::Air => {}
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

fn heuristic(a: Pos, b: Pos) -> i32 {
    ((a[0] - b[0]).abs() + (a[2] - b[2]).abs()) * 10 + (a[1] - b[1]).abs() * 2
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
        for [dx, dz] in [[1, 0], [-1, 0], [0, 1], [0, -1]] {
            for dy in [0, 1, -1] {
                let next = [at[0] + dx, at[1] + dy, at[2] + dz];
                if (next[0] - start[0]).abs() > RADIUS
                    || (next[2] - start[2]).abs() > RADIUS
                    || (next[1] - start[1]).abs() > 16
                    || !traversable(at, next, &mut cell)
                {
                    continue;
                }
                let next_cost = cost + 10 + if dy > 0 { 4 } else { dy.abs() * 2 };
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
        if distance(position, target) > 2.6 {
            self.pause();
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
        Input {
            forward,
            strafe,
            jump: grounded && target[1] > position[1] + 0.5 && dx.hypot(dz) < 1.25,
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
    fn refuses_unknown_terrain_hazards_and_large_drops() {
        assert!(!standable([0, 0, 0], &mut |_| Cell::Unknown));
        assert!(!standable([0, 0, 0], &mut |p| if p == [1, 0, 0] {
            Cell::Hazard
        } else {
            flat(p)
        }));
        assert!(!traversable([0, 0, 0], [1, -2, 0], &mut flat));
        assert!(!traversable([0, 0, 0], [1, 0, 1], &mut flat));
        assert_eq!(classify("minecraft:magma_block", true, false), Cell::Hazard);
        assert_eq!(classify("minecraft:oak_slab", false, false), Cell::Blocked);
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
