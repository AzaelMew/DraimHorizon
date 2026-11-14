// What the Baritone tab's Tunnel, Bridge and Drain sections ask the port's builder to build (MIT):
// each is a schematic made of the port's own pieces (`FillSchematic`, `CompositeSchematic`) or, for
// the drain patterns, one of ours. The builder then plans the walking, the breaking and the
// placing. Pure: the boxes and what belongs in them are tested without a game.
use super::baritone_schematic::{BlockSpec, CompositeSchematic, FillSchematic, Schematic};
use super::claim_borders_construction::{self as construction, Direction};
use super::claim_borders_draining as draining;
use std::rc::Rc;

type Pos = [i32; 3];

/// The cells of a straight strip that starts at the block in front of `at` and runs `length`
/// blocks along `direction`, `width` wide, centred on the line the player stands on (an even
/// width leans to the left of the way). Returns the lowest corner and the size along x, z.
fn strip(at: Pos, direction: Direction, length: i32, width: i32) -> (Pos, [i32; 2]) {
    let [dx, dz] = direction.vector();
    // Across the way: the larger share of an even width goes to the left.
    let left = width / 2;
    let right = (width - 1) / 2;
    let (x0, x1, z0, z1) = if dx != 0 {
        let (a, b) = (at[0] + dx, at[0] + dx * length);
        // Facing east the left is north (-z); facing west it is south (+z).
        let (lo, hi) = if dx > 0 {
            (at[2] - left, at[2] + right)
        } else {
            (at[2] - right, at[2] + left)
        };
        (a.min(b), a.max(b), lo, hi)
    } else {
        let (a, b) = (at[2] + dz, at[2] + dz * length);
        // Facing south the left is east (+x); facing north it is west (-x).
        let (lo, hi) = if dz > 0 {
            (at[0] - right, at[0] + left)
        } else {
            (at[0] - left, at[0] + right)
        };
        (lo, hi, a.min(b), a.max(b))
    };
    ([x0, at[1], z0], [x1 - x0 + 1, z1 - z0 + 1])
}

/// A tunnel `length` long, `width` wide and `height` high, from the player's feet level on, with
/// a floor under it when `floor` names a block: the lowest corner of the box (the floor's level)
/// and the schematic.
pub fn tunnel(
    at: Pos,
    direction: Direction,
    length: i32,
    width: i32,
    height: i32,
    floor: Option<&str>,
) -> (Pos, Rc<dyn Schematic>) {
    let (corner, [sx, sz]) = strip(at, direction, length, width);
    let air = FillSchematic {
        size: [sx, height, sz],
        spec: BlockSpec::air(),
    };
    match floor {
        Some(block) => {
            let layer = FillSchematic {
                size: [sx, 1, sz],
                spec: BlockSpec::named(block),
            };
            let parts: Vec<(Pos, Rc<dyn Schematic>)> =
                vec![([0, 0, 0], Rc::new(layer)), ([0, 1, 0], Rc::new(air))];
            (
                [corner[0], corner[1] - 1, corner[2]],
                Rc::new(CompositeSchematic {
                    size: [sx, height + 1, sz],
                    parts,
                    name: "tunnel".into(),
                }),
            )
        }
        None => (corner, Rc::new(air)),
    }
}

/// A floor `length` long and `width` wide of `block`, level with the one under the player's feet,
/// from the block in front of the player on.
pub fn bridge(
    at: Pos,
    direction: Direction,
    length: i32,
    width: i32,
    block: &str,
) -> (Pos, Rc<dyn Schematic>) {
    let (corner, [sx, sz]) = strip(at, direction, length, width);
    (
        [corner[0], corner[1] - 1, corner[2]],
        Rc::new(FillSchematic {
            size: [sx, 1, sz],
            spec: BlockSpec::named(block),
        }),
    )
}

/// A pond filled with `block` in the pattern of `config` around `anchor` (where the player
/// stands): only water (and seagrass) cells of the columns the pattern takes, down to
/// `draining::DEPTH` below the player's feet, are asked for a block; nothing else is touched, so no
/// ordinary block is left standing in the water.
pub struct DrainSchematic {
    config: draining::Config,
    anchor: Pos,
    corner: Pos,
    size: Pos,
    block: BlockSpec,
}
impl Schematic for DrainSchematic {
    fn size(&self) -> [i32; 3] {
        self.size
    }
    fn desired(&self, p: [i32; 3], current: &str) -> Option<BlockSpec> {
        let at = [
            self.corner[0] + p[0],
            self.corner[1] + p[1],
            self.corner[2] + p[2],
        ];
        let wet = draining::replaceable(current);
        (wet && draining::inside(self.config, self.anchor, at)
            && draining::in_pattern(self.config, self.anchor, [at[0], at[2]]))
        .then(|| self.block.clone())
    }
    fn label(&self) -> String {
        "drain".into()
    }
}
pub fn drain(anchor: Pos, config: draining::Config, block: &str) -> (Pos, Rc<dyn Schematic>) {
    let r = i32::from(config.radius);
    let corner = [anchor[0] - r, anchor[1] - draining::DEPTH, anchor[2] - r];
    (
        corner,
        Rc::new(DrainSchematic {
            config,
            anchor,
            corner,
            size: [2 * r + 1, draining::DEPTH + 1, 2 * r + 1],
            block: BlockSpec::named(block),
        }),
    )
}

/// The block a floor or a bridge is built of, from what the inventory holds: the cheap, plentiful
/// ones first (`construction::rank`), only blocks that are safe to spend (`construction::material`).
pub fn floor_block<'a>(inventory: impl IntoIterator<Item = &'a str>) -> Option<String> {
    inventory
        .into_iter()
        .filter(|n| construction::material(n))
        .min_by_key(|n| (construction::rank(n), n.to_string()))
        .map(str::to_owned)
}

/// The block a drain drops: sand, red sand or gravel, in that order of preference, from what is carried.
pub fn drain_block<'a>(inventory: impl IntoIterator<Item = &'a str> + Clone) -> Option<String> {
    ["minecraft:sand", "minecraft:red_sand", "minecraft:gravel"]
        .into_iter()
        .find(|want| inventory.clone().into_iter().any(|n| n == *want))
        .map(str::to_owned)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn wants(s: &dyn Schematic, corner: Pos, world: Pos, current: &str) -> Option<String> {
        let p = [
            world[0] - corner[0],
            world[1] - corner[1],
            world[2] - corner[2],
        ];
        let size = s.size();
        assert!(
            (0..3).all(|i| p[i] >= 0 && p[i] < size[i]),
            "{world:?} is outside the box"
        );
        s.desired(p, current).map(|b| b.name)
    }

    #[test]
    fn a_tunnel_box_runs_ahead_of_the_player_and_is_centred_on_their_column() {
        // East from (10, 64, 5): cells x = 11..=14, z = 4..=6 for a width of 3.
        let (corner, s) = tunnel([10, 64, 5], Direction::East, 4, 3, 2, None);
        assert_eq!(corner, [11, 64, 4]);
        assert_eq!(s.size(), [4, 2, 3]);
        // North from the same place: z = 4 down to 1, x = 9..=11.
        let (corner, s) = tunnel([10, 64, 5], Direction::North, 4, 3, 2, None);
        assert_eq!(corner, [9, 64, 1]);
        assert_eq!(s.size(), [3, 2, 4]);
        // West and south mirror them.
        let (corner, _) = tunnel([10, 64, 5], Direction::West, 4, 1, 2, None);
        assert_eq!(corner, [6, 64, 5]);
        let (corner, s) = tunnel([10, 64, 5], Direction::South, 4, 1, 2, None);
        assert_eq!((corner, s.size()), ([10, 64, 6], [1, 2, 4]));
        // An even width leans to the left of the way: east, left is north.
        let (corner, s) = tunnel([0, 64, 0], Direction::East, 2, 2, 2, None);
        assert_eq!((corner, s.size()), ([1, 64, -1], [2, 2, 2]));
        let (corner, _) = tunnel([0, 64, 0], Direction::West, 2, 2, 2, None);
        assert_eq!(corner, [-2, 64, 0]);
    }

    #[test]
    fn a_tunnel_with_a_floor_clears_air_above_a_layer_of_the_block() {
        let (corner, s) = tunnel(
            [10, 64, 5],
            Direction::East,
            4,
            3,
            2,
            Some("minecraft:cobblestone"),
        );
        // The box starts one lower, for the floor.
        assert_eq!(corner, [11, 63, 4]);
        assert_eq!(s.size(), [4, 3, 3]);
        for x in 11..=14 {
            for z in 4..=6 {
                assert_eq!(
                    wants(&*s, corner, [x, 63, z], "minecraft:air").as_deref(),
                    Some("minecraft:cobblestone")
                );
                for y in 64..=65 {
                    assert_eq!(
                        wants(&*s, corner, [x, y, z], "minecraft:stone").as_deref(),
                        Some("minecraft:air")
                    );
                }
            }
        }
        // Without a floor the box is just the air.
        let (corner, s) = tunnel([10, 64, 5], Direction::East, 4, 3, 2, None);
        assert_eq!(
            wants(&*s, corner, [11, 64, 4], "minecraft:stone").as_deref(),
            Some("minecraft:air")
        );
    }

    #[test]
    fn a_bridge_is_one_layer_under_the_feet_ahead_of_the_player() {
        let (corner, s) = bridge([0, 70, 0], Direction::South, 5, 2, "minecraft:stone");
        // South: z = 1..=5; width 2 leans left (east is left of south): x = 0..=1.
        assert_eq!((corner, s.size()), ([0, 69, 1], [2, 1, 5]));
        assert_eq!(
            wants(&*s, corner, [1, 69, 5], "minecraft:air").as_deref(),
            Some("minecraft:stone")
        );
        // The block under the player is not part of it.
        let inside = |w: Pos| (0..3).all(|i| w[i] >= corner[i] && w[i] < corner[i] + s.size()[i]);
        assert!(!inside([0, 69, 0]));
    }

    #[test]
    fn a_drain_asks_for_a_block_only_in_water_in_the_columns_of_its_pattern() {
        let anchor = [0, 64, 0];
        for (pattern, spacing) in [
            (draining::Pattern::Fill, 4u8),
            (draining::Pattern::Lines, 3),
            (draining::Pattern::Grid, 3),
        ] {
            let config = draining::Config {
                radius: 6,
                pattern,
                spacing,
            };
            let (corner, s) = drain(anchor, config, "minecraft:sand");
            assert_eq!(corner, [-6, 64 - draining::DEPTH, -6]);
            assert_eq!(s.size(), [13, draining::DEPTH + 1, 13]);
            for x in -6..=6 {
                for z in -6..=6 {
                    let at = [x, 60, z];
                    let water = wants(&*s, corner, at, "minecraft:water");
                    // Never on anything that is not water (stone, air, a boat of leaves).
                    for solid in ["minecraft:stone", "minecraft:air", "minecraft:oak_leaves"] {
                        assert_eq!(wants(&*s, corner, at, solid), None);
                    }
                    assert_eq!(
                        water.is_some(),
                        draining::in_pattern(config, anchor, [x, z]),
                        "{pattern:?} at {x},{z}"
                    );
                    if let Some(name) = water.as_deref() {
                        assert_eq!(name, "minecraft:sand");
                    }
                    // Seagrass is replaced like water; kelp is not.
                    assert_eq!(
                        wants(&*s, corner, at, "minecraft:seagrass").is_some(),
                        water.is_some()
                    );
                    assert_eq!(wants(&*s, corner, at, "minecraft:kelp"), None);
                }
            }
        }
        // A fill takes every water column; lines take rows, a grid rows and columns.
        let count = |pattern| {
            let config = draining::Config {
                radius: 6,
                pattern,
                spacing: 3,
            };
            let (corner, s) = drain(anchor, config, "minecraft:sand");
            (-6..=6)
                .flat_map(|x| (-6..=6).map(move |z| [x, 60, z]))
                .filter(|w| wants(&*s, corner, *w, "minecraft:water").is_some())
                .count()
        };
        let (fill, lines, grid) = (
            count(draining::Pattern::Fill),
            count(draining::Pattern::Lines),
            count(draining::Pattern::Grid),
        );
        assert_eq!(fill, 13 * 13);
        assert!(lines < grid && grid < fill, "{lines} {grid} {fill}");
    }

    #[test]
    fn blocks_are_chosen_from_the_inventory_cheapest_first() {
        assert_eq!(
            floor_block([
                "minecraft:diamond_block",
                "minecraft:cobblestone",
                "minecraft:stone"
            ])
            .as_deref(),
            Some("minecraft:cobblestone")
        );
        // Nothing safe to spend: no floor.
        assert_eq!(
            floor_block([
                "minecraft:chest",
                "minecraft:diamond_pickaxe",
                "minecraft:lava_bucket"
            ]),
            None
        );
        assert_eq!(floor_block(std::iter::empty()), None);
        let carried = ["minecraft:gravel", "minecraft:red_sand", "minecraft:stone"];
        assert_eq!(drain_block(carried).as_deref(), Some("minecraft:red_sand"));
        assert_eq!(drain_block(["minecraft:stone"]), None);
    }
}
