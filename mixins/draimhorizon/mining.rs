// Bounded ore mining jobs. Only server-confirmed block changes advance the count.
use super::{claim_borders_experimental::ORES, claim_borders_navigation as nav};
use std::time::{Duration, Instant};
pub const SCAN_RADIUS: i32 = 16;
#[derive(Clone, Copy, Debug)]
pub struct Block {
    pub cell: nav::Cell,
    pub ore: bool,
    pub falling: bool,
}
impl Default for Block {
    fn default() -> Self {
        Self {
            cell: nav::Cell::Unknown,
            ore: false,
            falling: false,
        }
    }
}
pub fn matches(ore: u8, name: &str) -> bool {
    ORES.get(usize::from(ore))
        .is_some_and(|o| o.blocks.contains(&name))
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
    pub fn new(ore: u8, limit: u16) -> Option<Self> {
        if usize::from(ore) >= ORES.len() || !(1..=64).contains(&limit) {
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
        let awaited = if self.phase == Phase::Confirming {
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
            if self.phase == Phase::Confirming {
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
        if self.phase != Phase::Confirming {
            self.phase = Phase::Searching;
            self.target = None;
            self.start_sequence = None;
            self.await_sequence = None;
        }
    }
}
#[cfg(test)]
mod tests {
    use super::*;
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
        assert!(Job::new(10, 1).is_none());
        assert!(Job::new(0, 0).is_none());
        assert!(Job::new(0, 65).is_none());
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
                }
            } else {
                block(p)
            }
        ));
    }
}
