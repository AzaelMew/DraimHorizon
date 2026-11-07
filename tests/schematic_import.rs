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
}
