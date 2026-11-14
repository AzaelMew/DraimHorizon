// Pure rules for Auto Tools: which held items count as a tool for its optional trigger.
/// Settings key of the optional trigger; absent or anything but "true" means off.
pub const ONLY_HOLDING_KEY: &str = "auto_tools_only_holding_tool";
/// Whether the setting value turns the held-tool trigger on (off by default).
pub fn only_holding(setting: Option<&str>) -> bool {
    setting == Some("true")
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
