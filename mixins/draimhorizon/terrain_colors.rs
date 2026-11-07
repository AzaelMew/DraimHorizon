// Resource-pack top-face colours, prepared off-thread and sampled only from loaded chunks.
use crate::game::Game;
use horizon_client::assets::RenderAssets;
use horizon_world::registry::{BlockRegistry, BlockStateId};
use std::sync::{Arc, Mutex, OnceLock};
#[derive(Clone, Copy, Default)]
pub(super) struct Material {
    rgb: u32,
    tint: u8,
}
#[derive(Default)]
struct State {
    generation: u64,
    palette: Option<Arc<Vec<Material>>>,
    task: Option<(u64, std::thread::JoinHandle<Vec<Material>>)>,
}
pub(super) fn palette(game: &Game<'_>) -> Option<(u64, Arc<Vec<Material>>)> {
    static CACHE: OnceLock<Mutex<State>> = OnceLock::new();
    let mut s = CACHE
        .get_or_init(|| Mutex::new(State::default()))
        .lock()
        .unwrap_or_else(|e| e.into_inner());
    let generation = game.pack.render.generation;
    if s.generation != generation {
        s.generation = generation;
        s.palette = None;
    }
    if s.task.as_ref().is_some_and(|(_, task)| task.is_finished()) {
        let (at, task) = s.task.take().unwrap();
        if let Ok(result) = task.join() {
            if at == generation {
                s.palette = Some(Arc::new(result));
            }
        }
    }
    if s.palette.is_none() && s.task.is_none() {
        let render = game.pack.render.clone();
        let registry = game.pack.assets.registry;
        let tables = game.pack.context.tables.clone();
        if let Ok(task) = std::thread::Builder::new()
            .name("map-texture-colours".into())
            .spawn(move || build(&render, registry, &tables))
        {
            s.task = Some((generation, task));
        }
    }
    s.palette.as_ref().map(|p| (generation, p.clone()))
}
fn build(
    render: &RenderAssets,
    registry: &'static BlockRegistry,
    tables: &horizon_client::mesher::MeshTables,
) -> Vec<Material> {
    let pixels = render.pixels.get();
    let stride = (render.sprite_size as usize)
        .saturating_mul(render.sprite_size as usize)
        .saturating_mul(4);
    let mut averages = Vec::new();
    if stride > 0 {
        for layer in pixels.chunks_exact(stride) {
            let mut sum = [0u64; 3];
            let mut alpha = 0u64;
            for p in layer.chunks_exact(4) {
                let a = u64::from(p[3]);
                if a < 32 {
                    continue;
                }
                alpha += a;
                for c in 0..3 {
                    sum[c] += u64::from(p[c]) * a;
                }
            }
            averages.push(if alpha == 0 {
                0
            } else {
                0xff000000
                    | ((sum[0] / alpha) as u32) << 16
                    | ((sum[1] / alpha) as u32) << 8
                    | (sum[2] / alpha) as u32
            });
        }
    }
    (0..registry.len())
        .map(|i| {
            let id = BlockStateId(i as u32);
            let Some(state) = registry.state(id) else {
                return Material::default();
            };
            let name = state.name.as_str();
            if name == "minecraft:water" {
                return Material {
                    rgb: 0xffd0d0d0,
                    tint: 3,
                };
            }
            let Ok(models) = render.model_catalog.collect_seeded(id, 42) else {
                return Material::default();
            };
            let face = models
                .iter()
                .flat_map(|(_, m)| &m.faces)
                .find(|f| f.quad.facing == horizon_assets::model::Direction::Up)
                .or_else(|| models.iter().flat_map(|(_, m)| &m.faces).next());
            let Some(face) = face else {
                return Material::default();
            };
            let mut rgb = averages.get(face.layer as usize).copied().unwrap_or(0);
            if rgb == 0 {
                return Material::default();
            }
            let tint = match tables.tint_source(id, face.quad.tint_index) {
                Some(horizon_client::mesher::TintSource::Grass) => 1,
                Some(horizon_client::mesher::TintSource::Foliage) => 2,
                Some(horizon_client::mesher::TintSource::Water) => 3,
                Some(horizon_client::mesher::TintSource::DryFoliage) => 4,
                Some(horizon_client::mesher::TintSource::Fixed(color)) => {
                    rgb = multiply(rgb, color);
                    0
                }
                None => 0,
            };
            Material { rgb, tint }
        })
        .collect()
}
fn multiply(rgb: u32, tint: u32) -> u32 {
    0xff000000
        | (((rgb >> 16 & 255) * (tint >> 16 & 255) / 255) << 16)
        | (((rgb >> 8 & 255) * (tint >> 8 & 255) / 255) << 8)
        | ((rgb & 255) * (tint & 255) / 255)
}
pub(super) fn tile(
    game: &Game<'_>,
    at: (i32, i32),
    heights: &[i16; 256],
    palette: &[Material],
) -> [u32; 256] {
    let Some(chunk) = game.world.chunks.get(&at) else {
        return [0; 256];
    };
    let biomes = world_biomes!(game);
    std::array::from_fn(|i| {
        let x = (i % 16) as i32;
        let z = (i / 16) as i32;
        let y = i32::from(heights[i]);
        let id = chunk.get(x, y, z) as usize;
        let Some(m) = palette.get(id).filter(|m| m.rgb != 0) else {
            return 0;
        };
        let biome = chunk.biome(x, y, z);
        let b = biomes.get(biome);
        let tint = match m.tint {
            1 => biomes.grass(biome, f64::from(at.0 * 16 + x), f64::from(at.1 * 16 + z)),
            2 => b.foliage,
            3 => b.water,
            4 => b.dry_foliage,
            _ => 0xffffff,
        };
        multiply(m.rgb, tint)
    })
}
