use claim_borders::{
    map_art::{self, Material, Request},
    schematic::{self, BlockState},
};
fn material() -> Material {
    Material {
        state: BlockState::parse("white_wool").unwrap(),
        shades: [[50; 3], [100; 3], [200; 3]],
    }
}
fn make(request: Request, rgba: &[u8]) -> map_art::Plan {
    map_art::generate(
        request,
        [1, 1],
        rgba,
        &[material()],
        BlockState::parse("cobblestone").unwrap(),
    )
    .unwrap()
}
#[test]
fn flat_tile_has_floor_guide_row_and_exact_materials() {
    let plan = make(
        Request {
            dither: false,
            ..Default::default()
        },
        &[100, 100, 100, 255],
    );
    assert_eq!((plan.width, plan.depth, plan.height), (128, 128, 2));
    assert!(plan.pixels.iter().all(|c| *c == [100; 3]));
    assert_eq!(plan.materials["minecraft:white_wool"], 16384);
    assert_eq!(plan.materials["minecraft:cobblestone"], 16384 + 256);
    assert!(
        plan.document
            .blocks
            .iter()
            .filter(|(_, s)| s.name == "minecraft:white_wool")
            .all(|(p, _)| p[1] == 1)
    );
}
#[test]
fn stairs_obey_height_limit_and_preview_matches_real_height_changes() {
    let plan = make(
        Request {
            stepped: true,
            height: 4,
            dither: false,
            ..Default::default()
        },
        &[200, 200, 200, 255],
    );
    let top = plan
        .document
        .blocks
        .iter()
        .filter(|(_, s)| s.name == "minecraft:white_wool")
        .map(|(p, _)| (*p, true))
        .collect::<std::collections::BTreeMap<_, _>>();
    assert!(plan.height <= 5);
    let mut columns = std::collections::BTreeMap::new();
    let cells = plan
        .document
        .blocks
        .iter()
        .map(|(p, _)| *p)
        .collect::<std::collections::BTreeSet<_>>();
    for p in &cells {
        columns
            .entry((p[0], p[2]))
            .and_modify(|y: &mut i32| *y = (*y).max(p[1]))
            .or_insert(p[1]);
    }
    for p in top.keys() {
        let north = columns[&(p[0], p[2] - 1)];
        let shade = if p[1] > north {
            2
        } else if p[1] < north {
            0
        } else {
            1
        };
        assert_eq!(
            plan.pixels[p[2] as usize * 128 + p[0] as usize],
            material().shades[shade]
        );
        assert!(cells.contains(&[p[0], p[1] - 1, p[2]]));
    }
}
#[test]
fn four_tiles_round_trip_through_litematica_and_keep_materials() {
    let plan = make(
        Request {
            tiles: [2, 2],
            dither: false,
            ..Default::default()
        },
        &[100, 100, 100, 255],
    );
    let regions = map_art::regions(&plan).unwrap();
    assert_eq!(regions.len(), 4);
    let meta = claim_borders::litematica_writer::Metadata {
        name: "Map art".into(),
        author: String::new(),
        description: String::new(),
        time_ms: 0,
        data_version: 0,
    };
    let bytes = claim_borders::litematica_writer::write(&meta, &regions);
    let parsed = schematic::decode(&bytes).unwrap();
    assert_eq!(parsed.materials(), plan.materials);
    let expected = plan
        .document
        .blocks
        .into_iter()
        .collect::<std::collections::BTreeMap<_, _>>();
    assert_eq!(
        parsed
            .blocks
            .into_iter()
            .collect::<std::collections::BTreeMap<_, _>>(),
        expected
    );
}
#[test]
fn transparency_uses_background_and_bad_inputs_are_rejected() {
    let plan = make(
        Request {
            background: "646464".into(),
            dither: false,
            ..Default::default()
        },
        &[255, 0, 0, 0],
    );
    assert!(plan.pixels.iter().all(|c| *c == [100; 3]));
    for request in [
        Request {
            tiles: [0, 1],
            ..Default::default()
        },
        Request {
            height: 0,
            ..Default::default()
        },
        Request {
            background: "invalid".into(),
            ..Default::default()
        },
    ] {
        assert!(
            map_art::generate(
                request,
                [1, 1],
                &[0; 4],
                &[material()],
                BlockState::parse("stone").unwrap()
            )
            .is_err()
        );
    }
    assert!(
        map_art::generate(
            Request::default(),
            [8192, 8192],
            &[],
            &[material()],
            BlockState::parse("stone").unwrap()
        )
        .is_err()
    );
    assert_eq!(
        Request {
            palette: 3,
            custom: "white_wool,black_wool".into(),
            ..Default::default()
        }
        .names()
        .unwrap(),
        ["minecraft:white_wool", "minecraft:black_wool"]
    );
}
#[test]
fn map_art_pages_fit_small_screens_and_emit_explicit_generation_requests() {
    use claim_borders::menu::{Action, Control, Field, Menu, Tab, ViewModel};
    let model = ViewModel::default();
    let mut menu = Menu::default();
    menu.tab = Tab::Schematics;
    fn click(menu: &mut Menu, model: &ViewModel, control: Control) -> Option<Action> {
        let rect = menu
            .layout(model, 360.0, 200.0)
            .into_iter()
            .find(|w| w.control == Some(control))
            .unwrap()
            .rect;
        menu.click(
            model,
            [360.0, 200.0],
            [rect[0] + rect[2] * 0.5, rect[1] + rect[3] * 0.5],
        )
    }
    click(&mut menu, &model, Control::MapArtOpen);
    for height in [200.0, 240.0, 400.0] {
        for page in 0..5 {
            menu.map_art_page = page;
            let rows = menu.layout(&model, 360.0, height);
            for row in rows.iter().filter(|w| {
                matches!(
                    w.control,
                    Some(
                        Control::MapArtPage(_)
                            | Control::MapArtOpen
                            | Control::MapArtAdjust(_)
                            | Control::MapArtGenerate
                            | Control::MapArtFolder
                            | Control::MapArtImage
                            | Control::Field(
                                Field::MapArtImage
                                    | Field::MapArtCustom
                                    | Field::MapArtFloor
                                    | Field::MapArtBackground
                            )
                    )
                )
            }) {
                assert!(row.rect[1] + row.rect[3] <= height - 48.0);
                assert!(row.rect[2] > 0.0 && row.rect[3] > 0.0);
            }
        }
    }
    menu.map_art_page = 0;
    click(&mut menu, &model, Control::Field(Field::MapArtImage));
    for c in "C:/images/cat.png".chars() {
        menu.character(c);
    }
    assert_eq!(menu.map_art.image, "C:/images/cat.png");
    menu.map_art_page = 3;
    assert!(
        matches!(click(&mut menu,&model,Control::MapArtGenerate),Some(Action::MapArtGenerate(request)) if request.image=="C:/images/cat.png")
    );
    assert!(menu.baritone_request(true).is_err());
}
