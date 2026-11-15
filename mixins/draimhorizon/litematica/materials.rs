// A port of Litematica's material list (LGPL-3.0, see NOTICE.md): `MaterialListEntry` (what is
// needed, what is still missing, what is the wrong block, what the player has), the entries of
// `MaterialListUtils.getMaterialList`, the counting of an inventory (a shulker box counts what
// it holds, a bundle what it holds) and the order of `MaterialListSorter`. Pure: items are ids,
// and the conversion from blocks to items is passed in.
use std::collections::HashMap;

/// `MaterialListEntry`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Entry {
    pub item: String,
    pub total: usize,
    pub missing: usize,
    pub mismatched: usize,
    pub available: usize,
}
impl Entry {
    /// What the player still has to get: needed and not missing-yet-held (`total - available`
    /// never below nothing). The original shows `countMissing` and `countAvailable` side by side;
    /// this is the column the "shortfall" of its list view is made of.
    pub fn shortfall(&self) -> usize {
        self.missing.saturating_sub(self.available)
    }
}

/// `MaterialListBase.SortCriteria`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Sort {
    Name,
    CountTotal,
    CountMissing,
    CountAvailable,
}
impl Sort {
    pub fn next(self) -> Self {
        match self {
            Sort::Name => Sort::CountTotal,
            Sort::CountTotal => Sort::CountMissing,
            Sort::CountMissing => Sort::CountAvailable,
            Sort::CountAvailable => Sort::Name,
        }
    }
}

/// One stack of an inventory: an item, how many, and what it holds when it is a shulker box or a
/// bundle (`getStoredItemCounts`, `getBundleItemCounts`).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Stack {
    pub item: String,
    pub count: usize,
    pub inside: Vec<Stack>,
}
impl Stack {
    pub fn new(item: &str, count: usize) -> Self {
        Self {
            item: item.to_owned(),
            count,
            inside: Vec::new(),
        }
    }
}

/// `getInventoryItemCounts`: a box or a bundle with something in it counts what it holds (and not
/// itself); every other stack counts as it is. Held items count once per box in the stack.
pub fn inventory_counts(stacks: &[Stack]) -> HashMap<String, usize> {
    let mut map = HashMap::new();
    for stack in stacks {
        if stack.count == 0 || stack.item.is_empty() {
            continue;
        }
        if stack.inside.is_empty() {
            *map.entry(stack.item.clone()).or_default() += stack.count;
        } else {
            add_held(&mut map, &stack.inside, stack.count);
        }
    }
    map
}
fn add_held(map: &mut HashMap<String, usize>, inside: &[Stack], multiplier: usize) {
    for held in inside {
        if held.count == 0 {
            continue;
        }
        // A bundle in a box: what it holds is counted too.
        if !held.inside.is_empty() {
            add_held(map, &held.inside, multiplier);
        }
        *map.entry(held.item.clone()).or_default() += held.count * multiplier;
    }
}

/// `convertStatesToStacks`: the counts of states as counts of items. `items` says which items,
/// and how many of each, one block of a state takes (nothing for a block that is not placed by
/// an item); a waterlogged block also takes a water bucket.
pub fn states_to_items<S>(
    states: &HashMap<S, usize>,
    waterlogged: impl Fn(&S) -> bool,
    items: impl Fn(&S) -> Vec<(String, usize)>,
) -> HashMap<String, usize>
where
    S: std::hash::Hash + Eq,
{
    let mut out: HashMap<String, usize> = HashMap::new();
    for (state, count) in states {
        if waterlogged(state) {
            *out.entry("minecraft:water_bucket".into()).or_default() += count;
        }
        for (item, per) in items(state) {
            if per > 0 {
                *out.entry(item).or_default() += count * per;
            }
        }
    }
    out
}

/// `getMaterialList`: one entry for every item of `total`, with what is missing and mismatched of
/// it and what the player has (`available`; empty without a player).
pub fn entries(
    total: &HashMap<String, usize>,
    missing: &HashMap<String, usize>,
    mismatched: &HashMap<String, usize>,
    available: &HashMap<String, usize>,
) -> Vec<Entry> {
    let mut list: Vec<Entry> = total
        .iter()
        .map(|(item, count)| Entry {
            item: item.clone(),
            total: *count,
            missing: missing.get(item).copied().unwrap_or(0),
            mismatched: mismatched.get(item).copied().unwrap_or(0),
            available: available.get(item).copied().unwrap_or(0),
        })
        .collect();
    list.sort_by(|a, b| a.item.cmp(&b.item));
    list
}

/// `MaterialListSorter`: by the chosen count, biggest first (the other way round when reversed),
/// equal counts by name; by name, the original's order is names backwards unless reversed.
pub fn sort(list: &mut [Entry], by: Sort, reverse: bool, name: impl Fn(&str) -> String) {
    list.sort_by(|a, b| {
        let by_name = name(&a.item).cmp(&name(&b.item));
        let count = |x: &Entry| match by {
            Sort::CountTotal => x.total,
            Sort::CountMissing => x.missing,
            Sort::CountAvailable => x.available,
            Sort::Name => 0,
        };
        if by == Sort::Name {
            return if reverse { by_name } else { by_name.reverse() };
        }
        let (ca, cb) = (count(a), count(b));
        if ca == cb {
            by_name
        } else if (ca > cb) != reverse {
            std::cmp::Ordering::Less
        } else {
            std::cmp::Ordering::Greater
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    fn map(pairs: &[(&str, usize)]) -> HashMap<String, usize> {
        pairs.iter().map(|(k, v)| ((*k).to_owned(), *v)).collect()
    }

    #[test]
    fn a_box_counts_what_it_holds_and_not_itself() {
        let mut shulker = Stack::new("minecraft:shulker_box", 2);
        shulker.inside = vec![
            Stack::new("minecraft:stone", 64),
            Stack::new("minecraft:dirt", 3),
        ];
        let mut bundle = Stack::new("minecraft:bundle", 1);
        bundle.inside = vec![Stack::new("minecraft:dirt", 5)];
        shulker.inside.push(bundle);
        let counts = inventory_counts(&[
            shulker,
            Stack::new("minecraft:stone", 10),
            Stack::new("minecraft:air", 0),
        ]);
        // Two boxes of 64 stone, plus the loose stack.
        assert_eq!(counts["minecraft:stone"], 138);
        assert_eq!(counts["minecraft:dirt"], 16);
        assert!(!counts.contains_key("minecraft:shulker_box"));
        assert!(!counts.contains_key("minecraft:bundle") || counts["minecraft:bundle"] == 2);
    }

    #[test]
    fn states_become_items_with_a_bucket_for_waterlogged_blocks() {
        let states: HashMap<&str, usize> = [("slab", 3), ("wet_stairs", 2)].into_iter().collect();
        let items = states_to_items(
            &states,
            |s| *s == "wet_stairs",
            |s| match *s {
                "slab" => vec![("minecraft:stone_slab".to_owned(), 2)],
                _ => vec![("minecraft:stone_stairs".to_owned(), 1)],
            },
        );
        assert_eq!(items["minecraft:stone_slab"], 6);
        assert_eq!(items["minecraft:stone_stairs"], 2);
        assert_eq!(items["minecraft:water_bucket"], 2);
    }

    #[test]
    fn entries_carry_total_missing_and_available_and_sort_like_the_original() {
        let total = map(&[("b", 10), ("a", 10), ("c", 3)]);
        let missing = map(&[("b", 4), ("a", 10)]);
        let have = map(&[("a", 2), ("c", 3)]);
        let mut list = entries(&total, &missing, &HashMap::new(), &have);
        assert_eq!(list.len(), 3);
        assert_eq!(list[0].shortfall(), 8);
        sort(&mut list, Sort::CountMissing, false, str::to_owned);
        assert_eq!(
            list.iter().map(|e| e.item.as_str()).collect::<Vec<_>>(),
            ["a", "b", "c"]
        );
        // Equal totals fall back to the name; reversed turns the counts round.
        sort(&mut list, Sort::CountTotal, false, str::to_owned);
        assert_eq!(list[0].item, "a");
        assert_eq!(list[1].item, "b");
        sort(&mut list, Sort::CountTotal, true, str::to_owned);
        assert_eq!(list[0].item, "c");
        // The name order of the original runs backwards until it is reversed.
        sort(&mut list, Sort::Name, false, str::to_owned);
        assert_eq!(list[0].item, "c");
        sort(&mut list, Sort::Name, true, str::to_owned);
        assert_eq!(list[0].item, "a");
        assert_eq!(Sort::Name.next().next().next().next(), Sort::Name);
    }
}
