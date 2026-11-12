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
pub const HAY_BLOCK: &str = "minecraft:hay_block";
/// Hay bales made per crafting round at most (nine wheat each).
pub const HAY_ROUND: u32 = 4;
/// Whether `name` is something these crops yield: the harvest itself, or hay bales from wheat.
pub fn yields(crops: &[Crop], name: &str) -> bool {
    crops.iter().any(|c| product(*c) == name) || (name == HAY_BLOCK && crops.contains(&Crop::Wheat))
}
/// The inventory slots to sell: every stack of what the crops yield (harvest and hay bales),
/// never the seeds. A yield that is its own seed (carrots, potatoes) keeps `keep` of it for
/// sowing: whole stacks go (a sale takes the stack in hand), the smallest first, while what is
/// left still covers it.
pub fn sell_slots(inventory: &[Stack], crops: &[Crop], keep: u32) -> Vec<usize> {
    let sown = |name: &str| crops.iter().any(|c| c.seed() == name);
    let mut out: Vec<usize> = Vec::new();
    let mut sown_stacks: Vec<(&str, u32, usize)> = Vec::new();
    for (slot, stack) in inventory.iter().enumerate() {
        match stack {
            Some((name, n)) if yields(crops, name) && sown(name) => {
                sown_stacks.push((name, *n, slot))
            }
            Some((name, _)) if yields(crops, name) => out.push(slot),
            _ => {}
        }
    }
    sown_stacks.sort_by_key(|&(name, n, slot)| (name, n, slot));
    let mut left: Vec<(&str, u32)> = Vec::new();
    for &(name, _, _) in &sown_stacks {
        if !left.iter().any(|(n, _)| *n == name) {
            left.push((name, count(inventory, name)));
        }
    }
    for (name, n, slot) in sown_stacks {
        if let Some((_, total)) = left.iter_mut().find(|(item, _)| *item == name)
            && *total >= keep.saturating_add(n)
        {
            *total -= n;
            out.push(slot);
        }
    }
    out.sort_unstable();
    out
}
/// The seeds a composter takes (it turns them into bone meal): wheat and beetroot seeds.
pub const COMPOST_SEEDS: [&str; 2] = ["minecraft:wheat_seeds", "minecraft:beetroot_seeds"];
/// The kind of seeds to put into the composter: the one with most beyond the `keep` kept for
/// sowing (a crop that is not farmed keeps none). `None` when there are no spare seeds.
pub fn compost_seed(inventory: &[Stack], crops: &[Crop], keep: u32) -> Option<&'static str> {
    COMPOST_SEEDS
        .into_iter()
        .map(|seed| {
            let kept = if crops.iter().any(|c| c.seed() == seed) {
                keep
            } else {
                0
            };
            (seed, count(inventory, seed).saturating_sub(kept))
        })
        .filter(|(_, spare)| *spare > 0)
        .max_by_key(|(_, spare)| *spare)
        .map(|(seed, _)| seed)
}
/// Whether the harvest should be taken to the containers now, between two crops: the inventory
/// is (nearly) full, the field is idle, a third of the inventory is harvest, or a few stacks have
/// gathered and the last unload is a while ago. Not after every crop.
pub fn unload_due(inventory: &[Stack], stacks: usize, idle: bool, since_unload_s: u64) -> bool {
    free_slots(inventory) <= 2
        || idle
        || stacks >= UNLOAD_STACKS
        || (stacks >= 3 && since_unload_s >= UNLOAD_EVERY_S)
}
/// Stacks of harvest that make an unload worth the walk at once.
pub const UNLOAD_STACKS: usize = 12;
/// Seconds after which a few stacks are taken away too.
pub const UNLOAD_EVERY_S: u64 = 240;
/// What to craft at a crafting table: the slot of a stack of at least nine wheat and how many
/// bales to make from it this round. `None` when no stack has enough or wheat is not farmed.
pub fn hay_plan(inventory: &[Stack], crops: &[Crop]) -> Option<(usize, u32)> {
    if !crops.contains(&Crop::Wheat) {
        return None;
    }
    inventory
        .iter()
        .enumerate()
        .find_map(|(slot, stack)| match stack {
            Some((name, n)) if name == "minecraft:wheat" && *n >= 9 => {
                Some((slot, (n / 9).min(HAY_ROUND)))
            }
            _ => None,
        })
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
        } else if yields(crops, name) {
            slots.push(slot);
        }
    }
    slots
}
/// Which container a trip goes to: the harvest, the seeds or the bone meal.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Role {
    Harvest,
    Seeds,
    Meal,
}
impl Role {
    pub const ALL: [Role; 3] = [Role::Harvest, Role::Seeds, Role::Meal];
    pub fn index(self) -> usize {
        match self {
            Self::Harvest => 0,
            Self::Seeds => 1,
            Self::Meal => 2,
        }
    }
}
pub const BONE_MEAL: &str = "minecraft:bone_meal";
pub const BONE_BLOCK: &str = "minecraft:bone_block";
/// Bone meal kept at hand: fetched, or crafted from blocks, up to this many.
pub const MEAL_STOCK: u32 = 64;
/// Bone blocks taken out of the container for crafting, at most.
pub const BLOCK_LIMIT: u32 = 64;
/// What goes where after a harvest.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Split {
    /// Into the harvest container.
    pub harvest: Vec<usize>,
    /// Into the seeds container.
    pub seeds: Vec<usize>,
}
/// As `deposit`, but the seeds beyond `keep` go to their own container when `split` is on, and
/// a crop that is its own seed (carrots, potatoes) is shared between the two: each stack goes
/// to the container that has had less so far, so about half ends up in each. Without `split`
/// everything goes to the harvest container, as before.
pub fn deposit_split(inventory: &[Stack], crops: &[Crop], keep: u32, split: bool) -> Split {
    let mut out = Split::default();
    let mut kept: Vec<(&str, u32)> = Vec::new();
    let (mut to_harvest, mut to_seeds) = (0u32, 0u32);
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
            } else if !split {
                out.harvest.push(slot);
            } else if crops.iter().any(|c| product(*c) == name) {
                // Both seed and crop: half and half.
                if to_seeds <= to_harvest {
                    out.seeds.push(slot);
                    to_seeds += n;
                } else {
                    out.harvest.push(slot);
                    to_harvest += n;
                }
            } else {
                out.seeds.push(slot);
            }
        } else if yields(crops, name) {
            out.harvest.push(slot);
        }
    }
    out
}
/// The container slots to take for bone meal: its stacks until the inventory holds `MEAL_STOCK`,
/// then, if that is not enough, bone blocks (at most `BLOCK_LIMIT`, and no more than make up the
/// difference) to be crafted into meal.
pub fn meal_withdraw(container: &[Stack], inventory: &[Stack]) -> Vec<usize> {
    let mut meal = count(inventory, BONE_MEAL);
    let mut blocks = count(inventory, BONE_BLOCK);
    let mut slots = Vec::new();
    for (slot, stack) in container.iter().enumerate() {
        if meal >= MEAL_STOCK {
            break;
        }
        if let Some((name, n)) = stack
            && name == BONE_MEAL
        {
            meal += n;
            slots.push(slot);
        }
    }
    for (slot, stack) in container.iter().enumerate() {
        if meal + 9 * blocks >= MEAL_STOCK || blocks >= BLOCK_LIMIT {
            break;
        }
        if let Some((name, n)) = stack
            && name == BONE_BLOCK
        {
            blocks += n;
            slots.push(slot);
        }
    }
    slots
}
/// What to craft in the inventory: the slot of the first stack of bone blocks and how many of
/// them make the meal up to `MEAL_STOCK` (nine meal a block). `None` when there is enough meal
/// or no blocks.
pub fn craft_plan(inventory: &[Stack]) -> Option<(usize, u32)> {
    let meal = count(inventory, BONE_MEAL);
    if meal >= MEAL_STOCK {
        return None;
    }
    let (slot, n) = inventory
        .iter()
        .enumerate()
        .find_map(|(slot, stack)| match stack {
            Some((name, n)) if name == BONE_BLOCK => Some((slot, *n)),
            _ => None,
        })?;
    Some((slot, n.min((MEAL_STOCK - meal).div_ceil(9))))
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
    fn carrots_and_potatoes_are_shared_between_the_two_containers() {
        use super::super::claim_borders_farming::Crop;
        let item = |name: &str, n: u32| Some((name.to_string(), n));
        let inventory = vec![
            item("minecraft:carrot", 64), // kept at hand for replanting
            item("minecraft:carrot", 64),
            item("minecraft:carrot", 64),
            item("minecraft:carrot", 64),
            item("minecraft:carrot", 64),
            item("minecraft:wheat", 40),
            item("minecraft:wheat_seeds", 64),
            item("minecraft:wheat_seeds", 20),
        ];
        let split = deposit_split(&inventory, &[Crop::Carrots, Crop::Wheat], 64, true);
        // Four carrot stacks over the kept one: two in each container.
        assert_eq!(split.seeds.iter().filter(|s| **s <= 4).count(), 2);
        assert_eq!(
            split
                .harvest
                .iter()
                .filter(|s| (1..=4).contains(*s))
                .count(),
            2
        );
        // Wheat is the harvest; its seeds beyond the kept stack go with the seeds.
        assert!(split.harvest.contains(&5));
        assert_eq!(split.seeds.last(), Some(&7));
        // Without a seeds container everything is one pile, as before.
        let one = deposit_split(&inventory, &[Crop::Carrots, Crop::Wheat], 64, false);
        assert_eq!(one.seeds, Vec::<usize>::new());
        assert_eq!(
            one.harvest,
            deposit(&inventory, &[Crop::Carrots, Crop::Wheat], 64)
        );
    }
    #[test]
    fn bone_meal_is_taken_as_meal_or_as_a_limited_number_of_blocks() {
        let item = |name: &str, n: u32| Some((name.to_string(), n));
        let container = vec![
            item("minecraft:bone_block", 64),
            item("minecraft:bone_meal", 30),
            item("minecraft:bone_block", 64),
            None,
        ];
        // Meal first (30), then only as many blocks as make up the rest: one stack, not two.
        assert_eq!(meal_withdraw(&container, &[None, None]), vec![1, 0]);
        // Meal enough: nothing.
        assert!(meal_withdraw(&container, &[item("minecraft:bone_meal", 64)]).is_empty());
        // No meal in the container: blocks, never beyond 64 of them.
        let blocks_only = vec![
            item("minecraft:bone_block", 64),
            item("minecraft:bone_block", 64),
        ];
        assert_eq!(meal_withdraw(&blocks_only, &[None]), vec![0]);
        // Crafting: 64 meal at nine a block is eight blocks from empty; one block tops up 40.
        let held = vec![
            item("minecraft:bone_block", 20),
            item("minecraft:bone_meal", 0),
        ];
        assert_eq!(craft_plan(&held), Some((0, 8)));
        let top_up = vec![
            item("minecraft:bone_meal", 56),
            item("minecraft:bone_block", 5),
        ];
        assert_eq!(craft_plan(&top_up), Some((1, 1)));
        assert_eq!(
            craft_plan(&[
                item("minecraft:bone_meal", 64),
                item("minecraft:bone_block", 5)
            ]),
            None
        );
        assert_eq!(craft_plan(&[item("minecraft:stone", 5)]), None);
    }
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
    fn hay_is_crafted_from_wheat_stacks_and_deposited() {
        use super::super::claim_borders_farming::Crop;
        let item = |name: &str, n: u32| Some((name.to_string(), n));
        let inventory = vec![item("minecraft:wheat", 5), item("minecraft:wheat", 40)];
        assert_eq!(hay_plan(&inventory, &[Crop::Wheat]), Some((1, 4)));
        assert_eq!(hay_plan(&inventory, &[Crop::Carrots]), None);
        assert_eq!(
            hay_plan(&[item("minecraft:wheat", 8)], &[Crop::Wheat]),
            None
        );
        let mixed = vec![
            item("minecraft:wheat_seeds", 9),
            item("minecraft:wheat", 3),
            item(HAY_BLOCK, 2),
        ];
        assert_eq!(sell_slots(&mixed, &[Crop::Wheat], KEEP_SEEDS), vec![1, 2]);
        // Carrots are their own seed: enough stays for sowing, whole stacks are sold.
        let carrots = vec![
            item("minecraft:carrot", 64),
            item("minecraft:carrot", 20),
            item("minecraft:carrot", 64),
            item("minecraft:potato", 64),
        ];
        assert_eq!(
            sell_slots(&carrots, &[Crop::Carrots], KEEP_SEEDS),
            vec![0, 1]
        );
        assert_eq!(
            sell_slots(&carrots[..1], &[Crop::Carrots], KEEP_SEEDS),
            Vec::<usize>::new()
        );
        // Potatoes not farmed here are not this farm's to sell.
        assert!(!sell_slots(&carrots, &[Crop::Carrots], KEEP_SEEDS).contains(&3));
        let bales = vec![item(HAY_BLOCK, 3)];
        assert_eq!(deposit(&bales, &[Crop::Wheat], KEEP_SEEDS), vec![0]);
        assert!(deposit(&bales, &[Crop::Carrots], KEEP_SEEDS).is_empty());
    }
    #[test]
    fn spare_seeds_go_to_the_composter_and_unloads_are_not_after_every_crop() {
        use super::super::claim_borders_farming::Crop;
        let item = |name: &str, n: u32| Some((name.to_string(), n));
        let mut inventory = vec![
            item("minecraft:wheat_seeds", 64),
            item("minecraft:wheat_seeds", 30),
            item("minecraft:beetroot_seeds", 10),
        ];
        // Wheat is farmed: 64 seeds stay for sowing, 30 are spare; beetroot is not: all spare.
        assert_eq!(
            compost_seed(&inventory, &[Crop::Wheat], KEEP_SEEDS),
            Some("minecraft:wheat_seeds")
        );
        assert_eq!(
            compost_seed(&inventory, &[Crop::Wheat, Crop::Beetroot], KEEP_SEEDS),
            Some("minecraft:wheat_seeds")
        );
        inventory.truncate(1);
        assert_eq!(compost_seed(&inventory, &[Crop::Wheat], KEEP_SEEDS), None);
        // Unloads: not for two stacks right after the last one; yes for many, when idle, when
        // nearly full, or for a few after a while.
        inventory.resize(36, None);
        assert!(!unload_due(&inventory, 2, false, 10));
        assert!(!unload_due(&inventory, 3, false, 60));
        assert!(unload_due(&inventory, 3, false, UNLOAD_EVERY_S));
        assert!(unload_due(&inventory, UNLOAD_STACKS, false, 0));
        assert!(unload_due(&inventory, 0, true, 0));
        let full = vec![item("minecraft:dirt", 64); 35];
        assert!(unload_due(&full, 0, false, 0));
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
