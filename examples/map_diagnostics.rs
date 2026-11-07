// Investigation harness; synthetic inputs, not an in-game frame-time benchmark.
// Run: cargo run --locked --release --example map_diagnostics
use claim_borders::{claim_map as map, map_history as history};
use std::{hint::black_box, time::Instant};

fn overlay(aspect: f32, offset: f64) -> (usize, usize) {
    let size = 512;
    let view = map::View {
        center: [8.0 + offset, 8.0],
        blocks_per_pixel: 0.5,
    };
    let snapshot = map::Snapshot {
        own_color: 0x00ff00,
        cells: vec![map::Cell {
            dimension: "minecraft:overworld".into(),
            x: 0,
            z: 0,
            owner: "fixture".into(),
            own: true,
            foreign: false,
        }],
        ..Default::default()
    };
    let mut image = vec![0; size * size * 4];
    map::paint_aspect(
        &mut image,
        size,
        view,
        "minecraft:overworld",
        &snapshot,
        map::Preferences::default(),
        aspect,
    );
    let strong = |x: usize, y: usize| image[(y * size + x) * 4 + 1] > 200;
    let vertical = (0..size).filter(|&x| strong(x, size / 2)).count();
    let horizontal = (0..size).filter(|&y| strong(size / 2, y)).count();
    (vertical, horizontal)
}

fn main() {
    println!("Synthetic CPU diagnostics; optimized build, no game/GPU timings.");
    for aspect in [1.0, 2.0, 3.0] {
        for offset in [0.0, 0.125, 0.25] {
            let (v, h) = overlay(aspect, offset);
            println!(
                "outline aspect={aspect} offset={offset}: strong pixels across vertical edges={v}, horizontal edges={h}"
            );
        }
    }
    // The regression is fixed: all four sides retain raster coverage.
    for offset in [0.0, 0.125, 0.25] {
        let (vertical, horizontal) = overlay(2.0, offset);
        assert!(vertical >= 2 && horizontal >= 2);
    }

    let snapshot = map::Snapshot {
        own_color: 0x00ff00,
        cells: vec![map::Cell {
            dimension: "minecraft:overworld".into(),
            x: 0,
            z: 0,
            owner: "fixture".into(),
            own: true,
            foreign: false,
        }],
        ..Default::default()
    };
    let mut image = vec![0; 512 * 512 * 4];
    let start = Instant::now();
    for _ in 0..100 {
        map::paint_aspect(
            black_box(&mut image),
            512,
            map::View {
                center: [8.0, 8.0],
                blocks_per_pixel: 0.5,
            },
            "minecraft:overworld",
            black_box(&snapshot),
            map::Preferences::default(),
            2.0,
        );
    }
    println!(
        "paint_aspect 512x512 / one claim: {:.3} ms/call (100 calls)",
        start.elapsed().as_secs_f64() * 10.0
    );
    let start = Instant::now();
    for _ in 0..10_000 {
        black_box(map::overlay_rects(
            map::View {
                center: [8.0, 8.0],
                blocks_per_pixel: 2.0,
            },
            "minecraft:overworld",
            black_box(&snapshot),
            map::Preferences::default(),
            [0.0, 0.0, 1024.0, 512.0],
        ));
    }
    println!(
        "fullscreen overlay geometry / one claim: {:.3} us/call (10000 calls)",
        start.elapsed().as_secs_f64() * 100.0
    );

    let root = std::env::temp_dir().join(format!(
        "draimhorizon-map-diagnostics-{}",
        std::process::id()
    ));
    // Nonpersistent Store never creates this directory or writes any map files.
    let context = history::Context {
        scope: "synthetic-probe".into(),
        dimension: "minecraft:overworld".into(),
        layer: i32::MAX,
        persistent: false,
    };
    let mut store = history::Store::new(root, vec![0, 0x809040]);
    for z in -8..8 {
        for x in -8..8 {
            store
                .insert(
                    &context,
                    (x, z),
                    history::Tile {
                        color: [1; 256],
                        height: [64; 256],
                        depth: [0; 256],
                        rgb: [0; 256],
                    },
                )
                .unwrap();
        }
    }
    let mut render = history::Render {
        center: [0.0, 0.0],
        bpp: 2.0,
        aspect: 2.0,
        size: 512,
        shading: true,
    };
    let start = Instant::now();
    for i in 0..100 {
        render.center[0] = f64::from(i) * 0.125;
        black_box(store.compose(&context, &render).unwrap());
    }
    println!(
        "compose 512x512 / 256 resident chunks / fractional pan: {:.3} ms/call (100 calls)",
        start.elapsed().as_secs_f64() * 10.0
    );

    // A continuous world slope must darken the first column in a new region too.
    let mut slope_store = history::Store::new(
        std::env::temp_dir().join("unused-map-diagnostics"),
        vec![0, 0x808080],
    );
    for x in [15, 16] {
        let mut tile = history::Tile {
            color: [1; 256],
            height: [0; 256],
            depth: [0; 256],
            rgb: [0; 256],
        };
        for z in 0..16 {
            for local_x in 0..16 {
                tile.height[z * 16 + local_x] = (300 - (x * 16 + local_x as i32)) as i16;
            }
        }
        slope_store.insert(&context, (x, 0), tile).unwrap();
    }
    let slope = slope_store
        .compose(
            &context,
            &history::Render {
                center: [256.0, 8.0],
                bpp: 0.5,
                aspect: 1.0,
                size: 64,
                shading: true,
            },
        )
        .unwrap();
    let row = 32;
    let red = |x: usize| slope[(row * 64 + x) * 4];
    println!(
        "continuous descending slope red channel at x=255/256/257: {}/{}/{}",
        red(31),
        red(32),
        red(33)
    );
    assert_eq!(red(31), red(33));
    assert_eq!(
        red(32),
        red(31),
        "continuous slope must not have a region-edge stripe"
    );
}
