// World map / Seed Map images as client dynamic textures in the GUI atlas pool.
// Game-side module (private Game fields reachable). Replaces the dropped HudAtlas::slots hook:
// the two images are created lazily under the names the shared code looks up in
// `game.gui.atlas.sprites`, and pixels are delivered with `DynamicTextures::update`, which keeps
// the last image so an atlas rebuild (resource reload / renderer rebuild) re-patches it.
use crate::game::Game;
use mod_api::Identifier;
use std::sync::atomic::{AtomicBool, Ordering};

pub const WORLD_MAP: &str = "draimhorizon:dynamic/world_map";
pub const SEED_MAP: &str = "draimhorizon:dynamic/seed_map";
/// Owner namespace of the ids; the budget (16 images, 4 MiB) is counted per namespace.
const OWNER: &str = "draimhorizon";
/// Fixed sides: world map = full-screen worker image (map_worker.rs query size 768),
/// seed map = seed_worker::SIDE. A fixed size keeps each image in one slot class
/// (a class change is refused by `update`). 768 -> 1024 class, 256 -> 256 class.
const IMAGES: [(&str, u32); 2] = [(SEED_MAP, 256), (WORLD_MAP, 768)];

static WARNED: AtomicBool = AtomicBool::new(false);

fn warn(what: &str, error: impl std::fmt::Debug) {
    if !WARNED.swap(true, Ordering::Relaxed) {
        eprintln!("[draimhorizon] map image {what} failed: {error:?}");
    }
}

/// Create the images that are missing (transparent). Cheap when both exist.
pub fn ensure(game: &mut Game<'_>) {
    for (name, side) in IMAGES {
        if game.gui.atlas.sprites.contains_key(name) {
            continue;
        }
        let Ok(id) = Identifier::parse(name) else {
            continue;
        };
        let rgba = vec![0u8; side as usize * side as usize * 4];
        let atlas = &mut game.gui.atlas;
        if let Err(error) = game.dynamic_textures.create(
            atlas.dynamic_texture_pool.as_ref(),
            &mut atlas.sprites,
            Some(OWNER),
            &id,
            [side, side],
            &rgba,
        ) {
            warn("create", error);
        }
    }
}

/// Replace the whole image `name` with `rgba` (side x side x 4 bytes). False when the pixels do
/// not fit or the pool refuses (caller then must not mark the frame as published).
pub fn upload(game: &mut Game<'_>, name: &str, rgba: &[u8]) -> bool {
    let Some(&(_, side)) = IMAGES.iter().find(|(n, _)| *n == name) else {
        return false;
    };
    if rgba.len() != side as usize * side as usize * 4 {
        return false;
    }
    ensure(game);
    let Ok(id) = Identifier::parse(name) else {
        return false;
    };
    let atlas = &mut game.gui.atlas;
    match game.dynamic_textures.update(
        atlas.dynamic_texture_pool.as_ref(),
        &mut atlas.sprites,
        Some(OWNER),
        &id,
        [side, side],
        rgba,
    ) {
        Ok(()) => true,
        Err(error) => {
            warn("update", error);
            false
        }
    }
}
