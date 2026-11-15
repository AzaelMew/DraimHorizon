// A port of Litematica's `SchematicVerifier` (LGPL-3.0, see NOTICE.md): the comparison of what a
// schematic wants with what stands in the world, sorted into the mismatch types of the original,
// with a count per (wanted, found) pair, the positions to highlight nearest first, and the
// ignored pairs. Pure: a state is a `u32` the caller chose, and what the original asks the game's
// block objects (air, fluid, same block) is asked through `States`.
use std::collections::{HashMap, HashSet};

pub type Pos = [i32; 3];
pub type State = u32;

/// `SchematicVerifier.MismatchType` (without `ALL`, which is "every type" and not a type).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Mismatch {
    /// The schematic has a block and the world has air.
    Missing,
    /// The world has a block where the schematic has air.
    Extra,
    /// Another block stands where a block is wanted.
    WrongBlock,
    /// The right block in another state.
    WrongState,
    /// Another block of the same group with the same state (a different wood, say).
    DiffBlock,
}
impl Mismatch {
    pub const ALL: [Mismatch; 5] = [
        Mismatch::Missing,
        Mismatch::Extra,
        Mismatch::WrongBlock,
        Mismatch::WrongState,
        Mismatch::DiffBlock,
    ];
    /// The colour the original draws the type in (`0xRRGGBB`).
    pub fn color(self) -> u32 {
        match self {
            Mismatch::Missing => 0x00ffff,
            Mismatch::Extra => 0xff00cf,
            Mismatch::WrongBlock => 0xff0000,
            Mismatch::WrongState => 0xffaf00,
            Mismatch::DiffBlock => 0xfaf000,
        }
    }
}

/// What the original asks the block objects.
pub trait States {
    fn is_air(&self, state: State) -> bool;
    /// Whether two states are states of one block.
    fn same_block(&self, a: State, b: State) -> bool;
    fn is_liquid(&self, state: State) -> bool;
    /// `BlockUtils.isInSameGroup` (the original's "different blocks" option): another block that
    /// stands for the same thing. Off by default in the original too.
    fn same_group(&self, _a: State, _b: State) -> bool {
        false
    }
}

/// `Configs` the check reads.
#[derive(Clone, Copy, Debug, Default)]
pub struct Options {
    /// `ignoreExistingFluids`: water and lava standing where the schematic wants air are not extra.
    pub ignore_fluids: bool,
    /// `enableDifferentBlocks`.
    pub different_blocks: bool,
}

/// `BlockMismatch`: one (wanted, found) pair and how many times it was seen.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct BlockMismatch {
    pub kind: Mismatch,
    pub expected: State,
    pub found: State,
    pub count: usize,
}

/// `SortCriteria`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Sort {
    NameExpected,
    NameFound,
    Count,
}

#[derive(Default)]
pub struct Verifier {
    positions: HashMap<Mismatch, HashMap<(State, State), Vec<Pos>>>,
    ignored: HashSet<(State, State)>,
    /// `correctStatesCount`: cells of the schematic that are right.
    pub correct: usize,
    /// `schematicBlocks` and `clientBlocks`: cells that are not air, in each.
    pub schematic_blocks: usize,
    pub world_blocks: usize,
    pub options: Options,
}

impl Verifier {
    pub fn new(options: Options) -> Self {
        Self {
            options,
            ..Self::default()
        }
    }

    /// `checkBlockStates` and the counting of `verifyChunk` for one cell: the mismatch type it
    /// falls under, or `None` when it is right (or ignored).
    pub fn check(
        &mut self,
        at: Pos,
        wanted: State,
        found: State,
        states: &impl States,
    ) -> Option<Mismatch> {
        if !states.is_air(wanted) {
            self.schematic_blocks += 1;
        }
        if !states.is_air(found) {
            self.world_blocks += 1;
        }
        if wanted == found || (states.is_air(wanted) && states.is_air(found)) {
            if !states.is_air(wanted) {
                self.correct += 1;
            }
            return None;
        }
        if self.ignored.contains(&(wanted, found)) {
            return None;
        }
        let kind = if !states.is_air(wanted) {
            if states.is_air(found) {
                Mismatch::Missing
            } else if !states.same_block(wanted, found) {
                if self.options.different_blocks && states.same_group(wanted, found) {
                    Mismatch::DiffBlock
                } else {
                    Mismatch::WrongBlock
                }
            } else {
                Mismatch::WrongState
            }
        } else if self.options.ignore_fluids && states.is_liquid(found) {
            return None;
        } else {
            Mismatch::Extra
        };
        self.positions
            .entry(kind)
            .or_default()
            .entry((wanted, found))
            .or_default()
            .push(at);
        Some(kind)
    }

    /// `ignoreStateMismatch`: this pair is not reported any more, and what was reported of it is
    /// forgotten.
    pub fn ignore(&mut self, expected: State, found: State) {
        self.ignored.insert((expected, found));
        for map in self.positions.values_mut() {
            map.remove(&(expected, found));
        }
    }
    pub fn clear_ignored(&mut self) {
        self.ignored.clear();
    }

    /// How many cells fall under `kind` (`getMismatchOverviewFor` summed up).
    pub fn count(&self, kind: Mismatch) -> usize {
        self.positions
            .get(&kind)
            .map_or(0, |m| m.values().map(Vec::len).sum())
    }
    /// All the mismatches.
    pub fn total(&self) -> usize {
        Mismatch::ALL.iter().map(|k| self.count(*k)).sum()
    }

    /// `getMismatchOverviewFor`: the pairs of a type (every type for `None`), sorted. `name` is
    /// the display name of a state, which the name sorts compare.
    pub fn overview(
        &self,
        kind: Option<Mismatch>,
        sort: Sort,
        reverse: bool,
        name: impl Fn(State) -> String,
    ) -> Vec<BlockMismatch> {
        let mut list = Vec::new();
        for k in Mismatch::ALL {
            if kind.is_some_and(|only| only != k) {
                continue;
            }
            if let Some(map) = self.positions.get(&k) {
                for ((expected, found), at) in map {
                    list.push(BlockMismatch {
                        kind: k,
                        expected: *expected,
                        found: *found,
                        count: at.len(),
                    });
                }
            }
        }
        // `VerifierResultSorter`.
        list.sort_by(|a, b| {
            let order = match sort {
                Sort::Count => b
                    .count
                    .cmp(&a.count)
                    .then_with(|| name(a.expected).cmp(&name(b.expected))),
                Sort::NameExpected => name(a.expected)
                    .cmp(&name(b.expected))
                    .then_with(|| name(a.found).cmp(&name(b.found))),
                Sort::NameFound => name(a.found)
                    .cmp(&name(b.found))
                    .then_with(|| name(a.expected).cmp(&name(b.expected))),
            };
            // The count sort runs big to small by default; the others small to big.
            if reverse { order.reverse() } else { order }
        });
        list
    }

    /// `getClosestMismatchedPositionsFor` over the types asked: the positions nearest to
    /// `center` first, at most `max`. `pairs` limits it to chosen (wanted, found) pairs
    /// (`selectedEntries`); empty means every pair.
    pub fn closest(
        &self,
        kinds: &[Mismatch],
        pairs: &[(State, State)],
        center: Pos,
        max: usize,
    ) -> Vec<(Mismatch, Pos)> {
        let mut out: Vec<(Mismatch, Pos)> = Vec::new();
        for kind in kinds {
            if let Some(map) = self.positions.get(kind) {
                for (pair, at) in map {
                    if pairs.is_empty() || pairs.contains(pair) {
                        out.extend(at.iter().map(|p| (*kind, *p)));
                    }
                }
            }
        }
        let distance = |p: &Pos| -> i64 {
            (0..3)
                .map(|i| i64::from(p[i] - center[i]).pow(2))
                .sum::<i64>()
        };
        out.sort_by_key(|(_, p)| (distance(p), *p));
        out.truncate(max);
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // 0 air, 1 stone, 2 oak stairs (one block, states 2 and 3), 4 water, 5 dirt.
    struct Blocks;
    impl States for Blocks {
        fn is_air(&self, s: State) -> bool {
            s == 0
        }
        fn same_block(&self, a: State, b: State) -> bool {
            a == b || (a == 2 && b == 3) || (a == 3 && b == 2)
        }
        fn is_liquid(&self, s: State) -> bool {
            s == 4
        }
    }

    #[test]
    fn every_cell_falls_under_the_type_of_the_original() {
        let mut v = Verifier::new(Options::default());
        let b = Blocks;
        assert_eq!(v.check([0, 0, 0], 1, 1, &b), None);
        assert_eq!(v.check([1, 0, 0], 1, 0, &b), Some(Mismatch::Missing));
        assert_eq!(v.check([2, 0, 0], 1, 5, &b), Some(Mismatch::WrongBlock));
        assert_eq!(v.check([3, 0, 0], 2, 3, &b), Some(Mismatch::WrongState));
        assert_eq!(v.check([4, 0, 0], 0, 5, &b), Some(Mismatch::Extra));
        assert_eq!(v.check([5, 0, 0], 0, 0, &b), None);
        assert_eq!((v.correct, v.schematic_blocks, v.world_blocks), (1, 4, 4));
        assert_eq!(v.total(), 4);
        // Water where the schematic wants air is extra only when fluids are not ignored.
        assert_eq!(v.check([6, 0, 0], 0, 4, &b), Some(Mismatch::Extra));
        let mut quiet = Verifier::new(Options {
            ignore_fluids: true,
            ..Options::default()
        });
        assert_eq!(quiet.check([0, 0, 0], 0, 4, &b), None);
    }

    #[test]
    fn an_ignored_pair_is_forgotten_and_not_reported_again() {
        let mut v = Verifier::new(Options::default());
        let b = Blocks;
        v.check([0, 0, 0], 1, 5, &b);
        v.check([1, 0, 0], 1, 5, &b);
        v.check([2, 0, 0], 1, 0, &b);
        assert_eq!(v.count(Mismatch::WrongBlock), 2);
        v.ignore(1, 5);
        assert_eq!(v.count(Mismatch::WrongBlock), 0);
        assert_eq!(v.check([3, 0, 0], 1, 5, &b), None);
        assert_eq!(v.total(), 1);
    }

    #[test]
    fn the_overview_sorts_and_the_closest_positions_come_nearest_first() {
        let mut v = Verifier::new(Options::default());
        let b = Blocks;
        for x in 0..3 {
            v.check([x, 0, 0], 1, 5, &b);
        }
        v.check([9, 0, 0], 1, 0, &b);
        let name = |s: State| format!("s{s}");
        let by_count = v.overview(None, Sort::Count, false, name);
        assert_eq!(by_count[0].count, 3);
        assert_eq!((by_count[0].expected, by_count[0].found), (1, 5));
        let by_found = v.overview(None, Sort::NameFound, false, name);
        assert_eq!(by_found[0].found, 0);
        assert!(
            v.overview(Some(Mismatch::Extra), Sort::Count, false, name)
                .is_empty()
        );
        let near = v.closest(&Mismatch::ALL, &[], [8, 0, 0], 2);
        assert_eq!(near[0], (Mismatch::Missing, [9, 0, 0]));
        assert_eq!(near.len(), 2);
        let only = v.closest(&[Mismatch::WrongBlock], &[(1, 5)], [0, 0, 0], 10);
        assert_eq!(only.len(), 3);
        assert!(
            v.closest(&[Mismatch::WrongBlock], &[(7, 7)], [0, 0, 0], 10)
                .is_empty()
        );
    }
}
