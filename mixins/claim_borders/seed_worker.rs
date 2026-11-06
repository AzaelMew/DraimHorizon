// Read-only vanilla prediction. No server requests, terrain writes or game-thread I/O.
use crate::game::{claim_borders_map::View, claim_borders_seed_map as model};
use horizon_core::position::ChunkPos;
use horizon_world::{
    registry::BlockRegistry,
    worldgen::{
        generator::{GeneratorData, GeneratorWorker, OverworldGenerator},
        structure::start::{Rings, StructureRegistry},
    },
};
use std::{
    collections::{BTreeMap, BTreeSet},
    path::PathBuf,
    sync::{Arc, Condvar, Mutex},
};
pub const SIDE: usize = 256;
#[derive(Clone, PartialEq)]
pub struct Query {
    pub scope: String,
    pub view: View,
    pub aspect: f32,
    pub dimension: usize,
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
    pub panel: Option<Panel>,
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
                        Ok((panel, catalog)) => Image {
                            query,
                            panel: Some(panel),
                            catalog,
                            error: None,
                        },
                        Err(error) => Image {
                            query,
                            panel: None,
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
    sets: StructureRegistry,
    members: BTreeMap<String, Vec<String>>,
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
        let members = raw
            .iter()
            .map(|(id, text)| {
                let value: serde_json::Value =
                    serde_json::from_str(text).map_err(|e| e.to_string())?;
                let entries = value["structures"]
                    .as_array()
                    .ok_or("invalid structure set")?;
                Ok((
                    id.clone(),
                    entries
                        .iter()
                        .filter_map(|v| v["structure"].as_str().map(str::to_owned))
                        .collect(),
                ))
            })
            .collect::<Result<BTreeMap<String, Vec<String>>, String>>()?;
        // This registry is used ONLY for placement (including frequency and exclusion).
        // Actual biome/terrain validation is delegated to the fully loaded generator.
        let sets = StructureRegistry::new(&BTreeMap::new(), &raw, BTreeMap::new(), &|_| {
            Ok(BTreeSet::new())
        })
        .map_err(|e| e.to_string())?;
        Ok(Self {
            data,
            workers,
            sets,
            members,
            cached: std::array::from_fn(|_| None),
        })
    }
    fn render(
        &mut self,
        query: &Query,
        canceled: &impl Fn() -> bool,
    ) -> Result<(Panel, Vec<String>), String> {
        let i = query.dimension.min(2);
        let view = query.view;
        let aspect = query.aspect;
        let catalog = self
            .data
            .biome_names()
            .iter()
            .map(|s| s.to_string())
            .collect();
        if let Some((v, a, p, panel)) = &self.cached[i]
            && *v == view
            && *a == aspect
            && p.same_layers(&query.preferences)
        {
            return Ok((panel.clone(), catalog));
        }
        let backdrop = self.cached[i]
            .as_ref()
            .filter(|(v, a, p, _)| *v == view && *a == aspect && p.same_biomes(&query.preferences))
            .map(|(_, _, _, panel)| (panel.pixels.clone(), panel.biomes.clone()));
        let worker = &mut self.workers[i];
        let mut pixels = vec![0; SIDE * SIDE * 4];
        let mut biomes = Vec::with_capacity(SIDE * SIDE);
        if let Some((old_pixels, old_biomes)) = backdrop {
            pixels = old_pixels;
            biomes = old_biomes;
        } else {
            for y in 0..SIDE {
                if canceled() {
                    return Err("canceled".into());
                }
                let quarts: Vec<_> = (0..SIDE)
                    .map(|x| {
                        let world = view.world_aspect(
                            [
                                (x as f32 + 0.5) * 128.0 / SIDE as f32,
                                (y as f32 + 0.5) * 128.0 / SIDE as f32,
                            ],
                            128.0,
                            aspect,
                        );
                        [
                            (world[0].floor() as i32).div_euclid(4),
                            16,
                            (world[1].floor() as i32).div_euclid(4),
                        ]
                    })
                    .collect();
                for (x, biome) in worker.noise_biomes(&quarts).into_iter().enumerate() {
                    let biome = biome.ok_or("генератор не вернул биом")?;
                    let rgb = if query.preferences.biomes {
                        model::biome_color(&biome)
                    } else {
                        0x263440
                    };
                    let factor = if query
                        .preferences
                        .biome
                        .as_ref()
                        .is_some_and(|b| b != &biome)
                    {
                        0.23
                    } else {
                        1.0
                    };
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
        }
        let mut candidates = BTreeSet::new();
        let mut limited = false;
        let rings = Rings::new();
        let in_view = |x: i32, z: i32| {
            let at = view.pixel_aspect(
                [f64::from(x * 16 + 8), f64::from(z * 16 + 8)],
                128.0,
                aspect,
            );
            at[0] >= -2.0 && at[1] >= -2.0 && at[0] <= 130.0 && at[1] <= 130.0
        };
        for set in &self.sets.sets {
            let relevant = self.members.get(&set.name).is_some_and(|entries| {
                entries.iter().any(|id| {
                    query.preferences.shows(id)
                        && model::structure_index(id)
                            .is_some_and(|n| model::STRUCTURES[n].group.dimension() == i)
                })
            });
            if !relevant {
                continue;
            }
            if let Some(spacing) = set.spacing() {
                let Some((lo, hi)) = model::region_bounds(view, aspect, spacing) else {
                    limited = true;
                    continue;
                };
                for rz in lo[1]..=hi[1] {
                    if canceled() {
                        return Err("canceled".into());
                    }
                    for rx in lo[0]..=hi[0] {
                        let Some(at) = StructureRegistry::potential_chunk(
                            set,
                            model::SEED,
                            rx * spacing,
                            rz * spacing,
                        ) else {
                            continue;
                        };
                        if !in_view(at.x, at.z)
                            || !self
                                .sets
                                .is_structure_chunk(set, model::SEED, at.x, at.z, &rings)
                        {
                            continue;
                        }
                        if candidates.len() < 512 {
                            candidates.insert((at.x, at.z));
                        } else {
                            limited = true;
                        }
                    }
                }
            } else {
                if canceled() {
                    return Err("canceled".into());
                }
                for at in worker.ring_positions(&set.name).unwrap_or_default() {
                    if in_view(at.x, at.z) {
                        if candidates.len() < 512 {
                            candidates.insert((at.x, at.z));
                        } else {
                            limited = true;
                        }
                    }
                }
            }
        }
        let mut marks = Vec::new();
        for (x, z) in candidates {
            if canceled() {
                return Err("canceled".into());
            }
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
        worker.release();
        Ok((panel, catalog))
    }
}
