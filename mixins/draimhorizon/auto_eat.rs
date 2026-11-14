// Pure rules for eating on its own while a task runs: when to eat and what.
/// Farm restocking threshold (out of 20); eating itself uses each food's exact value.
pub const HUNGRY_AT: i32 = 14;
/// How long to hold the use button for one bite before giving up (a meal takes 32 ticks).
pub const EAT_TIMEOUT_MS: u64 = 4000;
/// Foods with an effect nobody wants from a task: poison, hunger, nausea, teleporting, or
/// a rare and valuable item that is not worth a snack.
pub fn avoid(item: &str) -> bool {
    matches!(
        item.strip_prefix("minecraft:").unwrap_or(item),
        "rotten_flesh"
            | "spider_eye"
            | "pufferfish"
            | "poisonous_potato"
            | "chicken"
            | "chorus_fruit"
            | "golden_apple"
            | "enchanted_golden_apple"
            | "suspicious_stew"
            | "ominous_bottle"
            | "honey_bottle"
            | "milk_bucket"
    )
}
/// A food in the inventory: its slot, the item and the nutrition it restores.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Food {
    pub slot: usize,
    pub item: String,
    pub nutrition: i32,
}
/// Whether there is room for food; the chosen food must still fit in full.
pub fn hungry(food: i32, enabled: bool) -> bool {
    enabled && (0..20).contains(&food)
}
/// Read the explicit allowlist. Missing or malformed settings never allow arbitrary food.
pub fn allowed_foods(setting: Option<&str>) -> Vec<String> {
    setting
        .filter(|s| s.len() <= 16_384)
        .and_then(|s| serde_json::from_str::<Vec<String>>(s).ok())
        .unwrap_or_default()
        .into_iter()
        .filter(|item| !avoid(item))
        .take(256)
        .collect()
}
pub fn allowed(item: &str, foods: &[String]) -> bool {
    !avoid(item) && foods.iter().any(|f| f == item)
}
/// The food that restores the most without wasting any hunger points. Never eat at a full
/// bar, and wait rather than falling back to a food that is too large.
pub fn choose(foods: &[Food], level: i32) -> Option<&Food> {
    if !hungry(level, true) {
        return None;
    }
    let room = 20 - level;
    foods
        .iter()
        .filter(|f| !avoid(&f.item) && f.nutrition > 0 && f.nutrition <= room)
        .max_by_key(|f| (f.nutrition, std::cmp::Reverse(f.slot)))
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn food_waits_until_every_hunger_point_fits() {
        let foods = vec![food(0, "minecraft:cooked_beef", 8)];
        assert_eq!(choose(&foods, 14), None);
        assert_eq!(choose(&foods, 13), None);
        assert_eq!(choose(&foods, 12), Some(&foods[0]));
        assert_eq!(choose(&foods, 20), None);
    }
    fn food(slot: usize, item: &str, nutrition: i32) -> Food {
        Food {
            slot,
            item: item.into(),
            nutrition,
        }
    }
    #[test]
    fn eating_needs_missing_hunger_and_must_be_enabled() {
        assert!(hungry(14, true));
        assert!(hungry(3, true));
        assert!(hungry(15, true));
        assert!(hungry(19, true));
        assert!(!hungry(-1, true));
        assert!(!hungry(20, true));
        assert!(!hungry(3, false));
    }
    #[test]
    fn the_best_fitting_safe_food_is_chosen() {
        let foods = vec![
            food(1, "minecraft:bread", 5),
            food(2, "minecraft:cooked_beef", 8),
            food(3, "minecraft:rotten_flesh", 4),
            food(4, "minecraft:golden_apple", 4),
            food(5, "minecraft:apple", 4),
        ];
        // Hungry (10/20): the biggest that still fits, never the risky ones.
        assert_eq!(choose(&foods, 10).unwrap().item, "minecraft:cooked_beef");
        // No food fits: wait, even if this is the only food left.
        assert_eq!(choose(&foods, 17), None);
        // Room for 5: bread (5) beats apple (4).
        assert_eq!(choose(&foods, 15).unwrap().item, "minecraft:bread");
        // Only risky food: nothing is eaten.
        let bad = vec![
            food(0, "minecraft:rotten_flesh", 4),
            food(1, "minecraft:poisonous_potato", 2),
        ];
        assert_eq!(choose(&bad, 4), None);
        assert_eq!(choose(&[], 4), None);
    }
    #[test]
    fn only_explicitly_allowed_safe_foods_are_used() {
        let list = allowed_foods(Some(r#"["minecraft:bread","minecraft:golden_apple"]"#));
        assert!(allowed("minecraft:bread", &list));
        assert!(!allowed("minecraft:apple", &list));
        assert!(!allowed("minecraft:golden_apple", &list));
        let saved = serde_json::to_string(&list).unwrap();
        assert_eq!(allowed_foods(Some(&saved)), list);
        for setting in [None, Some("[]"), Some("broken"), Some("{}"), Some("[1]")] {
            assert!(allowed_foods(setting).is_empty());
        }
    }
    #[test]
    fn exact_fit_small_snacks_and_stable_slots() {
        let foods = vec![
            food(8, "minecraft:bread", 5),
            food(2, "minecraft:bread", 5),
            food(1, "minecraft:carrot", 3),
        ];
        assert_eq!(choose(&foods, 15).unwrap().slot, 2);
        assert_eq!(choose(&foods, 17).unwrap().slot, 1);
        assert_eq!(choose(&foods, 18), None);
        assert_eq!(choose(&foods, 21), None);
        assert_eq!(choose(&[food(0, "minecraft:apple", 0)], 0), None);
    }
}
