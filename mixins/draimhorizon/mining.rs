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
/// The item whose icon stands for a mining target: the ores in `ORES` order, then `EXTRA`.
const ICONS: [&str; COUNT] = [
    "minecraft:diamond",
    "minecraft:ancient_debris",
    "minecraft:emerald",
    "minecraft:gold_ingot",
    "minecraft:iron_ingot",
    "minecraft:copper_ingot",
    "minecraft:redstone",
    "minecraft:lapis_lazuli",
    "minecraft:coal",
    "minecraft:quartz",
    "minecraft:stone",
    "minecraft:cobblestone",
    "minecraft:dirt",
    "minecraft:sand",
    "minecraft:gravel",
    "minecraft:clay",
    "minecraft:oak_log",
    "minecraft:obsidian",
    "minecraft:glowstone",
    "minecraft:netherrack",
    "minecraft:sandstone",
    "minecraft:end_stone",
    "minecraft:pumpkin",
];
/// The icon item of a mining target index.
pub fn icon(index: u8) -> Option<&'static str> {
    ICONS.get(usize::from(index)).copied()
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
/// Plants and leaves that stand between the player and the block to mine and are cleared out
/// of the way (they break at once and are of no value).
pub fn clearable(name: &str) -> bool {
    let n = name.strip_prefix("minecraft:").unwrap_or(name);
    n.ends_with("_leaves")
        || n.ends_with("grass")
        || n.ends_with("fern")
        || n.ends_with("sapling")
        || n.ends_with("tulip")
        || n.ends_with("_roots")
        || matches!(
            n,
            "dead_bush"
                | "vine"
                | "snow"
                | "leaf_litter"
                | "poppy"
                | "dandelion"
                | "blue_orchid"
                | "allium"
                | "azure_bluet"
                | "oxeye_daisy"
                | "cornflower"
                | "lily_of_the_valley"
                | "sunflower"
                | "lilac"
                | "rose_bush"
                | "peony"
                | "pink_petals"
                | "wildflowers"
        )
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
    // Sand and gravel are mined with more of the same above them (a dune wall, a desert): the
    // block above only drops into the gap. For other ore a falling block above is a risk.
    if !block(at).falling && block([at[0], at[1] + 1, at[2]]).falling {
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
/// Why a search found nothing, for the status line: how many blocks of the kind were seen and
/// where each one was given up on.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct PlanStats {
    /// Blocks of the kind seen in the scanned cubes, buried ones included.
    pub seen: u32,
    /// Those with an open side: the ones that can be mined without digging.
    pub candidates: u32,
    /// No side of the block was safe to mine it from.
    pub unsafe_: u32,
    /// Safe, but nowhere to stand beside it.
    pub no_stand: u32,
    /// A place to stand, but no walking route to it.
    pub no_path: u32,
}
pub fn plan(
    start: nav::Pos,
    excluded: &[nav::Pos],
    block: impl FnMut(nav::Pos) -> Block,
    canceled: impl Fn() -> bool,
) -> Option<(nav::Pos, nav::Path)> {
    plan_with_stats(start, excluded, block, canceled, &mut PlanStats::default())
}
/// The nearest target that can be walked to, scanned outwards in growing cubes so that the
/// blocks beside the player are always looked at first and a slow scan still has something.
pub fn plan_with_stats(
    start: nav::Pos,
    excluded: &[nav::Pos],
    mut block: impl FnMut(nav::Pos) -> Block,
    canceled: impl Fn() -> bool,
    stats: &mut PlanStats,
) -> Option<(nav::Pos, nav::Path)> {
    let began = Instant::now();
    let mut candidates = Vec::new();
    let mut done = -1;
    'scan: for radius in [3, 6, 10, SCAN_RADIUS] {
        for y in -radius.min(12)..=radius.min(12) {
            for z in -radius..=radius {
                if canceled() {
                    return None;
                }
                if began.elapsed() > Duration::from_millis(400) {
                    break 'scan;
                }
                for x in -radius..=radius {
                    if x.abs().max(y.abs()).max(z.abs()) <= done {
                        continue;
                    }
                    let at = [start[0] + x, start[1] + y, start[2] + z];
                    if !block(at).ore || excluded.contains(&at) {
                        continue;
                    }
                    stats.seen += 1;
                    // A block with every side closed is dug to by `plan_buried`; the nearest
                    // ones are often the sand under the player's feet, which would crowd out
                    // the blocks that are open.
                    let open = [
                        [1, 0, 0],
                        [-1, 0, 0],
                        [0, 1, 0],
                        [0, -1, 0],
                        [0, 0, 1],
                        [0, 0, -1],
                    ]
                    .iter()
                    .any(|d| {
                        block([at[0] + d[0], at[1] + d[1], at[2] + d[2]]).cell == nav::Cell::Air
                    });
                    if open {
                        candidates.push((x * x + y * y + z * z, at));
                    }
                }
            }
        }
        done = radius;
        if candidates.len() >= 24 {
            break;
        }
    }
    stats.candidates = candidates.len() as u32;
    candidates.sort_unstable();
    candidates.truncate(32);
    for (_, target) in candidates {
        let (mut safe, mut stand) = (false, false);
        for [x, z] in [[1, 0], [-1, 0], [0, 1], [0, -1]] {
            // Standing below the block reaches up a trunk: logs several blocks above the ground.
            for y in [0, -1, 1, -2, -3, -4, -5] {
                if canceled() {
                    return None;
                }
                if began.elapsed() > Duration::from_millis(900) {
                    return None;
                }
                let at = [target[0] + x, target[1] + y, target[2] + z];
                if !safe_target(target, nav::center(at), &mut block) {
                    continue;
                }
                safe = true;
                if !nav::standable(at, &mut |p| block(p).cell) {
                    continue;
                }
                stand = true;
                if let Some(path) = nav::search(start, at, |p| block(p).cell, &canceled)
                    && path.complete
                {
                    return Some((target, path));
                }
            }
        }
        if !safe {
            stats.unsafe_ += 1;
        } else if !stand {
            stats.no_stand += 1;
        } else {
            stats.no_path += 1;
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
        // Digging down to sand or gravel goes through more of it; for other ore a falling block
        // in or above the way is left alone.
        let falling_target = read(target).falling;
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
                    && (falling_target || (!b.falling && !read([at[0], at[1] + 1, at[2]]).falling))
            },
            &canceled,
        );
        if let Some(path) = path.filter(|p| p.complete) {
            return Some((target, path));
        }
    }
    None
}
/// The next block to mine without a step: of the blocks within reach, the one nearest to the
/// block that was just broken (a vein, a column of sand, the next log), the nearest to the eye
/// among equals. `None` when there is none.
pub fn next_in_reach(
    last: Option<nav::Pos>,
    eye: [f64; 3],
    candidates: &[nav::Pos],
) -> Option<nav::Pos> {
    let anchor = last.map_or(eye, nav::center);
    candidates.iter().copied().min_by(|a, b| {
        let key = |at: &nav::Pos| {
            (
                nav::distance(nav::center(*at), anchor),
                nav::distance(nav::center(*at), eye),
            )
        };
        let (ka, kb) = (key(a), key(b));
        ka.0.total_cmp(&kb.0)
            .then(ka.1.total_cmp(&kb.1))
            .then(a.cmp(b))
    })
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Phase {
    Searching,
    Approaching,
    Breaking,
    Confirming,
    /// Walking to the items a broken block dropped, so they are picked up before moving on.
    Collecting,
    Done,
    NoTarget,
    Refused,
    Tool,
    Inventory,
    Timeout,
}
/// Why a mining job stands idle instead of stopping.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Wait {
    /// No suitable tool with durability to spare is carried.
    Tool,
    /// The inventory has no free slot for the drops.
    Inventory,
    /// On a trip to the storage container to empty the inventory.
    Storage,
}
#[derive(Clone, Debug)]
pub struct Job {
    pub ore: u8,
    /// Blocks to mine; 0 mines until stopped.
    pub limit: u16,
    pub confirmed: u16,
    pub active: bool,
    pub phase: Phase,
    pub target: Option<nav::Pos>,
    pub excluded: Vec<nav::Pos>,
    pub started_ms: u64,
    /// The block that was broken last: its drops are collected around it.
    pub last: Option<nav::Pos>,
    /// The current target is something in the way of the real one (grass, leaves): breaking it
    /// is not counted, and the search starts again afterwards.
    pub blocker: bool,
    /// How many times in a row the server refused the current block.
    pub refusals: u8,
    /// What the job is waiting for (shown as idle, not stopped).
    pub wait: Option<Wait>,
    /// How many such things were cleared for the current block.
    pub blockers: u8,
    /// The dropped item being walked to, items given up on, and when the player got there.
    pub collecting: Option<u32>,
    pub ignored: Vec<u32>,
    pub arrived_ms: Option<u64>,
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
        if usize::from(ore) >= COUNT && ore != ANY {
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
            last: None,
            blocker: false,
            refusals: 0,
            wait: None,
            blockers: 0,
            collecting: None,
            ignored: Vec::new(),
            arrived_ms: None,
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
    /// Aims at something that blocks the way to the real target; the real one is looked up again
    /// once it is out of the way.
    pub fn clear_blocker(&mut self, at: nav::Pos) {
        self.target = Some(at);
        self.blocker = true;
        self.blockers += 1;
        self.phase = Phase::Breaking;
        self.start_sequence = None;
        self.await_sequence = None;
    }
    pub fn skip(&mut self) {
        self.blocker = false;
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
    /// The server did not let the block go (or never answered): break it again, up to three
    /// times in a row, then leave an ore block alone; a dig route cannot go round it and stops.
    pub fn refuse(&mut self) {
        self.refusals += 1;
        self.start_sequence = None;
        self.await_sequence = None;
        if self.refusals < 3 {
            self.phase = Phase::Breaking;
        } else {
            self.refusals = 0;
            if self.ore == u8::MAX {
                self.active = false;
                self.phase = Phase::Refused;
            } else {
                self.skip();
            }
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
                self.refuse();
            }
            return;
        }
        self.refusals = 0;
        if self.blocker {
            // Cleared: not a mined block. Look for the real target again.
            self.blocker = false;
            self.target = None;
            self.start_sequence = None;
            self.await_sequence = None;
            self.phase = Phase::Searching;
            return;
        }
        self.blockers = 0;
        self.confirmed += 1;
        self.last = self.target;
        self.target = None;
        self.start_sequence = None;
        self.await_sequence = None;
        self.collecting = None;
        self.arrived_ms = None;
        self.phase = if self.limit > 0 && self.confirmed >= self.limit {
            self.active = false;
            Phase::Done
        } else if self.ore == u8::MAX {
            Phase::Searching
        } else {
            Phase::Collecting
        };
    }
    pub fn resume(&mut self) {
        if self.limit > 0 && self.confirmed >= self.limit {
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
        // A first refusal is tried again; three in a row leave an ore block alone.
        assert_eq!(
            (job.phase, job.active, job.refusals),
            (Phase::Breaking, true, 1)
        );
        for n in 0..2 {
            job.sent([3, 0, 0], 3, 20 + n);
            job.acknowledge(20 + n, true);
        }
        assert_eq!(job.phase, Phase::Searching);
        assert!(job.active && job.excluded.contains(&[3, 0, 0]));
        assert_eq!(job.confirmed, 1);
        // A dig route (a tunnel) stops after three refusals: it cannot go round the block.
        let mut dig = Job::excavation([4, 0, 0]);
        for n in 0..3 {
            dig.sent([4, 0, 0], 3, 30 + n);
            dig.acknowledge(30 + n, true);
        }
        assert_eq!(dig.phase, Phase::Refused);
        assert!(!dig.active);
    }
    /// A desert: sand to y = -1 (every sand block has more sand or air above it) and a dune
    /// wall of sand three blocks high at x = 4.
    fn desert(at: nav::Pos) -> Block {
        let sand = Block {
            cell: nav::Cell::Solid,
            ore: true,
            falling: true,
            breakable: true,
        };
        if at[1] < 0 || (at[0] == 4 && (0..3).contains(&at[1])) {
            sand
        } else {
            Block {
                cell: nav::Cell::Air,
                ..Block::default()
            }
        }
    }
    #[test]
    fn sand_is_found_in_a_desert_even_with_sand_stacked_above_it() {
        let mut stats = PlanStats::default();
        let (target, path) =
            plan_with_stats([0, 0, 0], &[], desert, || false, &mut stats).expect("sand to mine");
        assert!(path.complete);
        assert!(stats.candidates > 0);
        // The nearest block is mined, not one far away.
        let d = target[0] * target[0] + target[1] * target[1] + target[2] * target[2];
        assert!(d <= 4, "{target:?}");
        // The dune wall: only its blocks count, and they are still reachable.
        let wall_only = |at: nav::Pos| {
            let mut b = desert(at);
            b.ore = at[0] == 4 && (0..3).contains(&at[1]);
            b
        };
        let (target, _) = plan([0, 0, 0], &[], wall_only, || false).expect("a wall block");
        assert_eq!(target[0], 4);
        // Other ore with a falling block above it is still left alone.
        let risky = |at: nav::Pos| {
            let mut b = desert(at);
            b.ore = at == [4, 0, 0];
            b.falling = at != [4, 0, 0];
            b
        };
        let mut stats = PlanStats::default();
        assert!(plan_with_stats([0, 0, 0], &[], risky, || false, &mut stats).is_none());
        assert_eq!((stats.candidates, stats.unsafe_), (1, 1));
    }
    /// A ridge: one sand column to y = -1 in the middle of pits whose floor is at y = -4.
    fn ridge(at: nav::Pos) -> Block {
        let sand = Block {
            cell: nav::Cell::Solid,
            ore: true,
            falling: true,
            breakable: true,
        };
        if at[1] <= -4 || (at[0] == 0 && at[2] == 0 && at[1] <= -1) {
            sand
        } else {
            Block {
                cell: nav::Cell::Air,
                ..Block::default()
            }
        }
    }
    #[test]
    fn the_player_walks_down_a_three_block_drop_to_reach_the_sand() {
        // On the ridge every neighbour is three blocks lower: a walk steps off the edge.
        let (target, path) =
            plan([0, 0, 0], &[], ridge, || false).expect("a walk down into the pit");
        assert!(ridge(target).ore && path.complete);
        assert!(path.nodes.iter().any(|n| n[1] <= -3));
    }
    #[test]
    fn the_next_block_is_the_one_beside_the_last_break() {
        let eye = [0.5, 1.6, 0.5];
        let near_eye = [1, 0, 0];
        let beside_last = [3, 0, 1];
        assert_eq!(
            next_in_reach(Some([3, 0, 2]), eye, &[near_eye, beside_last]),
            Some(beside_last)
        );
        // With no block broken yet the nearest to the eye is taken.
        assert_eq!(
            next_in_reach(None, eye, &[beside_last, near_eye]),
            Some(near_eye)
        );
        assert_eq!(next_in_reach(None, eye, &[]), None);
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
        // No limit is allowed now: 0 mines until stopped.
        assert!(Job::new(0, 0).is_some());
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
    #[test]
    fn every_target_has_an_icon_item() {
        for i in 0..COUNT as u8 {
            let item = icon(i).unwrap();
            assert!(item.starts_with("minecraft:"), "{i}: {item}");
            // The icon is an item that the target actually yields or is made of.
            let (_, _, blocks) = target(i).unwrap();
            assert!(!blocks.is_empty());
        }
        assert_eq!(icon(COUNT as u8), None);
    }
    #[test]
    fn an_unlimited_job_collects_drops_after_each_block_and_never_finishes() {
        let mut job = Job::new(0, 0).unwrap();
        for n in 1..=100u16 {
            job.select([2, 0, 0]);
            job.phase = Phase::Breaking;
            job.sent([2, 0, 0], 0, i32::from(n));
            job.sent([2, 0, 0], 3, i32::from(n));
            job.acknowledge(i32::from(n), false);
            assert_eq!(job.confirmed, n);
            assert!(job.active);
            // The drops are picked up before the next block is looked for.
            assert_eq!(job.phase, Phase::Collecting);
            assert_eq!(job.last, Some([2, 0, 0]));
            job.phase = Phase::Searching;
        }
    }
    #[test]
    fn grass_and_leaves_in_the_way_are_cleared_without_counting() {
        assert!(clearable("minecraft:short_grass") && clearable("minecraft:oak_leaves"));
        assert!(clearable("minecraft:fern") && clearable("minecraft:poppy"));
        assert!(!clearable("minecraft:grass_block") && !clearable("minecraft:oak_log"));
        assert!(!clearable("minecraft:torch") && !clearable("minecraft:wheat"));
        let mut job = Job::new(ANY, 0).unwrap();
        job.select([4, 0, 0]);
        job.phase = Phase::Breaking;
        job.clear_blocker([3, 0, 0]);
        assert_eq!(
            (job.blockers, job.target, job.blocker),
            (1, Some([3, 0, 0]), true)
        );
        job.sent([3, 0, 0], 0, 1);
        job.sent([3, 0, 0], 3, 2);
        job.acknowledge(2, false);
        // Nothing was mined; the search starts over for the real target.
        assert_eq!(
            (job.confirmed, job.phase, job.target),
            (0, Phase::Searching, None)
        );
        assert!(!job.blocker && job.active);
        // A real block resets the count of blockers.
        job.blockers = 3;
        job.select([4, 0, 0]);
        job.phase = Phase::Breaking;
        job.sent([4, 0, 0], 0, 3);
        job.sent([4, 0, 0], 3, 4);
        job.acknowledge(4, false);
        assert_eq!((job.confirmed, job.blockers), (1, 0));
    }
}
