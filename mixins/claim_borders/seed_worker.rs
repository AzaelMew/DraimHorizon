// Read-only vanilla prediction. No server requests, terrain writes or game-thread I/O.
use crate::game::{claim_borders_map::View, claim_borders_seed_map as model};
use horizon_core::{
    position::ChunkPos,
    random::{LegacyRandom, RandomSource, WorldgenRandom},
};
use horizon_world::{
    registry::BlockRegistry,
    worldgen::generator::{GeneratorData, GeneratorWorker, OverworldGenerator},
};
use std::{
    collections::{BTreeMap, BTreeSet},
    path::PathBuf,
    sync::{Arc, Condvar, Mutex},
};
pub const SIDE: usize = 128;
#[derive(Clone, PartialEq)]
pub struct Query {
    pub scope: String,
    pub views: [View; 3],
    pub aspects: [f32; 3],
    pub preferences: model::Preferences,
}
#[derive(Clone)]
pub struct Mark {
    pub id: String,
    pub position: [f64; 2],
}
#[derive(Clone)]
pub struct Panel {
    pub pixels: Vec<u8>,
    pub biomes: Vec<String>,
    pub marks: Vec<Mark>,
    pub limited: bool,
}
pub struct Image {
    pub query: Query,
    pub panels: Vec<Panel>,
    pub catalog: Vec<String>,
    pub error: Option<String>,
}
#[derive(Default)]
struct Pending {
    query: Option<Query>,
    output: Option<Image>,
    generation: u64,
    stop: bool,
}
pub struct Handle {
    shared: Arc<(Mutex<Pending>, Condvar)>,
}
impl Handle {
    pub fn spawn(dir: PathBuf, registry: &'static BlockRegistry) -> Result<Self, String> {
        let shared = Arc::new((Mutex::new(Pending::default()), Condvar::new()));
        let inbox = shared.clone();
        std::thread::Builder::new()
            .name("draim-seed-map".into())
            .spawn(move || {
                let mut engine: Option<Result<Engine, String>> = None;
                loop {
                    let (query, generation) = {
                        let mut p = inbox.0.lock().unwrap_or_else(|e| e.into_inner());
                        while p.query.is_none() && !p.stop {
                            p = inbox.1.wait(p).unwrap_or_else(|e| e.into_inner());
                        }
                        if p.stop {
                            break;
                        }
                        (p.query.take().unwrap(), p.generation)
                    };
                    let canceled = || {
                        let p = inbox.0.lock().unwrap_or_else(|e| e.into_inner());
                        p.stop || p.generation != generation
                    };
                    let loaded = engine.get_or_insert_with(|| Engine::load(&dir, registry));
                    let result = match loaded {
                        Ok(engine) => engine.render(&query, &canceled),
                        Err(error) => Err(error.clone()),
                    };
                    if canceled() {
                        continue;
                    }
                    let image = match result {
                        Ok((panels, catalog)) => Image {
                            query,
                            panels,
                            catalog,
                            error: None,
                        },
                        Err(error) => Image {
                            query,
                            panels: Vec::new(),
                            catalog: Vec::new(),
                            error: Some(error),
                        },
                    };
                    inbox.0.lock().unwrap_or_else(|e| e.into_inner()).output = Some(image);
                }
            })
            .map_err(|e| e.to_string())?;
        Ok(Self { shared })
    }
    pub fn query(&self, query: Query) {
        let mut p = self.shared.0.lock().unwrap_or_else(|e| e.into_inner());
        p.generation = p.generation.wrapping_add(1);
        p.query = Some(query);
        self.shared.1.notify_one();
    }
    pub fn take(&self) -> Option<Image> {
        self.shared
            .0
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .output
            .take()
    }
    pub fn cancel(&self) {
        let mut p = self.shared.0.lock().unwrap_or_else(|e| e.into_inner());
        p.generation = p.generation.wrapping_add(1);
        p.query = None;
        p.output = None;
    }
}
impl Drop for Handle {
    fn drop(&mut self) {
        self.shared.0.lock().unwrap_or_else(|e| e.into_inner()).stop = true;
        self.shared.1.notify_one();
    }
}
struct Engine {
    data: Arc<GeneratorData>,
    workers: Vec<GeneratorWorker>,
    sets: BTreeMap<String, serde_json::Value>,
    cached: [Option<(View, f32, model::Preferences, Panel)>; 3],
}
impl Engine {
    fn load(dir: &std::path::Path, registry: &'static BlockRegistry) -> Result<Self, String> {
        let data =
            horizon_client::vanillagen::shared_data(dir, registry).map_err(|e| e.to_string())?;
        let mut workers = Vec::new();
        for dim in model::DIMENSIONS {
            let generator = OverworldGenerator::for_dimension(
                data.clone(),
                model::SEED,
                "minecraft:normal",
                dim,
            )
            .map_err(|e| e.to_string())?;
            workers.push(generator.worker());
        }
        let raw =
            std::fs::read_to_string(dir.join("structure_sets.json")).map_err(|e| e.to_string())?;
        let raw: BTreeMap<String, String> =
            serde_json::from_str(&raw).map_err(|e| e.to_string())?;
        let sets = raw
            .into_iter()
            .map(|(key, value)| serde_json::from_str(&value).map(|v| (key, v)))
            .collect::<Result<_, _>>()
            .map_err(|e| e.to_string())?;
        Ok(Self {
            data,
            workers,
            sets,
            cached: std::array::from_fn(|_| None),
        })
    }
    fn render(
        &mut self,
        query: &Query,
        canceled: &impl Fn() -> bool,
    ) -> Result<(Vec<Panel>, Vec<String>), String> {
        let mut panels = Vec::new();
        for i in 0..3 {
            let view = query.views[i];
            let aspect = query.aspects[i];
            if let Some((old_view, old_aspect, preferences, panel)) = &self.cached[i]
                && *old_view == view
                && *old_aspect == aspect
                && *preferences == query.preferences
            {
                panels.push(panel.clone());
                continue;
            }
            let worker = &mut self.workers[i];
            let mut pixels = vec![0; SIDE * SIDE * 4];
            let mut biomes = Vec::with_capacity(SIDE * SIDE);
            for y in 0..SIDE {
                if canceled() {
                    return Err("canceled".into());
                }
                let quarts: Vec<_> = (0..SIDE)
                    .map(|x| {
                        let at = view.world_aspect(
                            [x as f32 + 0.5, y as f32 + 0.5],
                            SIDE as f32,
                            aspect,
                        );
                        [
                            (at[0].floor() as i32).div_euclid(4),
                            16,
                            (at[1].floor() as i32).div_euclid(4),
                        ]
                    })
                    .collect();
                for (x, biome) in worker.noise_biomes(&quarts).into_iter().enumerate() {
                    let biome = biome.ok_or("генератор не вернул биом")?;
                    let rgb = if !query.preferences.biomes {
                        0x263440
                    } else {
                        model::biome_color(&biome)
                    };
                    let dim = query
                        .preferences
                        .biome
                        .as_ref()
                        .is_some_and(|b| b != &biome);
                    let factor = if dim { 0.23 } else { 1.0 };
                    let offset = (y * SIDE + x) * 4;
                    pixels[offset..offset + 4].copy_from_slice(&[
                        (((rgb >> 16) & 255) as f32 * factor) as u8,
                        (((rgb >> 8) & 255) as f32 * factor) as u8,
                        ((rgb & 255) as f32 * factor) as u8,
                        255,
                    ]);
                    biomes.push(biome);
                }
            }
            let mut candidates = BTreeSet::new();
            let mut limited = false;
            for (name, set) in &self.sets {
                let relevant = set["structures"].as_array().is_some_and(|entries| {
                    entries.iter().any(|v| {
                        v["structure"]
                            .as_str()
                            .is_some_and(|id| query.preferences.shows(id) && dimension_of(id) == i)
                    })
                });
                if !relevant {
                    continue;
                }
                let placement = &set["placement"];
                if placement["type"] == "minecraft:concentric_rings" {
                    if canceled() {
                        return Err("canceled".into());
                    }
                    for at in worker.ring_positions(name).unwrap_or_default() {
                        if candidates.len() < 512 {
                            candidates.insert((at.x, at.z));
                        } else {
                            limited = true;
                        }
                    }
                    continue;
                }
                if placement["type"] != "minecraft:random_spread" {
                    continue;
                }
                let spacing = placement["spacing"].as_i64().unwrap_or(0) as i32;
                let separation = placement["separation"].as_i64().unwrap_or(0) as i32;
                let salt = placement["salt"].as_i64().unwrap_or(0) as i32;
                if spacing <= separation || separation < 0 {
                    continue;
                }
                let Some((lo, hi)) = model::region_bounds(view, aspect, spacing) else {
                    limited = true;
                    continue;
                };
                for rz in lo[1]..=hi[1] {
                    for rx in lo[0]..=hi[0] {
                        let mut random = WorldgenRandom::new(LegacyRandom::new(0));
                        random.set_large_feature_with_salt(model::SEED, rx, rz, salt);
                        let mut spread = || {
                            let a = random.next_int_bound(spacing - separation).unwrap_or(0);
                            if placement["spread_type"] == "triangular" {
                                (a + random.next_int_bound(spacing - separation).unwrap_or(0)) / 2
                            } else {
                                a
                            }
                        };
                        let x = rx * spacing + spread();
                        let z = rz * spacing + spread();
                        if candidates.len() < 512 {
                            candidates.insert((x, z));
                        } else {
                            limited = true;
                        }
                    }
                }
            }
            let mut marks = Vec::new();
            for (x, z) in candidates {
                if canceled() {
                    return Err("canceled".into());
                }
                let pixel = view.pixel_aspect(
                    [f64::from(x * 16 + 8), f64::from(z * 16 + 8)],
                    128.0,
                    aspect,
                );
                if pixel[0] < -2.0 || pixel[1] < -2.0 || pixel[0] > 130.0 || pixel[1] > 130.0 {
                    continue;
                }
                // Full starts apply biome, frequency, exclusion and terrain checks.
                for start in worker.structure_starts(ChunkPos::new(x, z)) {
                    if query.preferences.shows(&start.name) {
                        marks.push(Mark {
                            id: start.name.clone(),
                            position: [f64::from(x * 16 + 8), f64::from(z * 16 + 8)],
                        });
                    }
                }
            }
            let panel = Panel {
                pixels,
                biomes,
                marks,
                limited,
            };
            self.cached[i] = Some((view, aspect, query.preferences.clone(), panel.clone()));
            panels.push(panel);
            worker.release();
        }
        let catalog = self
            .data
            .biome_names()
            .iter()
            .map(|s| s.to_string())
            .collect();
        Ok((panels, catalog))
    }
}
fn dimension_of(id: &str) -> usize {
    if id.contains("end_city") {
        2
    } else if id.contains("fortress") || id.contains("bastion") || id.ends_with("_nether") {
        1
    } else {
        0
    }
}
