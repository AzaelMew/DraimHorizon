// Client-side protection for decorative cave vines, independent of berry state.
pub fn cancel(enabled: bool, sneaking: bool, button: i32, block: &str) -> bool {
    enabled
        && !sneaking
        && matches!(button, 0 | 1)
        && matches!(block, "minecraft:cave_vines" | "minecraft:cave_vines_plant")
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn protects_both_sections_and_allows_deliberate_bypass() {
        for block in ["minecraft:cave_vines", "minecraft:cave_vines_plant"] {
            for button in [0, 1] {
                assert!(cancel(true, false, button, block));
                assert!(!cancel(true, true, button, block));
                assert!(!cancel(false, false, button, block));
            }
            assert!(!cancel(true, false, 2, block));
        }
        for block in [
            "minecraft:stone",
            "minecraft:vine",
            "minecraft:weeping_vines",
            "minecraft:air",
        ] {
            for button in [0, 1] {
                assert!(!cancel(true, false, button, block));
            }
        }
    }
}
