// World map / Seed Map / minimap images as client dynamic textures in the GUI atlas pool.
// Game-side module (private Game fields reachable). Replaces the dropped HudAtlas::slots hook:
// the images are created lazily under the names the shared code looks up in
// the GUI atlas's live texture table, and `DynamicTextures::update` keeps
// the last image so an atlas rebuild (resource reload / renderer rebuild) re-patches it.
//
// The 2026-11-19 Nightly also removed the engine minimap (its drawing, sprite slot and terrain
// scanner), so the live minimap is a third image here: `claim_borders_map_backend` composes the
// pixels from `claim_borders_minimap_scan`'s tiles, and `map_view`'s HUD pass draws it.
use crate::game::Game;
use mod_api::Identifier;
use std::sync::atomic::{AtomicBool, Ordering};

pub const WORLD_MAP: &str = "draimhorizon:dynamic/world_map";
pub const SEED_MAP: &str = "draimhorizon:dynamic/seed_map";
pub const MINIMAP: &str = "draimhorizon:dynamic/minimap";
/// Set once the minimap image holds pixels the worker composed (the engine's
/// `Extras.minimap.image.is_empty()` the shared code used to read).
static MINIMAP_UPLOADED: AtomicBool = AtomicBool::new(false);
/// Owner namespace of the ids; the budget (16 images, 4 MiB) is counted per namespace.
const OWNER: &str = "draimhorizon";
/// Fixed sides: world map = full-screen worker image (map_worker.rs query size 768),
/// seed map = seed_worker::SIDE, minimap = claim_borders_minimap_scan::SIZE. A fixed size keeps
/// each image in one slot class
/// (a class change is refused by `update`). 768 -> 1024 class, 256 -> 256 class.
const IMAGES: [(&str, u32); 3] = [
    (SEED_MAP, 256),
    (WORLD_MAP, 768),
    (
        MINIMAP,
        crate::game::claim_borders_minimap_scan::SIZE as u32,
    ),
];

static WARNED: AtomicBool = AtomicBool::new(false);

fn warn(what: &str, error: impl std::fmt::Debug) {
    if !WARNED.swap(true, Ordering::Relaxed) {
        log::warn!("draimhorizon: map image {what} failed: {error:?}");
        eprintln!("[draimhorizon] map image {what} failed: {error:?}");
    }
}

/// Create a transparent dynamic texture for each image that is missing.
pub fn ensure(game: &mut Game<'_>) {
    for (name, side) in IMAGES {
        if game.gui.atlas.live.contains_key(name) {
            continue;
        }
        let Ok(id) = Identifier::parse(name) else {
            continue;
        };
        let rgba = vec![0u8; side as usize * side as usize * 4];
        let (pool, mut names) = game.gui.atlas.dynamic_names();
        if let Err(error) =
            game.dynamic_textures
                .create(pool, &mut names, Some(OWNER), &id, [side, side], &rgba)
        {
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
    let (pool, mut names) = game.gui.atlas.dynamic_names();
    match game
        .dynamic_textures
        .update(pool, &mut names, Some(OWNER), &id, [side, side], rgba)
    {
        Ok(()) => {
            if name == MINIMAP {
                MINIMAP_UPLOADED.store(true, Ordering::Relaxed);
            }
            true
        }
        Err(error) => {
            warn("update", error);
            false
        }
    }
}

/// Whether the minimap image holds composed pixels (the engine's `Extras.minimap.image` used to
/// say this before its minimap was removed).
pub fn has_minimap() -> bool {
    MINIMAP_UPLOADED.load(Ordering::Relaxed)
}

/// Draw the live minimap image at `[x, y, size]`; the shared HUD pass draws its marks and frame on
/// top of it.
pub fn draw_minimap(
    game: &Game<'_>,
    batch: &mut sunrise_client::hud::HudBatch,
    [x, y, size]: [f32; 3],
) {
    if !has_minimap() {
        return;
    }
    batch.sprite_scaled(&game.gui.atlas, MINIMAP, x, y, size, size, u32::MAX);
}
