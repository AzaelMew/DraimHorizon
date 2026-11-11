// Client-side protection for decorative cave vines, independent of berry state.
pub fn cancel(enabled: bool, sneaking: bool, block: &str) -> bool {
    enabled && !sneaking && matches!(block, "minecraft:cave_vines" | "minecraft:cave_vines_plant")
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn protects_both_sections_and_allows_deliberate_bypass() {
        for block in ["minecraft:cave_vines", "minecraft:cave_vines_plant"] {
            assert!(cancel(true, false, block));
            assert!(!cancel(true, true, block));
            assert!(!cancel(false, false, block));
        }
        for block in [
            "minecraft:stone",
            "minecraft:vine",
            "minecraft:weeping_vines",
            "minecraft:air",
        ] {
            assert!(!cancel(true, false, block));
        }
    }
}
