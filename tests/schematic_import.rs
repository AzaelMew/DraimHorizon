#[cfg(test)]
mod tests {
    use claim_borders::schematic::*;
    #[test]
    fn soil_height_and_intrinsic_states_are_preserved() {
        use claim_borders::navigation::{Cell, classify, standing_block};
        assert_eq!(
            standing_block([-0.2, 63.9375, 1.5], "minecraft:farmland"),
            [-1, 64, 1]
        );
        assert_eq!(
            standing_block([-0.2, 63.5, 1.5], "minecraft:stone_slab"),
            [-1, 63, 1]
        );
        assert_eq!(classify("minecraft:farmland", false, false), Cell::Solid);
        let rail = BlockState::parse("minecraft:rail[shape=ascending_north]")
            .unwrap()
            .rotated(1);
        assert_eq!(rail.properties["shape"], "ascending_east");
        assert!(!rail.matches("minecraft:rail", |_| Some("north_south".into())));
        let door = BlockState::parse("minecraft:oak_door[facing=east,half=lower]").unwrap();
        assert!(!door.matches("minecraft:oak_door", |_| Some("west".into())));
        let blocks = vec![
            (
                [0, 0, 0],
                BlockState::parse("minecraft:stone_slab[type=double]").unwrap(),
            ),
            (
                [0, 1, 0],
                BlockState::parse("minecraft:oak_door[half=upper]").unwrap(),
            ),
        ];
        let doc = Blueprint {
            blocks,
            data_version: 0,
            ignored_data: 0,
            format: "schem",
        };
        assert_eq!(doc.materials().get("minecraft:stone_slab"), Some(&2));
        assert!(!doc.materials().contains_key("minecraft:oak_door"));
        assert!(BlockState::parse("minecraft:").is_err());
    }
    #[test]
    fn a_large_litematica_build_is_read() {
        // 140 x 4 x 140 with 39 200 blocks: past the earlier limits of 128 to a side and 32 768
        // blocks, which left such a file's preview empty.
        let doc = decode(include_bytes!("fixtures/schematics/litematic-large.nbt")).unwrap();
        assert_eq!(doc.format, "litematic");
        assert_eq!(doc.blocks.len(), 140 * 140 * 2);
        let log = doc.blocks.iter().find(|(p, _)| p == &[0, 1, 0]).unwrap();
        assert_eq!(log.1.name, "minecraft:oak_log");
        assert!(doc.blocks.iter().all(|(p, _)| p[1] < 2));
    }
    #[test]
    fn imports_both_sponge_versions_and_signed_multi_region_coordinates() {
        let v2 = decode(include_bytes!("fixtures/schematics/sponge-v2.nbt")).unwrap();
        let v3 = decode(include_bytes!("fixtures/schematics/sponge-v3.nbt")).unwrap();
        let lite = decode(include_bytes!(
            "fixtures/schematics/litematic-v7-negative.nbt"
        ))
        .unwrap();
        assert_eq!(v2.blocks, v3.blocks);
        assert_eq!(v2.blocks, lite.blocks);
        assert_eq!(v2.blocks.len(), 10);
        let cross = decode(include_bytes!(
            "fixtures/schematics/litematic-cross-word.nbt"
        ))
        .unwrap();
        assert_eq!(cross.blocks.len(), 25);
        let block = cross.blocks.iter().find(|(p, _)| p == &[22, 0, 0]).unwrap();
        assert_eq!(
            block.1.properties.get("test").map(String::as_str),
            Some("1")
        );
    }
    #[test]
    fn rotates_states_and_positions_without_losing_connections() {
        let state = BlockState::parse(
            "minecraft:oak_stairs[facing=north,half=bottom,north=true,east=false]",
        )
        .unwrap()
        .rotated(1);
        assert_eq!(state.properties["facing"], "east");
        assert_eq!(state.properties["east"], "true");
        assert_eq!(state.properties["south"], "false");
        let doc = decode(include_bytes!("fixtures/schematics/sponge-v3.nbt"))
            .unwrap()
            .transformed([10, 64, 20], 1)
            .unwrap();
        assert!(doc.blocks.iter().any(|(p, _)| p == &[11, 64, 19]));
    }
    #[test]
    fn rejects_truncation_trailing_data_and_path_traversal() {
        let input = include_bytes!("fixtures/schematics/sponge-v3.nbt");
        for n in [0, 1, 8, input.len() - 1] {
            assert!(decode(&input[..n]).is_err());
        }
        let mut extra = input.to_vec();
        extra.push(0);
        assert!(decode(&extra).is_err());
        for name in [
            "../x.schem",
            "C:x.schem",
            "a\\x.litematic",
            ".hidden.schem",
            "x.jar",
        ] {
            assert!(!safe_filename(name));
        }
        assert!(safe_filename("Дом.litematic"));
    }
    #[test]
    fn a_mirrored_preview_flips_cells_and_states_the_way_the_game_mirrors_them() {
        use claim_borders::schematic_places::{place, unplace};
        let st = |t: &str| BlockState::parse(t).unwrap();
        // Across Z: north and south change places, east and west stay.
        let across_z = st("minecraft:furnace[facing=north]").mirrored(1);
        assert_eq!(across_z.properties["facing"], "south");
        assert_eq!(
            st("minecraft:furnace[facing=east]").mirrored(1).properties["facing"],
            "east"
        );
        assert_eq!(
            st("minecraft:furnace[facing=east]").mirrored(2).properties["facing"],
            "west"
        );
        // A sign's 16 steps: 0 (south) stays across X, 8 (north) goes to 0 across Z.
        assert_eq!(
            st("minecraft:oak_sign[rotation=4]").mirrored(1).properties["rotation"],
            "4"
        );
        assert_eq!(
            st("minecraft:oak_sign[rotation=4]").mirrored(2).properties["rotation"],
            "12"
        );
        // Doors swap their hinge, stairs corners their side, rails their slope.
        assert_eq!(
            st("minecraft:oak_door[facing=east,hinge=left]")
                .mirrored(1)
                .properties["hinge"],
            "right"
        );
        let stair = st("minecraft:oak_stairs[facing=north,shape=inner_left]").mirrored(1);
        assert_eq!(
            (
                stair.properties["facing"].as_str(),
                stair.properties["shape"].as_str()
            ),
            ("south", "inner_right")
        );
        assert_eq!(
            st("minecraft:oak_stairs[facing=east,shape=inner_left]")
                .mirrored(1)
                .properties["shape"],
            "inner_left"
        );
        assert_eq!(
            st("minecraft:rail[shape=ascending_north]")
                .mirrored(1)
                .properties["shape"],
            "ascending_south"
        );
        // A mirror twice is no mirror, and a turn comes after it.
        let s = st("minecraft:furnace[facing=north]");
        assert_eq!(s.mirrored(1).mirrored(1), s);
        assert_eq!(s.transformed(4 + 1).properties["facing"], "west");
        // Cells: place and unplace undo each other for every turn and mirror.
        for code in 0..12 {
            for at in [[3, 1, -2], [0, 5, 7], [-4, 0, 0]] {
                let w = place(at, [10, 64, -20], code);
                assert_eq!(unplace(w, [10, 64, -20], code), at, "{code}");
            }
        }
        assert_eq!(place([3, 0, 2], [0, 0, 0], 4), [3, 0, -2]);
        assert_eq!(place([3, 0, 2], [0, 0, 0], 8), [-3, 0, 2]);
    }
}
#[cfg(test)]
mod items_tests {
    use claim_borders::schematic::BlockState;
    #[test]
    fn blocks_become_the_items_that_place_them() {
        let st = |t: &str| BlockState::parse(t).unwrap();
        let one = |t: &str| st(t).items();
        assert_eq!(
            one("minecraft:oak_slab[type=double]"),
            [("minecraft:oak_slab".to_owned(), 2)]
        );
        assert_eq!(
            one("minecraft:oak_slab[type=top]"),
            [("minecraft:oak_slab".to_owned(), 1)]
        );
        assert!(one("minecraft:oak_door[half=upper]").is_empty());
        assert!(one("minecraft:red_bed[part=head]").is_empty());
        assert_eq!(
            one("minecraft:wall_torch[facing=north]"),
            [("minecraft:torch".to_owned(), 1)]
        );
        assert_eq!(
            one("minecraft:oak_wall_sign[facing=north]"),
            [("minecraft:oak_sign".to_owned(), 1)]
        );
        assert_eq!(
            one("minecraft:redstone_wire[power=0]"),
            [("minecraft:redstone".to_owned(), 1)]
        );
        assert_eq!(
            one("minecraft:sea_pickle[pickles=3]"),
            [("minecraft:sea_pickle".to_owned(), 3)]
        );
        assert_eq!(
            one("minecraft:potted_poppy"),
            [
                ("minecraft:flower_pot".to_owned(), 1),
                ("minecraft:poppy".to_owned(), 1)
            ]
        );
        assert_eq!(
            one("minecraft:water[level=0]"),
            [("minecraft:water_bucket".to_owned(), 1)]
        );
        assert!(one("minecraft:water[level=3]").is_empty());
        assert!(one("minecraft:air").is_empty());
        assert!(st("minecraft:oak_stairs[waterlogged=true]").waterlogged());
    }
}
