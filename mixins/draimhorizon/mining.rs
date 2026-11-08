// Bounded ore mining jobs. Only server-confirmed block changes advance the count.
use super::{claim_borders_experimental::ORES, claim_borders_navigation as nav};
use std::{
    sync::atomic::{AtomicU32, Ordering},
    time::{Duration, Instant},
};
pub const SCAN_RADIUS: i32 = 16;
#[derive(Clone, Copy, Debug)]
pub struct Block {
    pub cell: nav::Cell,
    pub ore: bool,
    pub falling: bool,
    /// Solid and safe to break: not bedrock-like and without a block entity.
    pub breakable: bool,
}
impl Default for Block {
    fn default() -> Self {
        Self {
            cell: nav::Cell::Unknown,
            ore: false,
            falling: false,
            breakable: false,
        }
    }
}
/// What the miner can be told to collect besides the highlighted ores.
pub struct Target {
    pub label: &'static str,
    pub label_en: &'static str,
    pub blocks: &'static [&'static str],
}
const EXTRA: [Target; 13] = [
    Target {
        label: "Камень",
        label_en: "Stone",
        blocks: &[
            "minecraft:stone",
            "minecraft:deepslate",
            "minecraft:andesite",
            "minecraft:diorite",
            "minecraft:granite",
            "minecraft:tuff",
        ],
    },
    Target {
        label: "Булыжник",
        label_en: "Cobblestone",
        blocks: &[
            "minecraft:cobblestone",
            "minecraft:cobbled_deepslate",
            "minecraft:mossy_cobblestone",
        ],
    },
    Target {
        label: "Земля",
        label_en: "Dirt",
        blocks: &[
            "minecraft:dirt",
            "minecraft:grass_block",
            "minecraft:coarse_dirt",
            "minecraft:podzol",
            "minecraft:mycelium",
            "minecraft:rooted_dirt",
        ],
    },
    Target {
        label: "Песок",
        label_en: "Sand",
        blocks: &["minecraft:sand", "minecraft:red_sand"],
    },
    Target {
        label: "Гравий",
        label_en: "Gravel",
        blocks: &["minecraft:gravel"],
    },
    Target {
        label: "Глина",
        label_en: "Clay",
        blocks: &["minecraft:clay"],
    },
    Target {
        label: "Дерево",
        label_en: "Wood",
        blocks: &[
            "minecraft:oak_log",
            "minecraft:spruce_log",
            "minecraft:birch_log",
            "minecraft:jungle_log",
            "minecraft:acacia_log",
            "minecraft:dark_oak_log",
            "minecraft:mangrove_log",
            "minecraft:cherry_log",
            "minecraft:pale_oak_log",
            "minecraft:crimson_stem",
            "minecraft:warped_stem",
        ],
    },
    Target {
        label: "Обсидиан",
        label_en: "Obsidian",
        blocks: &["minecraft:obsidian", "minecraft:crying_obsidian"],
    },
    Target {
        label: "Светокамень",
        label_en: "Glowstone",
        blocks: &["minecraft:glowstone"],
    },
    Target {
        label: "Камень Незера",
        label_en: "Nether stone",
        blocks: &[
            "minecraft:netherrack",
            "minecraft:basalt",
            "minecraft:blackstone",
            "minecraft:soul_sand",
            "minecraft:soul_soil",
        ],
    },
    Target {
        label: "Песчаник",
        label_en: "Sandstone",
        blocks: &["minecraft:sandstone", "minecraft:red_sandstone"],
    },
    Target {
        label: "Эндерняк",
        label_en: "End stone",
        blocks: &["minecraft:end_stone"],
    },
    Target {
        label: "Тыквы и арбузы",
        label_en: "Pumpkins and melons",
        blocks: &["minecraft:pumpkin", "minecraft:melon"],
    },
];
/// Number of things the miner can collect: the highlighted ores, then the extra targets.
pub const COUNT: usize = ORES.len() + EXTRA.len();
/// (Russian label, English label, blocks) for a mining target index.
pub fn target(index: u8) -> Option<(&'static str, &'static str, &'static [&'static str])> {
    let index = usize::from(index);
    if let Some(ore) = ORES.get(index) {
        return Some((ore.label, ore.label_en, ore.blocks));
    }
    EXTRA
        .get(index.checked_sub(ORES.len())?)
        .map(|t| (t.label, t.label_en, t.blocks))
}
/// Every target index as a bit.
pub const ALL: u32 = (1 << COUNT) - 1;
/// A job that collects whichever of the selected targets is nearest.
pub const ANY: u8 = 200;
static SELECTION: AtomicU32 = AtomicU32::new(1);
/// The targets an `ANY` job collects. Read by the search worker too.
pub fn select(mask: u32) {
    SELECTION.store(mask & ALL, Ordering::Relaxed);
}
pub fn selection() -> u32 {
    SELECTION.load(Ordering::Relaxed)
}
pub fn matches(ore: u8, name: &str) -> bool {
    if ore == ANY {
        let mask = selection();
        return (0..COUNT as u8).any(|i| {
            mask & (1 << i) != 0 && target(i).is_some_and(|(_, _, blocks)| blocks.contains(&name))
        });
    }
    target(ore).is_some_and(|(_, _, blocks)| blocks.contains(&name))
}
pub fn falling(name: &str) -> bool {
    matches!(
        name,
        "minecraft:sand"
            | "minecraft:red_sand"
            | "minecraft:gravel"
            | "minecraft:anvil"
            | "minecraft:chipped_anvil"
            | "minecraft:damaged_anvil"
    ) || name.ends_with("_concrete_powder")
}
pub fn safe_target(
    at: nav::Pos,
    position: [f64; 3],
    block: &mut impl FnMut(nav::Pos) -> Block,
) -> bool {
    if !block(at).ore {
        return false;
    }
    if position[1] > f64::from(at[1])
        && position[1] <= f64::from(at[1]) + 1.2
        && (position[0] - f64::from(at[0]) - 0.5).abs() <= 0.82
        && (position[2] - f64::from(at[2]) - 0.5).abs() <= 0.82
    {
        return false;
    }
    if block([at[0], at[1] + 1, at[2]]).falling {
        return false;
    }
    let neighbors = [
        [1, 0, 0],
        [-1, 0, 0],
        [0, 1, 0],
        [0, -1, 0],
        [0, 0, 1],
        [0, 0, -1],
    ]
    .map(|d| block([at[0] + d[0], at[1] + d[1], at[2] + d[2]]));
    neighbors.iter().any(|b| b.cell == nav::Cell::Air)
        && neighbors
            .iter()
            .all(|b| !matches!(b.cell, nav::Cell::Hazard | nav::Cell::Unknown))
}
pub fn plan(
    start: nav::Pos,
    excluded: &[nav::Pos],
    mut block: impl FnMut(nav::Pos) -> Block,
    canceled: impl Fn() -> bool,
) -> Option<(nav::Pos, nav::Path)> {
    let began = Instant::now();
    let mut candidates = Vec::new();
    for y in -12..=12 {
        for z in -SCAN_RADIUS..=SCAN_RADIUS {
            if canceled() || began.elapsed() > Duration::from_millis(250) {
                return None;
            }
            for x in -SCAN_RADIUS..=SCAN_RADIUS {
                let at = [start[0] + x, start[1] + y, start[2] + z];
                if block(at).ore && !excluded.contains(&at) {
                    candidates.push((x * x + y * y + z * z, at));
                }
            }
        }
    }
    candidates.sort_unstable();
    candidates.truncate(16);
    for (_, target) in candidates {
        for [x, z] in [[1, 0], [-1, 0], [0, 1], [0, -1]] {
            for y in [0, -1, 1] {
                if canceled() || began.elapsed() > Duration::from_millis(500) {
                    return None;
                }
                let at = [target[0] + x, target[1] + y, target[2] + z];
                if !safe_target(target, nav::center(at), &mut block)
                    || !nav::standable(at, &mut |p| block(p).cell)
                {
                    continue;
                }
                if let Some(path) = nav::search(start, at, |p| block(p).cell, &canceled)
                    && path.complete
                {
                    return Some((target, path));
                }
            }
        }
    }
    None
}
/// Half-size of the cube searched for ore that has no open face yet.
pub const BURIED_RADIUS: i32 = 24;
/// The nearest ore of the kind that can be reached by digging, whether or not it is exposed,
/// with the route that gets next to it. `hints` are ores the player already sees marked by
/// the ore experiment; they are tried first. Nothing here edits the world.
pub fn plan_buried(
    start: nav::Pos,
    excluded: &[nav::Pos],
    hints: &[nav::Pos],
    can_place: bool,
    block: impl FnMut(nav::Pos) -> Block,
    canceled: impl Fn() -> bool,
) -> Option<(nav::Pos, nav::DigPath)> {
    let began = Instant::now();
    let block = std::cell::RefCell::new(block);
    let read = |at: nav::Pos| (*block.borrow_mut())(at);
    let distance = |at: nav::Pos| {
        let [x, y, z] = [at[0] - start[0], at[1] - start[1], at[2] - start[2]];
        x * x + y * y + z * z
    };
    let mut candidates: Vec<(i32, nav::Pos)> = Vec::new();
    for &at in hints {
        if read(at).ore && !excluded.contains(&at) && !candidates.iter().any(|c| c.1 == at) {
            // A marked ore wins over an unmarked one of similar distance.
            candidates.push((distance(at) / 2, at));
        }
    }
    for y in -BURIED_RADIUS..=BURIED_RADIUS {
        for z in -BURIED_RADIUS..=BURIED_RADIUS {
            if canceled() || began.elapsed() > Duration::from_millis(300) {
                return None;
            }
            for x in -BURIED_RADIUS..=BURIED_RADIUS {
                let at = [start[0] + x, start[1] + y, start[2] + z];
                if read(at).ore && !excluded.contains(&at) && !candidates.iter().any(|c| c.1 == at)
                {
                    candidates.push((distance(at), at));
                }
            }
        }
    }
    candidates.sort_unstable();
    candidates.truncate(12);
    for (_, target) in candidates {
        if canceled() || began.elapsed() > Duration::from_millis(900) {
            return None;
        }
        let path = nav::search_dig(
            start,
            target,
            true,
            can_place,
            |at| read(at).cell,
            |at| {
                let b = read(at);
                b.cell == nav::Cell::Solid
                    && b.breakable
                    && !b.falling
                    && !read([at[0], at[1] + 1, at[2]]).falling
            },
            &canceled,
        );
        if let Some(path) = path.filter(|p| p.complete) {
            return Some((target, path));
        }
    }
    None
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Phase {
    Searching,
    Approaching,
    Breaking,
    Confirming,
    Done,
    NoTarget,
    Refused,
    Tool,
    Inventory,
    Timeout,
}
#[derive(Clone, Debug)]
pub struct Job {
    pub ore: u8,
    pub limit: u16,
    pub confirmed: u16,
    pub active: bool,
    pub phase: Phase,
    pub target: Option<nav::Pos>,
    pub excluded: Vec<nav::Pos>,
    pub started_ms: u64,
    start_sequence: Option<i32>,
    await_sequence: Option<i32>,
}
impl Job {
    pub fn excavation(target: nav::Pos) -> Self {
        let mut job = Self::new(0, 1).unwrap();
        job.ore = u8::MAX;
        job.select(target);
        job.phase = Phase::Breaking;
        job
    }
    pub fn new(ore: u8, limit: u16) -> Option<Self> {
        if (usize::from(ore) >= COUNT && ore != ANY) || !(1..=64).contains(&limit) {
            return None;
        }
        Some(Self {
            ore,
            limit,
            confirmed: 0,
            active: true,
            phase: Phase::Searching,
            target: None,
            excluded: Vec::new(),
            started_ms: 0,
            start_sequence: None,
            await_sequence: None,
        })
    }
    pub fn select(&mut self, target: nav::Pos) {
        self.target = Some(target);
        self.phase = Phase::Approaching;
        self.start_sequence = None;
        self.await_sequence = None;
    }
    pub fn skip(&mut self) {
        if let Some(target) = self.target.take()
            && !self.excluded.contains(&target)
        {
            self.excluded.push(target);
        }
        self.phase = if self.excluded.len() >= 32 {
            self.active = false;
            Phase::NoTarget
        } else {
            Phase::Searching
        };
        self.start_sequence = None;
        self.await_sequence = None;
    }
    pub fn sent(&mut self, at: nav::Pos, action: i32, sequence: i32) {
        if self.target != Some(at) || !self.active || self.phase != Phase::Breaking {
            return;
        }
        if action == 0 {
            self.start_sequence = Some(sequence);
        }
        if action == 3 {
            self.await_sequence = Some(sequence);
            self.phase = Phase::Confirming;
        }
    }
    pub fn predicted_missing(&mut self) {
        if self.phase == Phase::Breaking && self.start_sequence.is_some() {
            self.await_sequence = self.start_sequence;
            self.phase = Phase::Confirming;
        }
    }
    pub fn acknowledge(&mut self, sequence: i32, still_ore: bool) {
        let awaited = if matches!(self.phase, Phase::Confirming | Phase::Timeout) {
            self.await_sequence
        } else if self.phase == Phase::Breaking {
            self.start_sequence
        } else {
            None
        };
        if awaited.is_none_or(|s| sequence < s) {
            return;
        }
        if still_ore {
            if matches!(self.phase, Phase::Confirming | Phase::Timeout) {
                self.active = false;
                self.phase = Phase::Refused;
            }
            return;
        }
        self.confirmed += 1;
        self.target = None;
        self.start_sequence = None;
        self.await_sequence = None;
        self.phase = if self.confirmed >= self.limit {
            self.active = false;
            Phase::Done
        } else {
            Phase::Searching
        };
    }
    pub fn resume(&mut self) {
        if self.confirmed >= self.limit {
            return;
        }
        self.active = true;
        if self.phase == Phase::Timeout && self.await_sequence.is_some() {
            self.phase = Phase::Confirming;
        }
        if self.phase != Phase::Confirming {
            self.phase = Phase::Searching;
            self.target = None;
            self.start_sequence = None;
            self.await_sequence = None;
        }
    }
    pub fn awaiting_confirmation(&self) -> bool {
        self.await_sequence.is_some() && matches!(self.phase, Phase::Confirming | Phase::Timeout)
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn every_target_has_a_label_and_blocks_and_ores_keep_their_indices() {
        assert_eq!(COUNT, ORES.len() + 13);
        for index in 0..COUNT as u8 {
            let (ru, en, blocks) = target(index).unwrap();
            assert!(!ru.is_empty() && !en.is_empty() && !blocks.is_empty());
            assert!(blocks.iter().all(|b| b.starts_with("minecraft:")));
            assert!(matches(index, blocks[0]));
        }
        assert!(target(COUNT as u8).is_none());
        assert!(matches(0, "minecraft:diamond_ore"));
        assert!(matches(ORES.len() as u8, "minecraft:stone"));
        assert!(!matches(ORES.len() as u8, "minecraft:diamond_ore"));
        assert!(Job::new(COUNT as u8 - 1, 8).is_some());
        assert!(Job::new(ANY, 8).is_some());
        select(1 | 1 << ORES.len());
        assert!(matches(ANY, "minecraft:diamond_ore"));
        assert!(matches(ANY, "minecraft:stone"));
        assert!(!matches(ANY, "minecraft:oak_log"));
        select(u32::MAX);
        assert_eq!(selection(), ALL);
        select(1);
        assert!(Job::new(COUNT as u8, 8).is_none());
    }
    #[test]
    fn counts_only_confirmed_breaks_and_refuses_server_rejections() {
        let mut job = Job::new(0, 2).unwrap();
        job.select([2, 0, 0]);
        job.phase = Phase::Breaking;
        job.sent([2, 0, 0], 0, 5);
        job.acknowledge(5, true);
        assert_eq!(job.confirmed, 0);
        assert!(job.active);
        job.sent([2, 0, 0], 3, 6);
        job.acknowledge(5, false);
        assert_eq!(job.confirmed, 0);
        job.acknowledge(6, false);
        job.acknowledge(6, false);
        assert_eq!(job.confirmed, 1);
        job.select([3, 0, 0]);
        job.phase = Phase::Breaking;
        job.sent([3, 0, 0], 0, 7);
        job.sent([3, 0, 0], 3, 8);
        job.acknowledge(8, true);
        assert_eq!(job.phase, Phase::Refused);
        assert!(!job.active);
        assert_eq!(job.confirmed, 1);
    }
    #[test]
    fn instant_breaks_and_paused_inflight_confirmations_preserve_the_limit() {
        let mut job = Job::new(0, 1).unwrap();
        job.select([2, 0, 0]);
        job.phase = Phase::Breaking;
        job.sent([2, 0, 0], 0, 4);
        job.predicted_missing();
        job.active = false;
        job.acknowledge(4, false);
        assert_eq!(job.confirmed, 1);
        assert_eq!(job.phase, Phase::Done);
        job.resume();
        assert!(!job.active);
        assert!(Job::new(COUNT as u8, 1).is_none());
        assert!(Job::new(0, 0).is_none());
        assert!(Job::new(0, 65).is_none());
        let mut late = Job::excavation([1, 0, 0]);
        late.sent([1, 0, 0], 0, 5);
        late.sent([1, 0, 0], 3, 6);
        late.phase = Phase::Timeout;
        late.active = false;
        assert!(late.awaiting_confirmation());
        late.resume();
        assert_eq!(late.phase, Phase::Confirming);
        late.active = false;
        late.acknowledge(6, false);
        assert_eq!(late.phase, Phase::Done);
        assert!(!late.active);
        assert_eq!(late.confirmed, 1);
    }
    #[test]
    fn plans_a_dig_route_to_buried_ore_and_skips_excluded_or_unbreakable_rock() {
        let ore = [5, 0, 0];
        let world = |p: nav::Pos| Block {
            cell: if p[1] < 0 || (p[0] >= 1 && p[1] <= 20) {
                nav::Cell::Solid
            } else {
                nav::Cell::Air
            },
            ore: p == ore,
            falling: false,
            breakable: true,
        };
        // Nothing is exposed, so the exposed-ore planner finds nothing here.
        assert!(plan([0, 0, 0], &[], world, || false).is_none());
        let (target, path) = plan_buried([0, 0, 0], &[], &[], false, world, || false).unwrap();
        assert_eq!(target, ore);
        assert!(path.complete);
        assert!(path.steps.iter().all(|s| !s.dig.is_empty()));
        let last = path.steps.last().unwrap().to;
        assert_eq!((last[0] - ore[0]).abs() + (last[2] - ore[2]).abs(), 1);
        // Excluded ore and a sealed-off ore are not routed to.
        assert!(plan_buried([0, 0, 0], &[ore], &[], false, world, || false).is_none());
        let sealed = |p: nav::Pos| Block {
            breakable: false,
            ..world(p)
        };
        assert!(plan_buried([0, 0, 0], &[], &[], false, sealed, || false).is_none());
        assert!(plan_buried([0, 0, 0], &[], &[], false, world, || true).is_none());
        // A hint is accepted only while the world still shows an ore there.
        assert!(plan_buried([0, 0, 0], &[ore], &[[9, 0, 0]], false, world, || false).is_none());
        let (target, _) = plan_buried([0, 0, 0], &[], &[ore], false, world, || false).unwrap();
        assert_eq!(target, ore);
    }
    #[test]
    fn plans_only_reachable_exposed_ore_and_never_mines_its_support() {
        let mut controller = nav::Navigator::default();
        controller.begin([1, 0, 0], true);
        controller.precise = true;
        controller.accept(Some(nav::Path {
            nodes: vec![[0, 0, 0], [1, 0, 0]],
            complete: true,
        }));
        assert_ne!(
            controller.input([1.3, 0., 0.5], 0., true),
            nav::Input::default()
        );
        assert_eq!(controller.status, nav::Status::Walking);
        assert_eq!(
            controller.input([1.45, 0., 0.5], 0., true),
            nav::Input::default()
        );
        assert_eq!(controller.status, nav::Status::Arrived);
        let ore = [2, 0, 0];
        let mut block = |p: nav::Pos| Block {
            cell: if p == ore || p[1] < 0 {
                nav::Cell::Solid
            } else {
                nav::Cell::Air
            },
            ore: p == ore,
            falling: false,
            breakable: true,
        };
        let (target, path) = plan([0, 0, 0], &[], &mut block, || false).unwrap();
        assert_eq!(target, ore);
        assert!(path.complete);
        assert!(!safe_target(ore, [2.5, 1., 0.5], &mut block));
        assert!(plan([0, 0, 0], &[ore], &mut block, || false).is_none());
        assert!(plan([0, 0, 0], &[], &mut block, || true).is_none());
        assert!(!safe_target(
            ore,
            [1.5, 0., 0.5],
            &mut |p| if p == [2, 1, 0] {
                Block {
                    cell: nav::Cell::Solid,
                    ore: false,
                    falling: true,
                    breakable: true,
                }
            } else {
                block(p)
            }
        ));
        assert!(!safe_target(
            ore,
            [1.5, 0., 0.5],
            &mut |p| if p == [2, 1, 0] {
                Block {
                    cell: nav::Cell::Hazard,
                    ore: false,
                    falling: false,
                    breakable: true,
                }
            } else {
                block(p)
            }
        ));
    }
}
