// Pure bounded plans. Each transaction leaves the cursor empty; native code verifies it.
use std::cmp::Ordering;
use std::collections::BTreeSet;
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Stack {
    /// Exact item, damage and components identity. Never merge by display name.
    pub key: String,
    pub id: String,
    pub name: String,
    pub count: u32,
    pub max: u32,
}
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Mode {
    #[default]
    Id,
    Name,
    Quantity,
}
impl Mode {
    pub fn next(self) -> Self {
        match self {
            Self::Id => Self::Name,
            Self::Name => Self::Quantity,
            Self::Quantity => Self::Id,
        }
    }
    pub fn label(self) -> &'static str {
        match self {
            Self::Id => "По ID",
            Self::Name => "По имени",
            Self::Quantity => "По количеству",
        }
    }
    pub fn number(self) -> u8 {
        match self {
            Self::Id => 0,
            Self::Name => 1,
            Self::Quantity => 2,
        }
    }
    pub fn from_number(n: u64) -> Self {
        match n {
            1 => Self::Name,
            2 => Self::Quantity,
            _ => Self::Id,
        }
    }
    fn compare(self, a: &Stack, b: &Stack) -> Ordering {
        let first = match self {
            Self::Id => a.id.cmp(&b.id),
            Self::Name => a.name.to_lowercase().cmp(&b.name.to_lowercase()),
            Self::Quantity => b.count.cmp(&a.count),
        };
        first
            .then_with(|| a.key.cmp(&b.key))
            .then_with(|| b.count.cmp(&a.count))
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Operation {
    Sort,
    Fill,
    Restock,
    Transfer,
}
impl Operation {
    pub fn symbol(self) -> &'static str {
        match self {
            Self::Sort => "S",
            Self::Fill => "F",
            Self::Restock => "R",
            Self::Transfer => "T",
        }
    }
    pub fn label(self) -> &'static str {
        match self {
            Self::Sort => "Сортировать",
            Self::Fill => "Дополнить неполные стаки",
            Self::Restock => "Пополнить имеющиеся предметы",
            Self::Transfer => "Перенести всё",
        }
    }
}
#[derive(Clone, Debug)]
pub struct Plan {
    pub clicks: Vec<usize>,
    pub after: Vec<Option<Stack>>,
}
fn valid(slots: &[Option<Stack>], indices: &[usize]) -> Result<(), String> {
    let unique: BTreeSet<_> = indices.iter().copied().collect();
    if slots.is_empty()
        || slots.len() > 256
        || indices.len() != unique.len()
        || indices.iter().any(|i| *i >= slots.len())
        || slots
            .iter()
            .flatten()
            .any(|s| s.count == 0 || s.max == 0 || s.count > s.max)
    {
        return Err("unsupported inventory contents or slot selection".into());
    }
    Ok(())
}
fn same(a: &Stack, b: &Stack) -> bool {
    a.key == b.key && a.max == b.max
}
fn move_into(slots: &[Option<Stack>], source: usize, dest: usize) -> Option<Plan> {
    let from = slots[source].as_ref()?;
    let room = match &slots[dest] {
        Some(to) if same(from, to) => to.max.saturating_sub(to.count),
        None => from.max,
        _ => 0,
    };
    let moved = room.min(from.count);
    if moved == 0 {
        return None;
    }
    let mut after = slots.to_vec();
    after[dest] = Some(Stack {
        count: slots[dest].as_ref().map_or(0, |s| s.count) + moved,
        ..from.clone()
    });
    let remaining = from.count - moved;
    after[source] = (remaining > 0).then(|| Stack {
        count: remaining,
        ..from.clone()
    });
    Some(Plan {
        clicks: if remaining == 0 {
            vec![source, dest]
        } else {
            vec![source, dest, source]
        },
        after,
    })
}
pub fn next_sort(
    slots: &[Option<Stack>],
    order: &[usize],
    mode: Mode,
) -> Result<Option<Plan>, String> {
    valid(slots, order)?;
    for (offset, &i) in order.iter().enumerate() {
        if let Some(a) = &slots[i]
            && a.count < a.max
        {
            for &j in &order[offset + 1..] {
                if slots[j].as_ref().is_some_and(|b| same(a, b)) {
                    return Ok(move_into(slots, j, i));
                }
            }
        }
    }
    for (offset, &i) in order.iter().enumerate() {
        let smallest = order[offset..]
            .iter()
            .copied()
            .filter(|j| slots[*j].is_some())
            .min_by(|a, b| mode.compare(slots[*a].as_ref().unwrap(), slots[*b].as_ref().unwrap()));
        if let Some(j) = smallest
            && j != i
            && slots[i]
                .as_ref()
                .is_none_or(|a| mode.compare(slots[j].as_ref().unwrap(), a).is_lt())
        {
            let mut after = slots.to_vec();
            after.swap(i, j);
            return Ok(Some(Plan {
                clicks: if slots[i].is_none() {
                    vec![j, i]
                } else {
                    vec![j, i, j]
                },
                after,
            }));
        }
    }
    Ok(None)
}
/// Destination is the area whose button was pressed. Hotbar/locked slots never enter either list.
/// Restock types are captured at operation start, so it cannot introduce a new item type.
pub fn next_transfer(
    slots: &[Option<Stack>],
    source: &[usize],
    dest: &[usize],
    operation: Operation,
    restock_types: &BTreeSet<String>,
    allowed: impl Fn(usize, usize) -> bool,
) -> Result<Option<Plan>, String> {
    let both: Vec<_> = source.iter().chain(dest).copied().collect();
    valid(slots, &both)?;
    if operation == Operation::Sort {
        return Err("sort requires a single area".into());
    }
    for &from in source {
        let Some(stack) = &slots[from] else {
            continue;
        };
        if operation == Operation::Restock && !restock_types.contains(&stack.key) {
            continue;
        }
        // Existing partial stacks before empty slots.
        for empty in [false, true] {
            if empty && operation == Operation::Fill {
                continue;
            }
            for &to in dest {
                if slots[to].is_none() != empty || !allowed(from, to) {
                    continue;
                }
                if let Some(plan) = move_into(slots, from, to) {
                    return Ok(Some(plan));
                }
            }
        }
    }
    Ok(None)
}
/// Stable slot traversal for rows or columns. Indices remain logical slots, not pixels.
pub fn traversal(indices: &[usize], columns: usize, by_columns: bool) -> Vec<usize> {
    let mut result = indices.to_vec();
    if by_columns && columns > 0 {
        result.sort_by_key(|i| (i % columns, i / columns));
    }
    result
}
pub fn next(slots: &[Option<Stack>]) -> Result<Option<Plan>, String> {
    next_sort(slots, &(0..slots.len()).collect::<Vec<_>>(), Mode::Id)
}
/// Logical Game slot ids; the native adapter validates every outgoing click against this set.
pub fn safe_game_slot(i: usize) -> bool {
    (9..36).contains(&i) || (300..354).contains(&i) || (710..725).contains(&i)
}
#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeMap;
    fn stack(id: &str, count: u32) -> Option<Stack> {
        Some(Stack {
            key: id.into(),
            id: id.into(),
            name: id.into(),
            count,
            max: 64,
        })
    }
    fn totals(slots: &[Option<Stack>]) -> BTreeMap<String, u32> {
        let mut result = BTreeMap::new();
        for s in slots.iter().flatten() {
            *result.entry(s.key.clone()).or_default() += s.count;
        }
        result
    }
    // Independent left-click interpreter, including partial merges and swaps.
    fn apply(slots: &mut [Option<Stack>], clicks: &[usize]) {
        let mut cursor: Option<Stack> = None;
        for &i in clicks {
            match (cursor.take(), slots[i].take()) {
                (None, slot) => cursor = slot,
                (Some(a), Some(mut b)) if same(&a, &b) => {
                    let n = (b.max - b.count).min(a.count);
                    b.count += n;
                    cursor = (a.count > n).then(|| Stack {
                        count: a.count - n,
                        ..a
                    });
                    slots[i] = Some(b);
                }
                (hand, slot) => {
                    slots[i] = hand;
                    cursor = slot;
                }
            }
        }
        assert!(cursor.is_none(), "cursor must finish empty");
    }
    #[test]
    fn all_modes_conserve_components_and_terminate_with_locked_slots() {
        let values = [
            None,
            stack("a", 1),
            stack("a", 63),
            stack("a:enchanted", 1),
            stack("b", 5),
        ];
        for seed in 0..625 {
            for mode in [Mode::Id, Mode::Name, Mode::Quantity] {
                let mut n = seed;
                let mut slots = vec![stack("locked", 17)];
                for _ in 0..4 {
                    slots.push(values[n % 5].clone());
                    n /= 5;
                }
                let original = totals(&slots);
                let locked = slots[0].clone();
                let mut steps = 0;
                while let Some(plan) = next_sort(&slots, &[1, 2, 3, 4], mode).unwrap() {
                    assert!(plan.clicks.len() <= 3);
                    assert!(plan.clicks.iter().all(|i| (1..5).contains(i)));
                    apply(&mut slots, &plan.clicks);
                    assert_eq!(slots, plan.after);
                    assert_eq!(slots[0], locked);
                    assert_eq!(totals(&slots), original);
                    steps += 1;
                    assert!(steps < 20);
                }
                let items: Vec<_> = slots[1..].iter().flatten().collect();
                assert!(items.windows(2).all(|p| !mode.compare(p[0], p[1]).is_gt()));
            }
        }
    }
    #[test]
    fn transfer_policies_and_locked_hotbar_preserve_items() {
        for op in [Operation::Fill, Operation::Restock, Operation::Transfer] {
            let mut slots = vec![
                stack("a", 60),
                None,
                None,
                stack("a", 12),
                stack("b", 20),
                stack("hotbar", 32),
            ];
            let original = totals(&slots);
            let types = BTreeSet::from(["a".to_owned()]);
            let mut steps = 0;
            while let Some(plan) =
                next_transfer(&slots, &[3, 4], &[0, 1, 2], op, &types, |_, _| true).unwrap()
            {
                assert!(!plan.clicks.contains(&5));
                apply(&mut slots, &plan.clicks);
                assert_eq!(slots, plan.after);
                assert_eq!(totals(&slots), original);
                steps += 1;
                assert!(steps < 10);
            }
            assert_eq!(slots[0].as_ref().unwrap().count, 64);
            assert_eq!(slots[5], stack("hotbar", 32));
            match op {
                Operation::Fill => {
                    assert!(slots[1].is_none());
                    assert_eq!(slots[3], stack("a", 8));
                }
                Operation::Restock => {
                    assert_eq!(slots[1], stack("a", 8));
                    assert_eq!(slots[4], stack("b", 20));
                }
                Operation::Transfer => {
                    assert!(slots[3].is_none() && slots[4].is_none());
                }
                _ => unreachable!(),
            }
        }
    }
    #[test]
    fn display_names_do_not_merge_distinct_components_and_order_uses_names() {
        let mut a = stack("z", 1).unwrap();
        a.name = "Apple".into();
        let mut b = stack("a", 1).unwrap();
        b.name = "Zebra".into();
        let slots = vec![Some(b), Some(a)];
        assert!(next_sort(&slots, &[0, 1], Mode::Id).unwrap().is_none());
        let plan = next_sort(&slots, &[0, 1], Mode::Name).unwrap().unwrap();
        assert_eq!(plan.after[0].as_ref().unwrap().id, "z");
        assert_eq!(totals(&plan.after), totals(&slots));
    }
    #[test]
    fn rejects_overlap_invalid_counts_and_respects_acceptance() {
        let slots = vec![stack("a", 1), None];
        assert!(
            next_transfer(
                &slots,
                &[0],
                &[0],
                Operation::Transfer,
                &BTreeSet::new(),
                |_, _| true
            )
            .is_err()
        );
        assert!(
            next_transfer(
                &slots,
                &[0],
                &[1],
                Operation::Transfer,
                &BTreeSet::new(),
                |_, _| false
            )
            .unwrap()
            .is_none()
        );
        assert!(next_sort(&slots, &[0, 2], Mode::Id).is_err());
        assert!(next(&[stack("bad", 65)]).is_err());
        assert_eq!(
            traversal(&[0, 1, 2, 3, 4, 5], 3, true),
            vec![0, 3, 1, 4, 2, 5]
        );
    }
    #[test]
    fn game_slot_allowlist_excludes_hotbar_equipment_and_crafting() {
        for i in 0..9 {
            assert!(!safe_game_slot(i));
        }
        for i in [
            36, 100, 101, 200, 500, 501, 502, 503, 504, 700, 701, 354, 725,
        ] {
            assert!(!safe_game_slot(i));
        }
        assert!((9..36).all(safe_game_slot));
        assert!((300..354).all(safe_game_slot));
        assert!((710..725).all(safe_game_slot));
    }
}
