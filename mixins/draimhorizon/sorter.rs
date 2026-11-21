// The storage sorter: like a copper golem, it takes items out of an input container and carries
// each to a container of the storage that is meant for it. Pure data and plans here; walking,
// clicks and item frames are in `sorter_view`.
//
// A container is meant for an item when an item frame on it shows that item, else when it is
// known to hold that item already (seen the last time it was open), else the overflow container
// takes it; with "fill empty" on, an empty container without a frame becomes the item's. Items
// with nowhere to go stay in the input.
use std::collections::{BTreeMap, BTreeSet};

pub type Pos = [i32; 3];
/// An item id and how many of it a slot holds; `None` for an empty slot.
pub type Stack = Option<(String, u32)>;
/// Containers kept in the index, over all worlds.
pub const MAX_CHESTS: usize = 2048;
/// Item ids kept per container (labels or contents): a double chest has 54 slots.
pub const MAX_ITEMS: usize = 54;
/// Input containers per world.
pub const MAX_INPUTS: usize = 4;
/// The largest side of the storage zone, and its most levels.
pub const MAX_SIDE: i32 = 128;
pub const MAX_HEIGHT: i32 = 24;
/// Stops of one carrying round at most.
pub const ROUTE_MAX: usize = 64;

pub use super::claim_borders_farm_storage::{is_container, is_copper_chest};
/// Chests that join with a neighbour into a double chest.
pub fn joins(name: &str) -> bool {
    matches!(name, "minecraft:chest" | "minecraft:trapped_chest") || is_copper_chest(name)
}
/// The other half of a double chest from the block's `type` and `facing` properties
/// (`ChestBlock.getConnectedDirection`: left turns clockwise from the facing, right counter).
pub fn partner(at: Pos, kind: Option<&str>, facing: Option<&str>) -> Option<Pos> {
    let clockwise = match kind? {
        "left" => true,
        "right" => false,
        _ => return None,
    };
    // Clockwise seen from above: north, east, south, west.
    let order = ["north", "east", "south", "west"];
    let i = order.iter().position(|f| Some(*f) == facing)?;
    let side = order[if clockwise { (i + 1) % 4 } else { (i + 3) % 4 }];
    let [dx, dz] = match side {
        "north" => [0, -1],
        "east" => [1, 0],
        "south" => [0, 1],
        _ => [-1, 0],
    };
    Some([at[0] + dx, at[1], at[2] + dz])
}
/// One position for both halves of a double chest: the smaller one.
pub fn canonical(at: Pos, partner: Option<Pos>) -> Pos {
    partner.map_or(at, |p| p.min(at))
}

/// The storage zone: a box marked by two corners, corners included.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Zone {
    pub lo: Pos,
    pub hi: Pos,
}
impl Zone {
    pub fn between(a: Pos, b: Pos) -> Self {
        Self {
            lo: [a[0].min(b[0]), a[1].min(b[1]), a[2].min(b[2])],
            hi: [a[0].max(b[0]), a[1].max(b[1]), a[2].max(b[2])],
        }
    }
    pub fn contains(&self, at: Pos) -> bool {
        (0..3).all(|i| self.lo[i] <= at[i] && at[i] <= self.hi[i])
    }
    /// Why the zone is too large to scan ("side" or "height"), if it is.
    pub fn too_large(&self) -> Option<&'static str> {
        if self.hi[0] - self.lo[0] >= MAX_SIDE || self.hi[2] - self.lo[2] >= MAX_SIDE {
            Some("side")
        } else if self.hi[1] - self.lo[1] >= MAX_HEIGHT {
            Some("height")
        } else {
            None
        }
    }
    pub fn volume(&self) -> i64 {
        (0..3)
            .map(|i| i64::from(self.hi[i] - self.lo[i] + 1))
            .product()
    }
    /// The block at `index` of the zone, row by row (x, then z, then y); `None` past the end.
    pub fn nth(&self, index: i64) -> Option<Pos> {
        let w = i64::from(self.hi[0] - self.lo[0] + 1);
        let d = i64::from(self.hi[2] - self.lo[2] + 1);
        if index < 0 || index >= self.volume() {
            return None;
        }
        let x = index % w;
        let z = (index / w) % d;
        let y = index / (w * d);
        Some([
            self.lo[0] + x as i32,
            self.lo[1] + y as i32,
            self.lo[2] + z as i32,
        ])
    }
}

/// One container of the storage: where it is (a double chest by its canonical half), the items
/// its frames show, what it held when last open and whether it was full then.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Chest {
    pub scope: String,
    pub pos: Pos,
    pub labels: BTreeSet<String>,
    pub contents: BTreeSet<String>,
    /// Opened at least once: `contents` is known (empty means empty).
    pub seen: bool,
    pub full: bool,
}
/// Where the sorter works in one world: the zone, the inputs and the overflow container.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Places {
    pub zone: Option<Zone>,
    pub inputs: Vec<Pos>,
    pub overflow: Option<Pos>,
}

/// What the rules pick a container from.
pub struct Rules<'a> {
    pub chests: &'a [Chest],
    pub places: &'a Places,
    pub fill_empty: bool,
    /// Containers given up on in this run (they refused items or would not open).
    pub skip: &'a BTreeSet<Pos>,
}
fn nearest<'a>(found: impl Iterator<Item = &'a Chest>, here: Pos) -> Option<Pos> {
    found.map(|c| c.pos).min_by_key(|p| {
        let d = |i: usize| i64::from(p[i] - here[i]);
        (d(0) * d(0) + d(1) * d(1) + d(2) * d(2), *p)
    })
}
impl Rules<'_> {
    fn usable(&self, c: &Chest) -> bool {
        !c.full
            && !self.skip.contains(&c.pos)
            && !self.places.inputs.contains(&c.pos)
            && Some(c.pos) != self.places.overflow
            && self.places.zone.is_none_or(|z| z.contains(c.pos))
    }
    /// The container `item` goes to from `here`: framed for it, holding it, the overflow, or an
    /// empty unframed one; the nearest of the first rule that finds any.
    pub fn choose(&self, item: &str, here: Pos) -> Option<Pos> {
        let usable = |c: &&Chest| self.usable(c);
        if let Some(at) = nearest(
            self.chests
                .iter()
                .filter(usable)
                .filter(|c| c.labels.contains(item)),
            here,
        ) {
            return Some(at);
        }
        // A framed container keeps to its frames: another item never goes in by contents.
        if let Some(at) = nearest(
            self.chests
                .iter()
                .filter(usable)
                .filter(|c| c.labels.is_empty() && c.contents.contains(item)),
            here,
        ) {
            return Some(at);
        }
        if let Some(at) = self.places.overflow
            && !self.skip.contains(&at)
            && !self.chests.iter().any(|c| c.pos == at && c.full)
        {
            return Some(at);
        }
        if self.fill_empty {
            return nearest(
                self.chests
                    .iter()
                    .filter(usable)
                    .filter(|c| c.labels.is_empty() && c.seen && c.contents.is_empty()),
                here,
            );
        }
        None
    }
}

/// The input's slots to take: those whose item has a container to go to, one inventory slot a
/// stack, at most `free` of them.
pub fn take_plan(input: &[Stack], free: usize, has_place: &dyn Fn(&str) -> bool) -> Vec<usize> {
    input
        .iter()
        .enumerate()
        .filter_map(|(slot, stack)| {
            let (name, _) = stack.as_ref()?;
            has_place(name).then_some(slot)
        })
        .take(free)
        .collect()
}
pub fn free_slots(inventory: &[Stack]) -> usize {
    inventory.iter().filter(|s| s.is_none()).count()
}
pub fn count(inventory: &[Stack], item: &str) -> u32 {
    inventory
        .iter()
        .flatten()
        .filter(|(name, _)| name == item)
        .map(|(_, n)| n)
        .sum()
}
/// How many of each item the inventory holds: the player's own, never carried off.
pub fn baseline(inventory: &[Stack]) -> BTreeMap<String, u32> {
    let mut out = BTreeMap::new();
    for (name, n) in inventory.iter().flatten() {
        *out.entry(name.clone()).or_insert(0) += n;
    }
    out
}
/// The inventory slots to put into a container meant for `items`: every stack of them while what
/// stays still covers the player's own count (`keep`), the largest stacks first (fewer clicks, and the player's own stack stays).
pub fn put_slots(
    inventory: &[Stack],
    items: &BTreeSet<String>,
    keep: &BTreeMap<String, u32>,
) -> Vec<usize> {
    let mut out = Vec::new();
    for item in items {
        let own = keep.get(item).copied().unwrap_or(0);
        let mut stacks: Vec<(u32, usize)> = inventory
            .iter()
            .enumerate()
            .filter_map(|(slot, s)| match s {
                Some((name, n)) if name == item => Some((*n, slot)),
                _ => None,
            })
            .collect();
        stacks.sort_unstable_by(|a, b| b.cmp(a));
        let mut left = stacks.iter().map(|(n, _)| n).sum::<u32>();
        for (n, slot) in stacks {
            if left.saturating_sub(n) < own {
                continue;
            }
            left -= n;
            out.push(slot);
        }
    }
    out.sort_unstable();
    out
}
/// The items carried beyond the player's own counts.
pub fn carried(inventory: &[Stack], keep: &BTreeMap<String, u32>) -> BTreeSet<String> {
    baseline(inventory)
        .into_iter()
        .filter(|(name, n)| *n > keep.get(name).copied().unwrap_or(0))
        .map(|(name, _)| name)
        .collect()
}
/// The order to visit `stops` in from `from`: always the nearest left, at most `ROUTE_MAX`.
pub fn route(from: Pos, stops: &[Pos]) -> Vec<Pos> {
    let mut left: Vec<Pos> = stops.to_vec();
    left.sort_unstable();
    left.dedup();
    let mut here = from;
    let mut out = Vec::new();
    while !left.is_empty() && out.len() < ROUTE_MAX {
        let d = |p: &Pos| {
            let d = |i: usize| i64::from(p[i] - here[i]);
            d(0) * d(0) + d(1) * d(1) * 4 + d(2) * d(2)
        };
        let (i, _) = left
            .iter()
            .enumerate()
            .min_by_key(|(_, p)| (d(p), **p))
            .expect("not empty");
        here = left.swap_remove(i);
        out.push(here);
    }
    out
}

/// The item ids a container window shows (its own slots, not the player's), bounded.
pub fn contents(slots: &[Stack]) -> BTreeSet<String> {
    slots
        .iter()
        .flatten()
        .map(|(name, _)| name.clone())
        .take(MAX_ITEMS)
        .collect()
}
/// Whether no slot of the window is empty.
pub fn is_full(slots: &[Stack]) -> bool {
    !slots.is_empty() && slots.iter().all(Option::is_some)
}
/// The container of `scope` at `pos`, added when it is not known yet (dropping the oldest when the
/// index is full).
pub fn entry<'a>(list: &'a mut Vec<Chest>, scope: &str, pos: Pos) -> &'a mut Chest {
    if let Some(i) = list.iter().position(|c| c.scope == scope && c.pos == pos) {
        return &mut list[i];
    }
    if list.len() >= MAX_CHESTS {
        list.remove(0);
    }
    list.push(Chest {
        scope: scope.to_owned(),
        pos,
        ..Chest::default()
    });
    list.last_mut().expect("just pushed")
}
/// What a window showed when it was open.
pub fn observe(list: &mut Vec<Chest>, scope: &str, pos: Pos, slots: &[Stack]) {
    let c = entry(list, scope, pos);
    c.contents = contents(slots);
    c.full = is_full(slots);
    c.seen = true;
}
/// The frames seen around the player: every known container of `scope` that `near` covers takes
/// the labels seen on it now (none when its frames are gone); containers with a frame are added.
/// True when anything changed.
pub fn apply_labels(
    list: &mut Vec<Chest>,
    scope: &str,
    seen: &BTreeMap<Pos, BTreeSet<String>>,
    near: &dyn Fn(Pos) -> bool,
) -> bool {
    let mut changed = false;
    for c in list.iter_mut().filter(|c| c.scope == scope && near(c.pos)) {
        let now = seen.get(&c.pos).cloned().unwrap_or_default();
        if c.labels != now {
            c.labels = now;
            changed = true;
        }
    }
    for (pos, labels) in seen {
        if !list.iter().any(|c| c.scope == scope && c.pos == *pos) {
            let c = entry(list, scope, *pos);
            c.labels = labels.iter().take(MAX_ITEMS).cloned().collect();
            changed = true;
        }
    }
    changed
}

fn clean(text: &str) -> String {
    text.chars()
        .filter(|c| !matches!(c, '\t' | '\n' | '\r'))
        .collect()
}
fn items(set: &BTreeSet<String>) -> String {
    set.iter()
        .map(|s| s.replace([',', '\t', '\n', '\r'], ""))
        .collect::<Vec<_>>()
        .join(",")
}
fn read_items(text: &str) -> BTreeSet<String> {
    text.split(',')
        .filter(|s| !s.is_empty())
        .take(MAX_ITEMS)
        .map(str::to_owned)
        .collect()
}
fn num(s: &str) -> Option<i32> {
    s.parse().ok()
}
/// One container a line: world, x, y, z, labels, contents, seen, full; tab-separated.
pub fn encode_index(list: &[Chest]) -> String {
    let skip = list.len().saturating_sub(MAX_CHESTS);
    list[skip..]
        .iter()
        .map(|c| {
            let [x, y, z] = c.pos;
            format!(
                "{}\t{x}\t{y}\t{z}\t{}\t{}\t{}\t{}",
                clean(&c.scope),
                items(&c.labels),
                items(&c.contents),
                u8::from(c.seen),
                u8::from(c.full)
            )
        })
        .collect::<Vec<_>>()
        .join("\n")
}
/// The containers kept by `encode_index`; lines that do not read are left out and a container
/// appearing twice keeps its last line.
pub fn decode_index(text: &str) -> Vec<Chest> {
    let mut out: Vec<Chest> = Vec::new();
    for line in text.lines() {
        let parts: Vec<&str> = line.split('\t').collect();
        let [scope, x, y, z, labels, contents, seen, full] = parts[..] else {
            continue;
        };
        let (Some(x), Some(y), Some(z)) = (num(x), num(y), num(z)) else {
            continue;
        };
        out.retain(|c| !(c.scope == scope && c.pos == [x, y, z]));
        out.push(Chest {
            scope: scope.to_owned(),
            pos: [x, y, z],
            labels: read_items(labels),
            contents: read_items(contents),
            seen: seen == "1",
            full: full == "1",
        });
    }
    let extra = out.len().saturating_sub(MAX_CHESTS);
    out.drain(..extra);
    out
}
/// The places of every world: a line per zone, input or overflow container.
pub fn encode_places(all: &BTreeMap<String, Places>) -> String {
    let mut lines = Vec::new();
    for (scope, p) in all {
        let scope = clean(scope);
        if let Some(z) = p.zone {
            let ([a, b, c], [d, e, f]) = (z.lo, z.hi);
            lines.push(format!("{scope}\tzone\t{a}\t{b}\t{c}\t{d}\t{e}\t{f}"));
        }
        for [x, y, z] in p.inputs.iter().take(MAX_INPUTS) {
            lines.push(format!("{scope}\tinput\t{x}\t{y}\t{z}"));
        }
        if let Some([x, y, z]) = p.overflow {
            lines.push(format!("{scope}\toverflow\t{x}\t{y}\t{z}"));
        }
    }
    lines.join("\n")
}
pub fn decode_places(text: &str) -> BTreeMap<String, Places> {
    let mut out: BTreeMap<String, Places> = BTreeMap::new();
    for line in text.lines() {
        let parts: Vec<&str> = line.split('\t').collect();
        let (Some(scope), Some(kind)) = (parts.first(), parts.get(1)) else {
            continue;
        };
        let n: Vec<i32> = parts[2..].iter().filter_map(|s| num(s)).collect();
        if n.len() != parts.len() - 2 {
            continue;
        }
        let p = out.entry((*scope).to_owned()).or_default();
        match (*kind, &n[..]) {
            ("zone", &[a, b, c, d, e, f]) => {
                let zone = Zone::between([a, b, c], [d, e, f]);
                if zone.too_large().is_none() {
                    p.zone = Some(zone);
                }
            }
            ("input", &[x, y, z])
                if p.inputs.len() < MAX_INPUTS && !p.inputs.contains(&[x, y, z]) =>
            {
                p.inputs.push([x, y, z]);
            }
            ("overflow", &[x, y, z]) => p.overflow = Some([x, y, z]),
            _ => {}
        }
    }
    out.retain(|_, p| *p != Places::default());
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    fn set(items: &[&str]) -> BTreeSet<String> {
        items.iter().map(|s| (*s).to_owned()).collect()
    }
    fn chest(pos: Pos, labels: &[&str], contents: &[&str]) -> Chest {
        Chest {
            scope: "w".into(),
            pos,
            labels: set(labels),
            contents: set(contents),
            seen: true,
            full: false,
        }
    }
    fn stack(name: &str, n: u32) -> Stack {
        Some((name.to_owned(), n))
    }

    #[test]
    fn containers_include_every_copper_chest_and_only_chests_join() {
        for name in [
            "minecraft:copper_chest",
            "minecraft:exposed_copper_chest",
            "minecraft:weathered_copper_chest",
            "minecraft:oxidized_copper_chest",
            "minecraft:waxed_oxidized_copper_chest",
            "minecraft:chest",
            "minecraft:barrel",
            "minecraft:red_shulker_box",
        ] {
            assert!(is_container(name), "{name}");
        }
        assert!(!is_container("minecraft:copper_block"));
        assert!(!is_container("minecraft:ender_chest"));
        assert!(joins("minecraft:waxed_copper_chest"));
        assert!(!joins("minecraft:barrel"));
    }

    #[test]
    fn both_halves_of_a_double_chest_share_one_position() {
        // Facing north, the left half's partner is east of it and the right half's west.
        let left = [10, 64, 5];
        let right = partner(left, Some("left"), Some("north")).unwrap();
        assert_eq!(right, [11, 64, 5]);
        assert_eq!(partner(right, Some("right"), Some("north")), Some(left));
        assert_eq!(canonical(left, Some(right)), canonical(right, Some(left)));
        for facing in ["north", "east", "south", "west"] {
            let p = partner(left, Some("left"), Some(facing)).unwrap();
            assert_eq!(
                partner(p, Some("right"), Some(facing)),
                Some(left),
                "{facing}"
            );
        }
        assert_eq!(partner(left, Some("single"), Some("north")), None);
        assert_eq!(partner(left, None, Some("north")), None);
    }

    #[test]
    fn frames_come_before_contents_then_overflow_then_nothing() {
        let chests = vec![
            chest([0, 0, 0], &[], &["minecraft:stone"]),
            chest([20, 0, 0], &["minecraft:stone"], &[]),
            chest([2, 0, 0], &["minecraft:egg"], &["minecraft:dirt"]),
        ];
        let mut places = Places::default();
        let skip = BTreeSet::new();
        let rules = Rules {
            chests: &chests,
            places: &places,
            fill_empty: false,
            skip: &skip,
        };
        // The frame wins over the nearer container that holds it.
        assert_eq!(rules.choose("minecraft:stone", [0, 0, 0]), Some([20, 0, 0]));
        // Contents of a framed container do not count: its frames decide.
        assert_eq!(rules.choose("minecraft:dirt", [0, 0, 0]), None);
        places.overflow = Some([5, 5, 5]);
        let rules = Rules {
            chests: &chests,
            places: &places,
            fill_empty: false,
            skip: &skip,
        };
        assert_eq!(rules.choose("minecraft:dirt", [0, 0, 0]), Some([5, 5, 5]));
    }

    #[test]
    fn contents_match_like_a_golem_and_the_nearest_wins() {
        let chests = vec![
            chest([30, 0, 0], &[], &["minecraft:stone"]),
            chest([3, 0, 0], &[], &["minecraft:stone"]),
        ];
        let places = Places::default();
        let skip = BTreeSet::new();
        let rules = Rules {
            chests: &chests,
            places: &places,
            fill_empty: false,
            skip: &skip,
        };
        assert_eq!(rules.choose("minecraft:stone", [0, 0, 0]), Some([3, 0, 0]));
    }

    #[test]
    fn full_skipped_inputs_and_outside_the_zone_are_never_chosen() {
        let mut chests = vec![
            chest([1, 0, 0], &["minecraft:egg"], &[]),
            chest([2, 0, 0], &["minecraft:egg"], &[]),
            chest([3, 0, 0], &["minecraft:egg"], &[]),
            chest([200, 0, 0], &["minecraft:egg"], &[]),
        ];
        chests[0].full = true;
        let places = Places {
            zone: Some(Zone::between([-10, -5, -10], [50, 5, 10])),
            inputs: vec![[2, 0, 0]],
            overflow: None,
        };
        let skip = BTreeSet::from([[3, 0, 0]]);
        let rules = Rules {
            chests: &chests,
            places: &places,
            fill_empty: false,
            skip: &skip,
        };
        assert_eq!(rules.choose("minecraft:egg", [0, 0, 0]), None);
    }

    #[test]
    fn empty_containers_are_taken_only_when_allowed_and_known_empty() {
        let mut unseen = chest([1, 0, 0], &[], &[]);
        unseen.seen = false;
        let chests = vec![
            unseen,
            chest([4, 0, 0], &[], &[]),
            chest([2, 0, 0], &["x"], &[]),
        ];
        let places = Places::default();
        let skip = BTreeSet::new();
        let mut rules = Rules {
            chests: &chests,
            places: &places,
            fill_empty: false,
            skip: &skip,
        };
        assert_eq!(rules.choose("minecraft:clay", [0, 0, 0]), None);
        rules.fill_empty = true;
        assert_eq!(rules.choose("minecraft:clay", [0, 0, 0]), Some([4, 0, 0]));
    }

    #[test]
    fn taking_and_putting_never_carry_off_the_players_own_items() {
        let input = vec![stack("a", 64), None, stack("b", 10), stack("c", 5)];
        let plan = take_plan(&input, 1, &|n| n != "b");
        assert_eq!(plan, vec![0]);
        assert_eq!(take_plan(&input, 9, &|n| n != "b"), vec![0, 3]);

        let before = vec![stack("a", 10), None, None, None];
        let keep = baseline(&before);
        let after = vec![stack("a", 10), stack("a", 64), stack("c", 5), None];
        assert_eq!(carried(&after, &keep), set(&["a", "c"]));
        // The 64 go; the player's own 10 stay.
        assert_eq!(put_slots(&after, &set(&["a"]), &keep), vec![1]);
        assert_eq!(put_slots(&after, &set(&["a", "c"]), &keep), vec![1, 2]);
        let done = vec![stack("a", 10), None, None, None];
        assert!(carried(&done, &keep).is_empty());
        assert!(put_slots(&done, &set(&["a"]), &keep).is_empty());
    }

    #[test]
    fn the_route_visits_the_nearest_stop_each_time_and_is_bounded() {
        let stops = [[10, 0, 0], [1, 0, 0], [5, 0, 0], [1, 0, 0]];
        assert_eq!(
            route([0, 0, 0], &stops),
            vec![[1, 0, 0], [5, 0, 0], [10, 0, 0]]
        );
        let many: Vec<Pos> = (0..200).map(|i| [i, 0, 0]).collect();
        assert_eq!(route([0, 0, 0], &many).len(), ROUTE_MAX);
    }

    #[test]
    fn observing_a_window_records_contents_and_fullness() {
        let mut list = Vec::new();
        observe(&mut list, "w", [1, 2, 3], &[stack("a", 1), None]);
        assert_eq!(list[0].contents, set(&["a"]));
        assert!(list[0].seen && !list[0].full);
        observe(&mut list, "w", [1, 2, 3], &[stack("a", 1), stack("b", 2)]);
        assert_eq!(list.len(), 1);
        assert!(list[0].full);
    }

    #[test]
    fn frames_seen_nearby_replace_labels_and_add_new_containers() {
        let mut list = vec![
            chest([0, 0, 0], &["old"], &[]),
            chest([99, 0, 0], &["far"], &[]),
        ];
        let seen = BTreeMap::from([([5, 0, 0], set(&["minecraft:egg"]))]);
        assert!(apply_labels(&mut list, "w", &seen, &|p| p[0] < 50));
        assert!(list[0].labels.is_empty(), "its frame was taken down");
        assert_eq!(list[1].labels, set(&["far"]), "out of sight: kept");
        assert_eq!(list[2].pos, [5, 0, 0]);
        assert_eq!(list[2].labels, set(&["minecraft:egg"]));
        assert!(!apply_labels(&mut list, "w", &seen, &|p| p[0] < 50));
    }

    #[test]
    fn the_index_is_bounded_and_reads_back() {
        let mut list = Vec::new();
        for i in 0..(MAX_CHESTS as i32 + 10) {
            entry(&mut list, "w", [i, 0, 0]);
        }
        assert_eq!(list.len(), MAX_CHESTS);
        assert_eq!(list[0].pos, [10, 0, 0]);

        let mut c = chest(
            [1, -2, 3],
            &["minecraft:egg"],
            &["minecraft:stone", "minecraft:dirt"],
        );
        c.scope = "srv\t#overworld".into();
        c.full = true;
        let back = decode_index(&encode_index(&[c.clone()]));
        assert_eq!(back.len(), 1);
        assert_eq!(back[0].scope, "srv#overworld");
        assert_eq!(back[0].labels, c.labels);
        assert_eq!(back[0].contents, c.contents);
        assert!(back[0].seen && back[0].full);
        assert!(decode_index("bad\nw\t1\t2\tx\t\t\t0\t0\nw\t1").is_empty());
    }

    #[test]
    fn places_read_back_and_bad_or_oversized_lines_are_dropped() {
        let mut all = BTreeMap::new();
        all.insert(
            "w".to_owned(),
            Places {
                zone: Some(Zone::between([0, 10, 0], [-20, 20, 30])),
                inputs: vec![[1, 2, 3], [4, 5, 6]],
                overflow: Some([7, 8, 9]),
            },
        );
        let back = decode_places(&encode_places(&all));
        assert_eq!(back, all);
        let bad = "w\tzone\t0\t0\t0\t500\t0\t0\nw\tinput\t1\tx\t3\nw\tnope\t1\t2\t3";
        assert!(decode_places(bad).is_empty());
    }

    #[test]
    fn the_zone_is_walked_block_by_block() {
        let z = Zone::between([0, 0, 0], [1, 1, 2]);
        assert_eq!(z.volume(), 12);
        let all: Vec<Pos> = (0..12).filter_map(|i| z.nth(i)).collect();
        assert_eq!(all.len(), 12);
        assert!(all.iter().all(|p| z.contains(*p)));
        let mut unique = all.clone();
        unique.sort_unstable();
        unique.dedup();
        assert_eq!(unique.len(), 12);
        assert_eq!(z.nth(12), None);
        assert_eq!(
            Zone::between([0, 0, 0], [128, 0, 0]).too_large(),
            Some("side")
        );
        assert_eq!(
            Zone::between([0, 0, 0], [0, 24, 0]).too_large(),
            Some("height")
        );
        assert_eq!(Zone::between([0, 0, 0], [127, 23, 127]).too_large(), None);
    }
}
