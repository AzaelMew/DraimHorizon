// Pure rules for eating on its own while a task runs: when to eat and what.
/// Eat when the food bar is at or below this (of 20): 7 shanks.
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
/// Whether to start eating now.
pub fn hungry(food: i32, enabled: bool) -> bool {
    enabled && food <= HUNGRY_AT
}
/// The food to eat at `level`: the one that restores the most without wasting nutrition beyond
/// a full bar; when every food would waste some, the one that wastes the least.
pub fn choose(foods: &[Food], level: i32) -> Option<&Food> {
    let room = (20 - level).max(0);
    let usable = || foods.iter().filter(|f| !avoid(&f.item) && f.nutrition > 0);
    usable()
        .filter(|f| f.nutrition <= room)
        .max_by_key(|f| (f.nutrition, std::cmp::Reverse(f.slot)))
        .or_else(|| usable().min_by_key(|f| (f.nutrition, f.slot)))
}
#[cfg(test)]
mod tests {
    use super::*;
    fn food(slot: usize, item: &str, nutrition: i32) -> Food {
        Food {
            slot,
            item: item.into(),
            nutrition,
        }
    }
    #[test]
    fn eating_starts_at_seven_shanks_and_only_when_enabled() {
        assert!(hungry(14, true));
        assert!(hungry(3, true));
        assert!(!hungry(15, true));
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
        // Little room left (17/20): the largest that fits is 3, none does, so the smallest.
        assert_eq!(choose(&foods, 17).unwrap().item, "minecraft:apple");
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
}
