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
/// Settings key of "Eat to heal"; absent or anything but "true" means off.
pub const HEAL_KEY: &str = "auto_eat_to_heal";
/// Whether the setting value turns "Eat to heal" on (off by default).
pub fn heal_enabled(setting: Option<&str>) -> bool {
    setting == Some("true")
}
/// Settings key of the saved Auto Eat switch; absent means on (the default).
pub const ENABLED_KEY: &str = "auto_eat";
/// Settings key holding the experimental-features consent.
pub const CONSENT_KEY: &str = "experiments_accepted";
/// Whether the saved switch is on. Absent or anything but "false" means on.
pub fn enabled(setting: Option<&str>) -> bool {
    setting != Some("false")
}
/// Whether the player has accepted the experimental warning.
pub fn consented(setting: Option<&str>) -> bool {
    setting == Some("true")
}
/// Whether Auto Eat may act now, from the raw saved switch and consent values exactly as the
/// adapter reads them (a fresh profile has neither).
pub fn active(setting: Option<&str>, consent: Option<&str>) -> bool {
    consented(consent) && enabled(setting)
}
/// Whether a living player is below their actual maximum health, as Horizon decides it
/// (`living_health::is_hurt`): health is an `f32`, and the maximum attribute is narrowed to
/// `f32` before the comparison, so a full bar at a fractional maximum is not injured. Missing,
/// non-finite, non-positive or dead values never count as injured.
pub fn injured(health: f32, max_health: f64) -> bool {
    let max = max_health as f32;
    health.is_finite() && max.is_finite() && max > 0.0 && health > 0.0 && health < max
}
/// Whether a food of `nutrition` may be eaten at hunger `level`: it fits in full, or, while
/// healing, any positive food at a bar that is not full (the overfill is accepted).
pub fn eligible(nutrition: i32, level: i32, heal: bool) -> bool {
    hungry(level, true) && nutrition > 0 && (nutrition <= 20 - level || heal)
}
/// The food that restores the most without wasting any hunger points. Never eat at a full
/// bar, and wait rather than falling back to a food that is too large.
pub fn choose(foods: &[Food], level: i32) -> Option<&Food> {
    choose_healing(foods, level, false)
}
/// Like [`choose`]. While `heal` is set (injured, "Eat to heal" on) and nothing fits in full,
/// the smallest safe food is taken, so as little hunger as possible is wasted.
pub fn choose_healing(foods: &[Food], level: i32, heal: bool) -> Option<&Food> {
    if !hungry(level, true) {
        return None;
    }
    let safe = || {
        foods
            .iter()
            .filter(|f| !avoid(&f.item) && eligible(f.nutrition, level, heal))
    };
    safe()
        .filter(|f| f.nutrition <= 20 - level)
        .max_by_key(|f| (f.nutrition, std::cmp::Reverse(f.slot)))
        .or_else(|| safe().min_by_key(|f| (f.nutrition, f.slot)))
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
    fn a_fresh_profile_never_eats_before_the_experimental_warning_is_accepted() {
        // A fresh profile has neither key: the switch reads on, the consent is missing.
        assert!(enabled(None) && !consented(None));
        assert!(!active(None, None), "default-on Auto Eat needs consent");
        for consent in [Some("false"), Some(""), Some("1"), Some("TRUE")] {
            assert!(!active(None, consent));
            assert!(!active(Some("true"), consent));
        }
        // Consent given: the saved switch decides, default on, off respected.
        assert!(active(None, Some("true")));
        assert!(active(Some("true"), Some("true")));
        assert!(!active(Some("false"), Some("true")));
        assert!(!active(Some("false"), None));
    }
    #[test]
    fn injury_matches_horizons_f32_health_semantics() {
        // Horizon: health > 0 && health < (max_health_attribute as f32), all in f32.
        // Health is stored as f32, so a full bar at a fractional maximum must not be injured.
        for max in [20.3_f64, 20.1, 40.7, 6.3, 1.1, 100.9, 20.000001, 0.1] {
            let full: f32 = max as f32;
            assert!(!injured(full, max), "full at {max}");
            let below = f32::from_bits(full.to_bits() - 1);
            assert!(injured(below, max), "one step below {max}");
        }
        // The maximum rounds down to f32 (20.3 -> 20.299999...) and rounds up (20.30000099).
        let down = 20.3000001_f64;
        assert!((down as f32) as f64 <= down, "rounds down");
        assert!(!injured(down as f32, down));
        let up = 20.30000099_f64;
        assert!((up as f32) as f64 > up, "rounds up");
        assert!(!injured(up as f32, up));
        assert!(injured((up as f32) - 0.000_01, up));
    }
    #[test]
    fn heal_setting_defaults_off() {
        assert!(!heal_enabled(None));
        assert!(!heal_enabled(Some("")));
        assert!(!heal_enabled(Some("1")));
        assert!(!heal_enabled(Some("false")));
        assert!(heal_enabled(Some("true")));
    }
    #[test]
    fn injury_uses_actual_max_and_rejects_invalid_numbers() {
        assert!(injured(19.0, 20.0));
        assert!(!injured(20.0, 20.0));
        assert!(!injured(25.0, 20.0));
        // A raised maximum counts: 20 of 40 is injured, 20 of 20 is not.
        assert!(injured(20.0, 40.0));
        assert!(injured(5.0, 6.0));
        assert!(!injured(0.0, 20.0), "dead");
        assert!(!injured(-1.0, 20.0));
        assert!(!injured(f32::NAN, 20.0));
        assert!(!injured(10.0, f64::NAN));
        assert!(!injured(10.0, 0.0));
        assert!(!injured(10.0, -4.0));
        assert!(!injured(f32::INFINITY, f64::INFINITY));
        assert!(
            !injured(10.0, f64::MAX),
            "a maximum that overflows f32 is invalid"
        );
        assert!(!injured(10.0, f64::INFINITY));
    }
    #[test]
    fn healing_allows_overfill_only_when_enabled_and_not_full() {
        assert!(!eligible(8, 15, false));
        assert!(eligible(8, 15, true));
        assert!(eligible(5, 15, false));
        assert!(!eligible(8, 20, true), "never at a full bar");
        assert!(!eligible(0, 10, true));
        assert!(!eligible(-3, 10, true));
        assert!(!eligible(5, -1, true));
    }
    #[test]
    fn healing_keeps_the_fitting_choice_first_then_the_smallest_top_up() {
        let foods = vec![
            food(1, "minecraft:cooked_beef", 8),
            food(2, "minecraft:bread", 5),
            food(3, "minecraft:apple", 4),
        ];
        // A fitting food still wins, healing or not.
        assert_eq!(choose_healing(&foods, 10, true), choose(&foods, 10));
        assert_eq!(
            choose_healing(&foods, 15, true).unwrap().item,
            "minecraft:bread"
        );
        // Nothing fits at 18/20 (room 2): not injured waits, injured eats the smallest.
        assert_eq!(choose_healing(&foods, 18, false), None);
        assert_eq!(
            choose_healing(&foods, 18, true).unwrap().item,
            "minecraft:apple"
        );
        // Full hunger: never.
        assert_eq!(choose_healing(&foods, 20, true), None);
        // Equal nutrition: lowest slot.
        let tie = vec![
            food(7, "minecraft:apple", 4),
            food(3, "minecraft:carrot", 4),
        ];
        assert_eq!(choose_healing(&tie, 19, true).unwrap().slot, 3);
    }
    #[test]
    fn healing_never_overrides_risky_foods_or_removed_nutrition() {
        let foods = vec![
            food(0, "minecraft:rotten_flesh", 4),
            food(1, "minecraft:golden_apple", 4),
            food(2, "minecraft:bread", 0),
        ];
        assert_eq!(choose_healing(&foods, 18, true), None);
        let ok = vec![
            food(0, "minecraft:rotten_flesh", 1),
            food(5, "minecraft:bread", 5),
        ];
        assert_eq!(choose_healing(&ok, 18, true).unwrap().slot, 5);
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
