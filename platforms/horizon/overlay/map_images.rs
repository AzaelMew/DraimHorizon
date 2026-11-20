use crate::game::{Game, claim_borders_seed_worker};
use horizon_client::{
    font::Patch,
    hud::{MOD_IMAGE_SPRITE, Rect},
};

pub(super) const WORLD_MAP_SIDE: usize = 768;
const SEED_MAP_SIDE: u32 = claim_borders_seed_worker::SIDE as u32;
const WORLD_MAP: &str = "claim_borders/world_map";
const SEED_MAP: &str = "claim_borders/seed_map";

/// Give the shared screens separate, non-overlapping regions in Horizon's mod-image slot.
pub(super) fn register(game: &mut Game<'_>) {
    let atlas = &mut game.gui.atlas;
    if atlas.sprites.contains_key(WORLD_MAP) && atlas.sprites.contains_key(SEED_MAP) {
        return;
    }
    let Some(slot) = atlas.sprites.get(MOD_IMAGE_SPRITE).copied() else {
        return;
    };
    let world_side = WORLD_MAP_SIDE as u32;
    let dimensions = (atlas.width, atlas.height);
    let (width, height) = slot.texels(dimensions);
    if width < world_side + SEED_MAP_SIDE || height < world_side.max(SEED_MAP_SIDE) {
        return;
    }
    atlas.sprites.insert(
        WORLD_MAP.to_owned(),
        region(slot, dimensions, 0, world_side),
    );
    atlas.sprites.insert(
        SEED_MAP.to_owned(),
        region(slot, dimensions, world_side, SEED_MAP_SIDE),
    );
    crate::game::claim_borders_map_view::invalidate_world_image();
    crate::game::claim_borders_seed_view::invalidate_seed_image();
}

pub(super) fn forget(game: &mut Game<'_>) {
    game.gui.atlas.sprites.remove(WORLD_MAP);
    game.gui.atlas.sprites.remove(SEED_MAP);
}

fn region(slot: Rect, (width, height): (u32, u32), x: u32, side: u32) -> Rect {
    Rect {
        u0: slot.u0 + x as f32 / width as f32,
        v0: slot.v0,
        u1: slot.u0 + (x + side) as f32 / width as f32,
        v1: slot.v0 + side as f32 / height as f32,
        w: side,
        h: side,
    }
}

pub(super) fn seed_patch(game: &Game<'_>, region: Rect, pixels: &[u8]) -> Patch {
    let atlas = &game.gui.atlas;
    Patch {
        x: (region.u0 * atlas.width as f32).round() as u32,
        y: (region.v0 * atlas.height as f32).round() as u32,
        width: SEED_MAP_SIDE,
        height: SEED_MAP_SIDE,
        rgba: pixels.to_vec(),
    }
}
