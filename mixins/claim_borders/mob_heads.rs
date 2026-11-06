// Actual entity heads from Horizon's baked models and current texture atlas.
// No bundled Minecraft textures, image decoding, skin downloads or invented face glyphs.
use crate::game::{Game, claim_borders_map as model};
use horizon_client::{
    hud::HudBatch,
    mobs::{Mob, Part},
};
use std::{
    collections::BTreeMap,
    sync::{Arc, Mutex, OnceLock},
};
#[derive(Default)]
struct Cache {
    generation: u64,
    heads: BTreeMap<usize, Arc<Vec<model::HeadQuad>>>,
}
fn find_head(part: &Part) -> Option<&Part> {
    if matches!(
        part.name.as_str(),
        "head_parts" | "head" | "head1" | "main_head" | "skull"
    ) {
        return Some(part);
    }
    part.children.iter().find_map(find_head)
}
fn find_named<'a>(part: &'a Part, name: &str) -> Option<&'a Part> {
    if part.name == name {
        return Some(part);
    }
    part.children
        .iter()
        .find_map(|child| find_named(child, name))
}
fn trim_appendages(part: &mut Part) {
    part.children.retain(|child| {
        !["tentacle", "leg", "tail", "wing", "fin"]
            .iter()
            .any(|name| child.name.contains(name))
    });
    for child in &mut part.children {
        trim_appendages(child);
    }
}
fn icon(game: &Game<'_>, mob: &Mob) -> Arc<Vec<model::HeadQuad>> {
    let (layer, _) = horizon_client::mobs::render::body(mob);
    let root = mob.kind.layer(&layer).unwrap_or(&mob.kind.model);
    let key = Arc::as_ptr(root) as usize;
    static CACHE: OnceLock<Mutex<Cache>> = OnceLock::new();
    let mut cache = CACHE
        .get_or_init(|| Mutex::new(Cache::default()))
        .lock()
        .unwrap_or_else(|e| e.into_inner());
    if cache.generation != game.pack.render.generation {
        cache.heads.clear();
        cache.generation = game.pack.render.generation;
    }
    if let Some(icon) = cache.heads.get(&key) {
        return icon.clone();
    }
    let mut vertices = Vec::new();
    if let Some(head) = find_head(root) {
        horizon_client::mobs::render_model(
            head,
            glam::Mat4::IDENTITY,
            &|part| part.pose,
            [0.0, 0.0, 1.0, 1.0],
            0,
            0,
            &mut vertices,
        );
        // ChickenModel keeps its beak and wattle beside (rather than inside) the head.
        if mob.kind.id == "minecraft:chicken" {
            for name in ["beak", "red_thing"] {
                if let Some(part) = find_named(root, name) {
                    horizon_client::mobs::render_model(
                        part,
                        glam::Mat4::IDENTITY,
                        &|part| part.pose,
                        [0.0, 0.0, 1.0, 1.0],
                        0,
                        0,
                        &mut vertices,
                    );
                }
            }
        }
    } else {
        // Headless body-shaped mobs keep their eyes/mouth/cube slices, without appendages.
        let mut face = (**root).clone();
        trim_appendages(&mut face);
        horizon_client::mobs::render_model(
            &face,
            glam::Mat4::IDENTITY,
            &|part| part.pose,
            [0.0, 0.0, 1.0, 1.0],
            0,
            0,
            &mut vertices,
        );
    }

    let faces = vertices
        .chunks_exact(4)
        .filter(|quad| quad[0].normal[2] < 0)
        .map(|quad| model::HeadFace {
            position: std::array::from_fn(|i| quad[i].pos),
            uv: std::array::from_fn(|i| quad[i].uv),
        })
        .collect();
    let icon = Arc::new(model::project_head(faces));
    if cache.heads.len() >= 128 {
        cache.heads.clear();
    }
    cache.heads.insert(key, icon.clone());
    icon
}
pub(super) fn draw(
    game: &Game<'_>,
    batch: &mut HudBatch,
    mob: &Mob,
    center: [f32; 2],
    size: f32,
    alpha: u32,
) {
    let (_, texture) = horizon_client::mobs::render::body(mob);
    let Some(sprite) = game.gui.atlas.sprites.get(texture.as_ref()) else {
        return;
    };
    for quad in icon(game, mob).iter() {
        let position = quad
            .position
            .map(|p| [center[0] + p[0] * size, center[1] + p[1] * size]);
        let uv = quad.uv.map(|uv| {
            [
                sprite.u0 + (sprite.u1 - sprite.u0) * uv[0],
                sprite.v0 + (sprite.v1 - sprite.v0) * uv[1],
            ]
        });
        batch.quad(position, uv, (alpha << 24) | 0x00ff_ffff);
    }
}
