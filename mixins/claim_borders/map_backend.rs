// Child of game::extras: uses the existing terrain scanner and dynamic atlas slot.
use super::*;
use crate::game::{claim_borders_map as model, claim_borders_map_view as view};
use std::sync::{Mutex, OnceLock};
struct Cache {
    key: String,
    last: std::time::Instant,
    full: bool,
}
pub(in crate::game) fn update(game: &mut Game<'_>) -> bool {
    let Some(request) = view::request(game) else {
        return false;
    };
    static CACHE: OnceLock<Mutex<Cache>> = OnceLock::new();
    let mut cache = CACHE
        .get_or_init(|| {
            Mutex::new(Cache {
                key: String::new(),
                full: false,
                last: std::time::Instant::now() - Duration::from_secs(1),
            })
        })
        .lock()
        .unwrap_or_else(|p| p.into_inner());
    let live = request.dimension == request.source.dimension;
    let head = game
        .player
        .as_ref()
        .map_or(request.source.player[1] + 1.62, |p| {
            p.position.y + f64::from(p.dimensions.eye_height)
        })
        .floor() as i32;
    let covered = game.world.light(
        request.source.player[0].floor() as i32,
        head,
        request.source.player[2].floor() as i32,
    ) >> 4
        == 0;
    let layer = match request.preferences.layer {
        model::Layer::Surface => Layer::Surface,
        model::Layer::Cave => Layer::Below(head),
        model::Layer::Auto => {
            if covered || game.dim == Dim::Nether {
                Layer::Below(head)
            } else {
                Layer::Surface
            }
        }
    };
    let key = format!("{}|{}|{:?}", request.source.scope, request.dimension, layer);
    if cache.key != key {
        game.extras.minimap.clear();
        cache.key = key;
        cache.last = std::time::Instant::now() - Duration::from_secs(1);
    }
    let mode_changed = cache.full != request.full;
    cache.full = request.full;
    if !mode_changed && cache.last.elapsed() < Duration::from_millis(50) {
        return true;
    }
    cache.last = std::time::Instant::now();
    let center = (
        model::origin(request.view.center[0]).div_euclid(16),
        model::origin(request.view.center[1]).div_euclid(16),
    );
    let mut pixels = if live {
        let radius = ((128.0 * request.view.blocks_per_pixel / 32.0).ceil() as i32 + 2)
            .min(game.settings.render_distance.max(2));
        game.extras.minimap.update(
            &game.world,
            game.res.facts(),
            center,
            radius,
            layer,
            Duration::from_micros(400),
        );
        game.extras.minimap.compose(View {
            x: request.view.center[0],
            z: request.view.center[1],
            yaw: 0.0,
            rotate: false,
            blocks_per_pixel: request.view.blocks_per_pixel,
            round: false,
        });
        game.extras.minimap.image.clone()
    } else {
        vec![0; 128 * 128 * 4]
    };
    if pixels.len() != 128 * 128 * 4 {
        pixels.resize(128 * 128 * 4, 0);
    }
    for pixel in pixels.chunks_exact_mut(4) {
        if pixel[3] == 0 {
            pixel.copy_from_slice(&[24, 31, 44, 255]);
        }
    }
    model::paint(
        &mut pixels,
        128,
        request.view,
        &request.dimension,
        &request.source,
        request.preferences,
        request.full,
    );
    // Vanilla's HUD checks this buffer before drawing the slot, including non-live dimensions.
    if game.extras.minimap.image.is_empty() {
        game.extras.minimap.image = pixels.clone();
    }
    if let Some(rect) = game.gui.atlas.sprites.get(MINIMAP_SPRITE).copied() {
        let (w, h) = (game.gui.atlas.width as f32, game.gui.atlas.height as f32);
        game.map_patches.push(horizon_client::font::Patch {
            x: (rect.u0 * w).round() as u32,
            y: (rect.v0 * h).round() as u32,
            width: 128,
            height: 128,
            rgba: pixels,
        });
        view::published(request.dimension, request.view);
    }
    true
}
