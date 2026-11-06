// Isolated, cancellable ore calculation. Generated chunks never enter the live world.
use crate::game::{claim_borders_experimental as model, claim_borders_seed_map as seed};
use horizon_client::chunk::Chunk;
use horizon_core::position::ChunkPos;
use horizon_world::{
    registry::BlockRegistry,
    worldgen::generator::{GeneratorWorker, OverworldGenerator, ProtoChunk},
};
use std::{
    collections::BTreeMap,
    path::PathBuf,
    sync::{Arc, Condvar, Mutex},
};
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Key {
    pub scope: String,
    pub dimension: String,
    pub center: (i32, i32),
    pub vertical_band: i32,
    pub ores: u16,
    pub prediction: bool,
}
pub struct Query {
    pub key: Key,
    pub player: [f64; 3],
    pub chunks: Vec<((i32, i32), Arc<Chunk>)>,
}
pub struct Result {
    pub key: Key,
    pub marks: Vec<model::Mark>,
    pub error: Option<String>,
}
#[derive(Default)]
struct Pending {
    query: Option<Query>,
    output: Option<Result>,
    generation: u64,
    stop: bool,
}
pub struct Handle {
    shared: Arc<(Mutex<Pending>, Condvar)>,
}
impl Handle {
    pub fn spawn(
        dir: PathBuf,
        registry: &'static BlockRegistry,
    ) -> std::result::Result<Self, String> {
        let shared = Arc::new((Mutex::new(Pending::default()), Condvar::new()));
        let inbox = shared.clone();
        std::thread::Builder::new()
            .name("ore-experiment".into())
            .spawn(move || {
                horizon_threads::idle_current();
                let mut generators = BTreeMap::<String, (GeneratorWorker, (i32, i32))>::new();
                let mut table = (0u16, Vec::<Option<u8>>::new());
                loop {
                    let (query, at) = {
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
                        p.stop || p.generation != at
                    };
                    if table.0 != query.key.ores || table.1.is_empty() {
                        let prefs = model::Preferences {
                            ores: query.key.ores,
                            ..model::Preferences::default()
                        };
                        table = (
                            query.key.ores,
                            (0..registry.len())
                                .map(|i| {
                                    registry
                                        .state(horizon_world::registry::BlockStateId(i as u32))
                                        .and_then(|s| prefs.classify(&s.name))
                                })
                                .collect(),
                        );
                    }
                    let result = if query.key.ores == 0 {
                        Ok(Vec::new())
                    } else if query.key.prediction {
                        predict(&dir, registry, &mut generators, &query, &table.1, &canceled)
                    } else {
                        scan(&query, &table.1, &canceled)
                    };
                    if canceled() {
                        continue;
                    }
                    let (marks, error) = match result {
                        Ok(m) => (model::nearest(m, query.player), None),
                        Err(e) => (Vec::new(), Some(e)),
                    };
                    let mut p = inbox.0.lock().unwrap_or_else(|e| e.into_inner());
                    if p.generation == at && !p.stop {
                        p.output = Some(Result {
                            key: query.key,
                            marks,
                            error,
                        });
                    }
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
    pub fn take(&self) -> Option<Result> {
        self.shared
            .0
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .output
            .take()
    }
}
impl Drop for Handle {
    fn drop(&mut self) {
        self.shared.0.lock().unwrap_or_else(|e| e.into_inner()).stop = true;
        self.shared.1.notify_one();
    }
}
fn scan(
    q: &Query,
    table: &[Option<u8>],
    canceled: &impl Fn() -> bool,
) -> std::result::Result<Vec<model::Mark>, String> {
    let mut marks = Vec::new();
    for (at, chunk) in &q.chunks {
        for y in chunk.min_y()..chunk.max_y() {
            if canceled() {
                return Err("canceled".into());
            }
            if (f64::from(y) - q.player[1]).abs() > model::RANGE {
                continue;
            }
            for z in 0..16 {
                for x in 0..16 {
                    if let Some(kind) = table.get(chunk.get(x, y, z) as usize).copied().flatten() {
                        marks.push(model::Mark {
                            position: [at.0 * 16 + x, y, at.1 * 16 + z],
                            kind,
                            predicted: false,
                        });
                    }
                }
            }
        }
    }
    Ok(marks)
}
fn predict(
    dir: &std::path::Path,
    registry: &'static BlockRegistry,
    generators: &mut BTreeMap<String, (GeneratorWorker, (i32, i32))>,
    q: &Query,
    table: &[Option<u8>],
    canceled: &impl Fn() -> bool,
) -> std::result::Result<Vec<model::Mark>, String> {
    if q.key.dimension == "minecraft:the_end" {
        return Ok(Vec::new());
    }
    if !matches!(
        q.key.dimension.as_str(),
        "minecraft:overworld" | "minecraft:the_nether"
    ) {
        return Err("Прогноз доступен только для обычного мира и Незера".into());
    }
    if !generators.contains_key(&q.key.dimension) {
        let data =
            horizon_client::vanillagen::shared_data(dir, registry).map_err(|e| e.to_string())?;
        let generator = OverworldGenerator::for_dimension(
            data,
            seed::SEED,
            "minecraft:normal",
            &q.key.dimension,
        )
        .map_err(|e| e.to_string())?;
        generators.insert(
            q.key.dimension.clone(),
            (generator.worker(), generator.level()),
        );
    }
    let (worker, (min, height)) = generators.get_mut(&q.key.dimension).unwrap();
    let result = (|| {
        let (cx, cz) = q.key.center;
        let mut terrain = BTreeMap::<(i32, i32), ProtoChunk>::new();
        // Decorate every neighbour capable of placing a feature into the middle chunk.
        // Each of those centres requires a 3x3 terrain region, giving a 5x5 halo overall.
        for z in cz - 2..=cz + 2 {
            for x in cx - 2..=cx + 2 {
                if canceled() {
                    return Err("canceled".into());
                }
                let chunk = worker
                    .generate_proto(ChunkPos::new(x, z))
                    .map_err(|e| e.to_string())?;
                terrain.insert((x, z), chunk);
            }
        }
        for z in cz - 1..=cz + 1 {
            for x in cx - 1..=cx + 1 {
                if canceled() {
                    return Err("canceled".into());
                }
                let positions: [(i32, i32); 9] =
                    std::array::from_fn(|i| (x + i as i32 % 3 - 1, z + i as i32 / 3 - 1));
                let mut region: [ProtoChunk; 9] = std::array::from_fn(|i| {
                    terrain.remove(&positions[i]).expect("5x5 terrain halo")
                });
                worker.decorate(&mut region).map_err(|e| e.to_string())?;
                for (at, chunk) in positions.into_iter().zip(region) {
                    terrain.insert(at, chunk);
                }
            }
        }
        let chunk = &terrain[&(cx, cz)];
        let mut marks = Vec::new();
        for y in *min..*min + *height {
            if canceled() {
                return Err("canceled".into());
            }
            if (f64::from(y) - q.player[1]).abs() > model::RANGE {
                continue;
            }
            for z in 0..16 {
                for x in 0..16 {
                    if let Some(kind) = table
                        .get(chunk.block(x, y, z).0 as usize)
                        .copied()
                        .flatten()
                    {
                        marks.push(model::Mark {
                            position: [cx * 16 + x, y, cz * 16 + z],
                            kind,
                            predicted: true,
                        });
                    }
                }
            }
        }
        Ok(marks)
    })();
    worker.release();
    worker.forget_structure_starts();
    result
}
