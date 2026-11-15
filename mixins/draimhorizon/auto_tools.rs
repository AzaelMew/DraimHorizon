// Pure rules for Auto Tools: which held items count as a tool for its optional trigger.
/// Settings key of the optional trigger; absent or anything but "true" means off.
pub const ONLY_HOLDING_KEY: &str = "auto_tools_only_holding_tool";
/// Whether the setting value turns the held-tool trigger on (off by default).
pub fn only_holding(setting: Option<&str>) -> bool {
    setting == Some("true")
}
/// Settings key of "Allow from inventory"; absent or anything but "true" means off.
pub const FROM_INVENTORY_KEY: &str = "auto_tools_from_inventory";
/// Whether the setting value lets Auto Tools take tools from the main inventory (off by default).
pub fn from_inventory(setting: Option<&str>) -> bool {
    setting == Some("true")
}
/// Hotbar slots, and hotbar plus main inventory.
pub const HOTBAR_SLOTS: usize = 9;
pub const ALL_SLOTS: usize = 36;
/// How many slots Auto Tools searches: the hotbar, or with the setting the inventory too. The
/// domain is cut before ranking, so a tool outside it never outranks a usable one inside.
pub fn search_slots(from_inventory: bool) -> usize {
    if from_inventory {
        ALL_SLOTS
    } else {
        HOTBAR_SLOTS
    }
}
/// Slot to use among the first `limit` slots: the selected one first, then in slot order; a
/// fresh tool before a worn one, within each the earlier one.
pub fn rank_slot(
    selected: usize,
    limit: usize,
    fits: impl Fn(usize) -> bool,
    fresh: impl Fn(usize) -> bool,
) -> Option<usize> {
    let order = || {
        std::iter::once(selected)
            .filter(|slot| *slot < limit)
            .chain(0..limit)
    };
    order()
        .find(|slot| fits(*slot) && fresh(*slot))
        .or_else(|| order().find(|slot| fits(*slot)))
}
/// Minimum gap between two inventory swaps, in milliseconds.
pub const SWAP_GAP_MS: u64 = 250;
/// Whether another inventory swap may be sent, given the time since the last one.
pub fn swap_due(since_last_ms: Option<u64>) -> bool {
    since_last_ms.is_none_or(|ms| ms >= SWAP_GAP_MS)
}
/// Whether a main-hand stack is a pickaxe, axe or shovel. Only the item category counts: wear
/// and the block under the crosshair are not looked at.
pub fn is_held_tool(item: &str, present: bool) -> bool {
    present
        && ["_pickaxe", "_axe", "_shovel"]
            .iter()
            .any(|kind| item.ends_with(kind))
}
/// Whether Auto Tools may switch: always without the trigger, else only holding a tool.
pub fn may_switch(only_holding: bool, held: Option<(&str, bool)>) -> bool {
    !only_holding || held.is_some_and(|(item, present)| is_held_tool(item, present))
}
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn inventory_setting_defaults_off() {
        assert!(!from_inventory(None));
        assert!(!from_inventory(Some("")));
        assert!(!from_inventory(Some("on")));
        assert!(!from_inventory(Some("false")));
        assert!(from_inventory(Some("true")));
        assert_eq!(search_slots(false), 9);
        assert_eq!(search_slots(true), 36);
    }
    // Slot 3: worn hotbar tool; slot 20: fresh inventory tool.
    fn worn_hotbar_fresh_inventory(limit: usize) -> Option<usize> {
        rank_slot(0, limit, |s| s == 3 || s == 20, |s| s == 20)
    }
    #[test]
    fn hotbar_only_keeps_a_usable_worn_hotbar_tool_over_a_fresh_inventory_one() {
        assert_eq!(worn_hotbar_fresh_inventory(search_slots(false)), Some(3));
    }
    #[test]
    fn inventory_toggle_prefers_the_fresh_inventory_tool() {
        assert_eq!(worn_hotbar_fresh_inventory(search_slots(true)), Some(20));
    }
    #[test]
    fn only_inventory_tool_is_invisible_to_hotbar_only() {
        let only_inventory = |limit| rank_slot(0, limit, |s| s == 20, |_| true);
        assert_eq!(only_inventory(search_slots(false)), None);
        assert_eq!(only_inventory(search_slots(true)), Some(20));
    }
    #[test]
    fn selected_slot_and_ties_rank_stably() {
        // Selected usable and fresh wins over earlier slots.
        assert_eq!(rank_slot(5, 9, |s| s == 2 || s == 5, |_| true), Some(5));
        // Both fresh, selected not usable: lowest slot.
        assert_eq!(rank_slot(7, 36, |s| s == 4 || s == 12, |_| true), Some(4));
        // Selected worn, another hotbar slot fresh: the fresh one.
        assert_eq!(rank_slot(5, 9, |s| s == 2 || s == 5, |s| s == 2), Some(2));
        // All worn: selected stays.
        assert_eq!(rank_slot(5, 9, |s| s == 2 || s == 5, |_| false), Some(5));
        assert_eq!(rank_slot(5, 9, |_| false, |_| true), None);
    }
    #[test]
    fn swaps_are_rate_limited() {
        assert!(swap_due(None));
        assert!(!swap_due(Some(0)));
        assert!(!swap_due(Some(SWAP_GAP_MS - 1)));
        assert!(swap_due(Some(SWAP_GAP_MS)));
    }
    #[test]
    fn setting_defaults_off() {
        assert!(!only_holding(None));
        assert!(!only_holding(Some("")));
        assert!(!only_holding(Some("yes")));
        assert!(!only_holding(Some("false")));
        assert!(only_holding(Some("true")));
    }
    #[test]
    fn every_material_and_category_qualifies() {
        for material in [
            "wooden",
            "stone",
            "copper",
            "iron",
            "golden",
            "diamond",
            "netherite",
        ] {
            for kind in ["pickaxe", "axe", "shovel"] {
                let id = format!("minecraft:{material}_{kind}");
                assert!(is_held_tool(&id, true), "{id}");
                assert!(!is_held_tool(&id, false), "{id} with an empty stack");
            }
        }
    }
    #[test]
    fn other_items_never_qualify() {
        for id in [
            "minecraft:diamond_sword",
            "minecraft:iron_hoe",
            "minecraft:shears",
            "minecraft:bread",
            "minecraft:air",
            "minecraft:stone",
            "minecraft:mace",
            "minecraft:trident",
        ] {
            assert!(!is_held_tool(id, true), "{id}");
        }
    }
    #[test]
    fn gate_off_keeps_old_behaviour() {
        assert!(may_switch(false, None));
        assert!(may_switch(false, Some(("minecraft:diamond_sword", true))));
    }
    #[test]
    fn gate_on_needs_a_held_tool() {
        assert!(!may_switch(true, None));
        assert!(!may_switch(true, Some(("minecraft:diamond_sword", true))));
        assert!(!may_switch(true, Some(("minecraft:iron_pickaxe", false))));
        assert!(may_switch(true, Some(("minecraft:iron_pickaxe", true))));
        // The held tool need not suit the block or be fresh: only its category counts.
        assert!(may_switch(true, Some(("minecraft:wooden_shovel", true))));
    }
}
