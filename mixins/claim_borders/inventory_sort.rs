// Pure, bounded chest sorting plans. Every transaction ends with an empty cursor.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Stack {
    pub key: String,
    pub count: u32,
    pub max: u32,
}
#[derive(Clone, Debug)]
pub struct Plan {
    pub clicks: Vec<usize>,
    pub after: Vec<Option<Stack>>,
}
pub fn next(slots: &[Option<Stack>]) -> Result<Option<Plan>, String> {
    if slots.is_empty()
        || slots.len() > 54
        || slots
            .iter()
            .flatten()
            .any(|s| s.count == 0 || s.max == 0 || s.count > s.max)
    {
        return Err("unsupported chest contents".into());
    }
    for i in 0..slots.len() {
        if let Some(a) = &slots[i]
            && a.count < a.max
        {
            for j in i + 1..slots.len() {
                if let Some(b) = &slots[j]
                    && a.key == b.key
                    && a.max == b.max
                {
                    let moved = (a.max - a.count).min(b.count);
                    let mut after = slots.to_vec();
                    after[i].as_mut().unwrap().count += moved;
                    let remaining = b.count - moved;
                    after[j] = if remaining == 0 {
                        None
                    } else {
                        Some(Stack {
                            count: remaining,
                            ..b.clone()
                        })
                    };
                    return Ok(Some(Plan {
                        clicks: if remaining == 0 {
                            vec![j, i]
                        } else {
                            vec![j, i, j]
                        },
                        after,
                    }));
                }
            }
        }
    }
    for i in 0..slots.len() {
        let smallest = (i..slots.len())
            .filter(|j| slots[*j].is_some())
            .min_by(|a, b| {
                slots[*a]
                    .as_ref()
                    .unwrap()
                    .key
                    .cmp(&slots[*b].as_ref().unwrap().key)
            });
        if let Some(j) = smallest
            && j != i
            && slots[i]
                .as_ref()
                .is_none_or(|a| slots[j].as_ref().unwrap().key < a.key)
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
#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeMap;
    fn totals(slots: &[Option<Stack>]) -> BTreeMap<String, u32> {
        let mut result = BTreeMap::new();
        for s in slots.iter().flatten() {
            *result.entry(s.key.clone()).or_default() += s.count;
        }
        result
    }
    #[test]
    fn plans_conserve_metadata_and_terminate_for_small_inventories() {
        let values = [
            None,
            Some(Stack {
                key: "a".into(),
                count: 1,
                max: 64,
            }),
            Some(Stack {
                key: "a".into(),
                count: 63,
                max: 64,
            }),
            Some(Stack {
                key: "a:enchanted".into(),
                count: 1,
                max: 1,
            }),
            Some(Stack {
                key: "b".into(),
                count: 1,
                max: 64,
            }),
        ];
        for seed in 0..625 {
            let mut n = seed;
            let mut slots = Vec::new();
            for _ in 0..4 {
                slots.push(values[n % 5].clone());
                n /= 5;
            }
            let original = totals(&slots);
            let mut steps = 0;
            while let Some(plan) = next(&slots).unwrap() {
                assert!(plan.clicks.len() <= 3);
                assert!(plan.clicks.iter().all(|i| *i < 4));
                assert_eq!(totals(&plan.after), original);
                slots = plan.after;
                steps += 1;
                assert!(steps < 20);
            }
            let keys: Vec<_> = slots.iter().flatten().map(|s| s.key.as_str()).collect();
            assert!(keys.windows(2).all(|pair| pair[0] <= pair[1]));
        }
    }
}
