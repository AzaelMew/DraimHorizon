//! Times the walking route planner on generated terrain, read the way the game's planner thread
//! reads it (a chunk map, then a table from block state to cell).
//! Run: cargo run --release --example route_bench
use claim_borders::mining;
use claim_borders::navigation::{self as nav, Cell, Pos};
use std::collections::HashMap;
use std::time::Instant;

/// Rolling hills with ponds, scattered trees and a few walls; deterministic.
fn terrain_height(x: i32, z: i32) -> i32 {
    let (fx, fz) = (f64::from(x), f64::from(z));
    let h = (fx * 0.07).sin() * 4.0 + (fz * 0.05).cos() * 5.0 + ((fx + fz) * 0.021).sin() * 7.0;
    64 + h.round() as i32
}
fn hash(x: i32, z: i32) -> u32 {
    let mut h = (x as u32).wrapping_mul(0x9E37_79B1) ^ (z as u32).wrapping_mul(0x85EB_CA77);
    h ^= h >> 15;
    h = h.wrapping_mul(0x2C1B_3C6D);
    h ^ (h >> 12)
}
/// Block "state ids": 0 air, 1 stone, 2 grass, 3 log, 4 leaves, 5 water, 6 lava.
fn block(x: i32, y: i32, z: i32) -> u32 {
    let ground = terrain_height(x, z);
    if y < ground {
        return if y == ground - 1 { 2 } else { 1 };
    }
    if ground < 62 && y < 62 {
        return 5;
    }
    let tree = hash(x, z).is_multiple_of(97);
    if tree && y < ground + 5 {
        return 3;
    }
    for dx in -2..=2 {
        for dz in -2..=2 {
            if hash(x + dx, z + dz).is_multiple_of(97) && (ground + 3..ground + 6).contains(&y) {
                return 4;
            }
        }
    }
    if hash(x / 7, z / 7).is_multiple_of(23) && y < ground + 3 && (x % 7 == 0) {
        return 1;
    }
    if hash(x * 3 + 1, z).is_multiple_of(1500) && y == ground {
        return 6;
    }
    0
}
struct World {
    chunks: HashMap<(i32, i32), Vec<u32>>,
    table: HashMap<u32, Cell>,
}
impl World {
    fn new(center: Pos, radius: i32) -> Self {
        let mut chunks = HashMap::new();
        let (c0, c1) = ((center[0] - radius) >> 4, (center[0] + radius) >> 4);
        let (d0, d1) = ((center[2] - radius) >> 4, (center[2] + radius) >> 4);
        for cx in c0 - 1..=c1 + 1 {
            for cz in d0 - 1..=d1 + 1 {
                let mut v = vec![0; 16 * 16 * 128];
                for y in 0..128 {
                    for z in 0..16 {
                        for x in 0..16 {
                            v[(y * 16 + z) * 16 + x] =
                                block(cx * 16 + x as i32, y as i32, cz * 16 + z as i32);
                        }
                    }
                }
                chunks.insert((cx, cz), v);
            }
        }
        let table = HashMap::from([
            (0, Cell::Air),
            (1, Cell::Solid),
            (2, Cell::Solid),
            (3, Cell::Solid),
            (4, Cell::Solid),
            (5, Cell::Water),
            (6, Cell::Hazard),
        ]);
        Self { chunks, table }
    }
    fn cell(&self, at: Pos) -> Cell {
        let Some(c) = self
            .chunks
            .get(&(at[0].div_euclid(16), at[2].div_euclid(16)))
        else {
            return Cell::Unknown;
        };
        if !(0..128).contains(&at[1]) {
            return Cell::Unknown;
        }
        let id = c[(at[1] as usize * 16 + at[2].rem_euclid(16) as usize) * 16
            + at[0].rem_euclid(16) as usize];
        *self.table.get(&id).unwrap_or(&Cell::Unknown)
    }
}
fn ground(x: i32, z: i32) -> Pos {
    let mut y = terrain_height(x, z);
    while block(x, y, z) != 0 || block(x, y + 1, z) != 0 {
        y += 1;
    }
    [x, y, z]
}
fn main() {
    let cases: [(&str, Pos, Pos, bool); 6] = [
        ("near 25", [0, 0, 0], [25, 0, 10], false),
        ("near 30 back", [0, 0, 0], [-30, 0, -5], false),
        ("wide 80", [0, 0, 0], [80, 0, 30], true),
        ("wide 140", [0, 0, 0], [140, 0, -40], true),
        ("wide 140 diag", [0, 0, 0], [-100, 0, 100], true),
        ("unreachable", [0, 0, 0], [-110, 0, 100], true),
    ];
    let world = World::new([0, 64, 0], 180);
    let mut total = 0.0;
    for (name, s, g, wide) in cases {
        let (start, goal) = (ground(s[0], s[2]), ground(g[0], g[2]));
        nav::set_reach(if wide { nav::WIDE } else { nav::NEAR });
        let mut reads = 0u64;
        let t = Instant::now();
        let path = nav::search(
            start,
            goal,
            |at| {
                reads += 1;
                world.cell(at)
            },
            || false,
        );
        let ms = t.elapsed().as_secs_f64() * 1000.0;
        total += ms;
        let (len, complete) = path.map_or((0, false), |p| (p.nodes.len(), p.complete));
        println!("{name:>14}: {ms:8.1} ms  reads {reads:>10}  nodes {len:>4}  complete {complete}");
    }
    println!("{:>14}: {total:8.1} ms", "total");
    // The miner's planner: the nearest reachable log among the trees around.
    nav::set_reach(nav::NEAR);
    for (x, z, walled) in [
        (0, 0, false),
        (40, -20, false),
        (-60, 35, false),
        (10, 10, true),
    ] {
        let start = ground(x, z);
        // Walled: six logs, each in air inside a closed stone box a few blocks off: open (so
        // candidates) but out of reach.
        let centers: Vec<Pos> = if walled {
            [[5, 0], [-5, 0], [0, 5], [0, -5], [6, 6], [-6, -6]]
                .iter()
                .map(|[dx, dz]| [start[0] + dx, start[1] + 1, start[2] + dz])
                .collect()
        } else {
            Vec::new()
        };
        let inside = |at: Pos, r: i32| {
            centers
                .iter()
                .any(|c| (0..3).all(|i| (at[i] - c[i]).abs() <= r))
        };
        let t = Instant::now();
        let found = mining::plan(
            start,
            &[],
            |at| {
                let id = world
                    .chunks
                    .get(&(at[0].div_euclid(16), at[2].div_euclid(16)))
                    .and_then(|c| {
                        (0..128).contains(&at[1]).then(|| {
                            c[(at[1] as usize * 16 + at[2].rem_euclid(16) as usize) * 16
                                + at[0].rem_euclid(16) as usize]
                        })
                    })
                    .unwrap_or(0);
                if inside(at, 2) {
                    let middle = centers.contains(&at);
                    let air = inside(at, 1) && !middle;
                    return mining::Block {
                        cell: if air { Cell::Air } else { Cell::Solid },
                        ore: middle,
                        falling: false,
                        breakable: !air,
                        soft: false,
                        water: false,
                    };
                }
                mining::Block {
                    cell: world.cell(at),
                    ore: id == 3,
                    falling: false,
                    breakable: id != 0 && id != 5,
                    soft: id == 4,
                    water: id == 5,
                }
            },
            || false,
        );
        println!(
            "{:>14}: {:8.1} ms  found {:?}",
            format!("mine {x},{z}{}", if walled { " walled" } else { "" }),
            t.elapsed().as_secs_f64() * 1000.0,
            found.map(|(t, p)| (t, p.nodes.len()))
        );
    }
}
