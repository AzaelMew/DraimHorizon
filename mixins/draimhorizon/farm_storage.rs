// Pure plans for the farm's storage container: what goes into it after a harvest and which
// seeds come out of it. Slots are game slots: 0..9 the hotbar, 9..36 the main inventory.
use super::claim_borders_farming::Crop;
/// An item id and how many of it a slot holds; `None` for an empty slot.
pub type Stack = Option<(String, u32)>;
/// Seeds kept in the inventory for replanting; everything beyond goes to storage.
pub const KEEP_SEEDS: u32 = 64;
/// Seeds taken out of storage when the inventory has run out of them.
pub const TAKE_SEEDS: u32 = 128;
/// Containers with a menu that holds items freely: chests, barrels and shulker boxes.
pub fn is_container(name: &str) -> bool {
    matches!(
        name,
        "minecraft:chest" | "minecraft:trapped_chest" | "minecraft:barrel"
    ) || name.ends_with("shulker_box")
}
/// The menu types of those containers (`generic_9x1` to `generic_9x6` and the shulker box).
pub fn is_container_menu(menu: &str) -> bool {
    menu == "minecraft:shulker_box"
        || menu
            .strip_prefix("minecraft:generic_9x")
            .is_some_and(|rows| matches!(rows, "1" | "2" | "3" | "4" | "5" | "6"))
}
/// What the crop yields when harvested.
pub fn product(crop: Crop) -> &'static str {
    match crop {
        Crop::Wheat => "minecraft:wheat",
        Crop::Carrots => "minecraft:carrot",
        Crop::Potatoes => "minecraft:potato",
        Crop::Beetroot => "minecraft:beetroot",
    }
}
/// The inventory slots to move into a container after mining: everything but what the miner
/// still needs. `needed` names what stays whatever the amount (tools, food, a totem), and
/// `building` marks blocks kept to build with, up to `keep` of each kind, counted from the hotbar
/// on; a stack that would take that kind over the limit goes into the container.
pub fn deposit_mined(
    inventory: &[Stack],
    needed: &dyn Fn(&str) -> bool,
    building: &dyn Fn(&str) -> bool,
    keep: u32,
) -> Vec<usize> {
    let mut kept: Vec<(&str, u32)> = Vec::new();
    let mut slots = Vec::new();
    for (slot, stack) in inventory.iter().enumerate() {
        let Some((name, n)) = stack else {
            continue;
        };
        if needed(name) {
            continue;
        }
        if building(name) {
            match kept.iter_mut().find(|(item, _)| item == name) {
                Some((_, total)) if *total + n <= keep => {
                    *total += n;
                    continue;
                }
                Some(_) => {}
                None if *n <= keep => {
                    kept.push((name, *n));
                    continue;
                }
                None => {}
            }
        }
        slots.push(slot);
    }
    slots
}
pub fn free_slots(inventory: &[Stack]) -> usize {
    inventory.iter().filter(|s| s.is_none()).count()
}
pub fn count(inventory: &[Stack], item: &str) -> u32 {
    inventory
        .iter()
        .flatten()
        .filter(|(name, _)| name == item)
        .map(|(_, n)| *n)
        .sum()
}
/// The inventory slots to move into the container: every stack of a selected crop's harvest, and
/// the seeds beyond `keep` of each kind (counted from the hotbar on, so the kept seeds stay at
/// hand). For a crop that is its own seed (carrots, potatoes) the same rule applies to the item.
pub fn deposit(inventory: &[Stack], crops: &[Crop], keep: u32) -> Vec<usize> {
    let mut kept: Vec<(&str, u32)> = Vec::new();
    let mut slots = Vec::new();
    for (slot, stack) in inventory.iter().enumerate() {
        let Some((name, n)) = stack else {
            continue;
        };
        if crops.iter().any(|c| c.seed() == name) {
            let at = match kept.iter().position(|(item, _)| *item == name) {
                Some(at) => at,
                None => {
                    kept.push((name, 0));
                    kept.len() - 1
                }
            };
            if kept[at].1 < keep {
                kept[at].1 += n;
            } else {
                slots.push(slot);
            }
        } else if crops.iter().any(|c| product(*c) == name) {
            slots.push(slot);
        }
    }
    slots
}
/// The container slots to take so that the inventory holds `want` of `seed`.
pub fn withdraw(container: &[Stack], inventory: &[Stack], seed: &str, want: u32) -> Vec<usize> {
    let mut have = count(inventory, seed);
    let mut slots = Vec::new();
    for (slot, stack) in container.iter().enumerate() {
        if have >= want {
            break;
        }
        if let Some((name, n)) = stack
            && name == seed
        {
            have += n;
            slots.push(slot);
        }
    }
    slots
}
#[cfg(test)]
mod tests {
    #[test]
    fn mined_loot_goes_to_the_container_except_what_the_miner_needs() {
        let item = |name: &str, n: u32| Some((name.to_string(), n));
        let inventory = vec![
            item("minecraft:diamond_pickaxe", 1),
            item("minecraft:sand", 64),
            item("minecraft:bread", 12),
            item("minecraft:sand", 64),
            None,
            item("minecraft:gravel", 20),
            item("minecraft:sand", 30),
        ];
        let needed = |name: &str| name.ends_with("_pickaxe") || name == "minecraft:bread";
        let sand = |name: &str| name == "minecraft:sand";
        // One stack of sand is kept to build with; the rest, and the gravel, go.
        assert_eq!(deposit_mined(&inventory, &needed, &sand, 64), vec![3, 5, 6]);
        // Nothing is kept when nothing is for building.
        assert_eq!(
            deposit_mined(&inventory, &needed, &|_| false, 64),
            vec![1, 3, 5, 6]
        );
        // The tool and the food alone: nothing to move.
        assert!(deposit_mined(&inventory[..1], &needed, &sand, 64).is_empty());
    }
    use super::*;
    fn stack(item: &str, n: u32) -> Stack {
        Some((item.into(), n))
    }
    #[test]
    fn only_free_containers_count_as_storage() {
        assert!(is_container("minecraft:chest"));
        assert!(is_container("minecraft:barrel"));
        assert!(is_container("minecraft:blue_shulker_box"));
        assert!(is_container("minecraft:shulker_box"));
        // A furnace or a hopper has a menu too, but it is not a place to keep a harvest.
        assert!(!is_container("minecraft:furnace"));
        assert!(!is_container("minecraft:hopper"));
        assert!(is_container_menu("minecraft:generic_9x3"));
        assert!(is_container_menu("minecraft:shulker_box"));
        assert!(!is_container_menu("minecraft:generic_3x3"));
        assert!(!is_container_menu("inventory"));
    }
    #[test]
    fn a_harvest_goes_in_and_a_stack_of_seeds_stays() {
        let mut inventory: Vec<Stack> = vec![None; 36];
        inventory[0] = stack("minecraft:diamond_pickaxe", 1);
        inventory[1] = stack("minecraft:wheat_seeds", 64);
        inventory[9] = stack("minecraft:wheat", 64);
        inventory[10] = stack("minecraft:wheat_seeds", 40);
        inventory[11] = stack("minecraft:wheat", 17);
        inventory[12] = stack("minecraft:cobblestone", 64);
        // The hotbar seeds are kept; the extra seeds and all wheat go; other items stay.
        assert_eq!(
            deposit(&inventory, &[Crop::Wheat], KEEP_SEEDS),
            vec![9, 10, 11]
        );
        // Carrots are their own seed: 64 stay, the rest goes.
        let mut carrots: Vec<Stack> = vec![None; 36];
        carrots[3] = stack("minecraft:carrot", 64);
        carrots[9] = stack("minecraft:carrot", 64);
        carrots[10] = stack("minecraft:carrot", 30);
        assert_eq!(deposit(&carrots, &[Crop::Carrots], KEEP_SEEDS), vec![9, 10]);
        assert!(deposit(&vec![None; 36], &[Crop::Beetroot], KEEP_SEEDS).is_empty());
        // Two crops: each keeps its own stack of seeds, the other harvest goes as well.
        let mut two: Vec<Stack> = vec![None; 36];
        two[0] = stack("minecraft:wheat_seeds", 64);
        two[1] = stack("minecraft:beetroot_seeds", 64);
        two[9] = stack("minecraft:wheat_seeds", 10);
        two[10] = stack("minecraft:beetroot_seeds", 10);
        two[11] = stack("minecraft:beetroot", 5);
        assert_eq!(
            deposit(&two, &[Crop::Wheat, Crop::Beetroot], KEEP_SEEDS),
            vec![9, 10, 11]
        );
        // A crop that is not selected is left alone.
        assert_eq!(deposit(&two, &[Crop::Wheat], KEEP_SEEDS), vec![9]);
    }
    #[test]
    fn seeds_are_taken_until_the_inventory_has_enough() {
        let chest: Vec<Stack> = vec![
            stack("minecraft:wheat", 64),
            stack("minecraft:wheat_seeds", 64),
            None,
            stack("minecraft:wheat_seeds", 64),
            stack("minecraft:wheat_seeds", 12),
        ];
        let empty: Vec<Stack> = vec![None; 36];
        assert_eq!(
            withdraw(&chest, &empty, "minecraft:wheat_seeds", 128),
            vec![1, 3]
        );
        // Seeds already carried count towards the amount.
        let mut some = empty.clone();
        some[0] = stack("minecraft:wheat_seeds", 100);
        assert_eq!(
            withdraw(&chest, &some, "minecraft:wheat_seeds", 128),
            vec![1]
        );
        let mut enough = empty;
        enough[0] = stack("minecraft:wheat_seeds", 128);
        assert!(withdraw(&chest, &enough, "minecraft:wheat_seeds", 128).is_empty());
        assert!(withdraw(&chest, &vec![None; 36], "minecraft:beetroot_seeds", 64).is_empty());
    }
    #[test]
    fn free_slots_and_counts_read_the_inventory() {
        let mut inventory: Vec<Stack> = vec![None; 36];
        inventory[4] = stack("minecraft:wheat", 10);
        inventory[20] = stack("minecraft:wheat", 5);
        assert_eq!(free_slots(&inventory), 34);
        assert_eq!(count(&inventory, "minecraft:wheat"), 15);
    }
}
