// Native terrain scan on the game thread, disk storage and composition on one worker.
use super::*;
use crate::game::{
    claim_borders_map as model, claim_borders_map_history as history,
    claim_borders_map_view as view, claim_borders_map_worker as worker,
};
use std::{
    collections::{BTreeMap, VecDeque},
    sync::{Mutex, OnceLock},
    time::Instant,
};
struct Cache {
    key: String,
    last_scan: Instant,
    captured_at: Instant,
    last_query: Instant,
    full: bool,
    worker: Option<worker::Handle>,
    seen: BTreeMap<(i32, i32), u64>,
    context: Option<history::Context>,
    texture_generation: Option<u64>,
    live_scope: Option<(String, String)>,
    live_slices: VecDeque<LiveSlice>,
    live_displayed: Option<(Layer, model::View, bool)>,
    live_refresh: Instant,
}
struct LiveSlice {
    layer: Layer,
    scanner: minimap::Minimap,
    tiles: BTreeMap<(i32, i32), history::Tile>,
}
fn state() -> &'static Mutex<Cache> {
    static CACHE: OnceLock<Mutex<Cache>> = OnceLock::new();
    CACHE.get_or_init(|| {
        Mutex::new(Cache {
            key: String::new(),
            last_scan: Instant::now() - Duration::from_secs(1),
            captured_at: Instant::now() - Duration::from_secs(1),
            last_query: Instant::now() - Duration::from_secs(1),
            full: false,
            worker: None,
            seen: BTreeMap::new(),
            context: None,
            texture_generation: None,
            live_scope: None,
            live_slices: VecDeque::new(),
            live_displayed: None,
            live_refresh: Instant::now(),
        })
    })
}
pub(in crate::game) fn waypoint(token: u64, dimension: String, position: [f64; 2]) -> bool {
    let cache = state().lock().unwrap_or_else(|p| p.into_inner());
    let Some(context) = cache.context.clone() else {
        return false;
    };
    cache.worker.as_ref().is_some_and(|w| {
        w.probe(
            token,
            history::Context {
                dimension,
                ..context
            },
            position,
        )
    })
}
pub(in crate::game) fn finish() {
    if let Some(handle) = &state().lock().unwrap_or_else(|p| p.into_inner()).worker {
        handle.finish();
    }
}
pub(in crate::game) fn flush() {
    let mut cache = state().lock().unwrap_or_else(|p| p.into_inner());
    cache.live_scope = None;
    cache.live_slices.clear();
    cache.live_displayed = None;
    if let Some(handle) = &cache.worker {
        handle.flush();
    }
}
fn draw_minimap(
    game: &mut Game<'_>,
    cache: &mut Cache,
    request: &view::Request,
    layer: Layer,
    palette: Option<&[crate::game::claim_borders_terrain_colors::Material]>,
) {
    let scope = (
        request.source.scope.clone(),
        request.source.dimension.clone(),
    );
    if cache.live_scope.as_ref() != Some(&scope) {
        cache.live_scope = Some(scope);
        cache.live_slices.clear();
        cache.live_displayed = None;
    }
    let mut slice = cache
        .live_slices
        .iter()
        .position(|s| s.layer == layer)
        .and_then(|i| cache.live_slices.remove(i))
        .unwrap_or_else(|| LiveSlice {
            layer,
            scanner: minimap::Minimap::default(),
            tiles: BTreeMap::new(),
        });
    let center = request.view.center.map(|p| p.floor() as i32);
    let chunk = (center[0] >> 4, center[1] >> 4);
    let radius = ((request.view.blocks_per_pixel * 4.0).ceil() as i32 + 1)
        .min(game.settings.render_distance.max(2));
    let before = Instant::now();
    let scanned = slice.scanner.update(
        &game.world,
        game.res.facts(),
        chunk,
        radius,
        layer,
        Duration::from_micros(400),
    );
    let old_count = slice.tiles.len();
    slice.tiles.retain(|&(x, z), _| {
        (x - chunk.0).abs() <= radius + 2
            && (z - chunk.1).abs() <= radius + 2
            && game.world.chunks.contains_key(&(x, z))
    });
    let removed = old_count != slice.tiles.len();
    for (at, color, height, depth) in slice.scanner.claim_borders_tiles(before) {
        let rgb = palette.map_or([0; 256], |p| {
            crate::game::claim_borders_terrain_colors::tile(game, at, height, p)
        });
        slice.tiles.insert(
            at,
            history::Tile {
                color: *color,
                height: *height,
                depth: *depth,
                rgb,
            },
        );
    }
    let p = request.preferences;
    let changed = cache.live_displayed.is_none_or(|(old, last, shading)| {
        old != layer
            || shading != p.shading
            || last.blocks_per_pixel != request.view.blocks_per_pixel
            || last
                .center
                .into_iter()
                .zip(request.view.center)
                .any(|(a, b)| (a - b).abs() >= f64::from(p.zoom) / 4.0)
    });
    if (changed
        || removed
        || scanned != 0
        || game.extras.minimap.image.is_empty()
        || cache.live_refresh.elapsed() >= Duration::from_millis(150))
        && let Some(rect) = game.gui.atlas.sprites.get(MINIMAP_SPRITE).copied()
    {
        let size = minimap::SIZE;
        let mut pixels = history::compose_minimap(
            &slice.tiles,
            request.view.center,
            request.view.blocks_per_pixel,
            &horizon_client::blocks::MAP_COLORS,
            p.shading,
        );
        model::paint_aspect(
            &mut pixels,
            size,
            request.view,
            &request.dimension,
            &request.source,
            p,
            1.0,
        );
        let (w, h) = (game.gui.atlas.width as f32, game.gui.atlas.height as f32);
        game.map_patches.push(horizon_client::font::Patch {
            x: (rect.u0 * w).round() as u32,
            y: (rect.v0 * h).round() as u32,
            width: size as u32,
            height: size as u32,
            rgba: pixels,
        });
        if game.extras.minimap.image.is_empty() {
            game.extras.minimap.image = vec![0; size * size * 4];
        }
        cache.live_displayed = Some((layer, request.view, p.shading));
        cache.live_refresh = Instant::now();
        view::mini_published(request.dimension.clone(), request.view);
    }
    cache.live_slices.push_back(slice);
    if cache.live_slices.len() > 4 {
        cache.live_slices.pop_front();
    }
}
pub(in crate::game) fn update(game: &mut Game<'_>) -> bool {
    if let Some(patch) = view::take_seed_patch() {
        game.map_patches.push(patch);
    }
    let Some(request) = view::request(game) else {
        return false;
    };
    let mut cache = state().lock().unwrap_or_else(|p| p.into_inner());
    if cache.worker.is_none() {
        let root = match Game::native_config_dir() {
            Ok(p) => p.join("claim_borders").join("maps"),
            Err(_) => return false,
        };
        match worker::Handle::spawn(root, horizon_client::blocks::MAP_COLORS.to_vec()) {
            Ok(handle) => cache.worker = Some(handle),
            Err(error) => {
                view::history_status(Vec::new(), Some(error));
                return false;
            }
        }
    }
    let texture_palette = crate::game::claim_borders_terrain_colors::palette(game);
    let texture_generation = texture_palette.as_ref().map(|(generation, _)| *generation);
    if texture_generation != cache.texture_generation {
        cache.texture_generation = texture_generation;
        cache.seen.clear();
        cache.live_slices.clear();
        cache.live_displayed = None;
        cache.captured_at = Instant::now() - Duration::from_secs(86400);
    }
    let head = game
        .player
        .as_ref()
        .map_or(request.source.player[1] + 1.62, |p| {
            p.position.y + f64::from(p.dimensions.eye_height)
        })
        .floor() as i32;
    let scan_layer = match request.preferences.layer.scan_ceiling(head) {
        None => Layer::Surface,
        Some(ceiling) => Layer::Below(ceiling),
    };
    let layer_id = match scan_layer {
        Layer::Surface => i32::MAX,
        Layer::Below(y) => y.div_euclid(16),
    };
    let scope = format!(
        "{}#atlas{}",
        request.source.scope, request.preferences.profile
    );
    let key = format!(
        "{}|{}|{:?}",
        scope,
        request.source.dimension,
        (layer_id, request.preferences.remember)
    );
    if cache.key != key {
        game.extras.minimap.clear();
        cache.key = key;
        cache.seen.clear();
        cache.captured_at = Instant::now() - Duration::from_secs(1);
        cache.last_scan = Instant::now() - Duration::from_secs(1);
        cache.last_query = Instant::now() - Duration::from_secs(1);
    }
    let live_context = history::Context {
        scope,
        dimension: request.source.dimension.clone(),
        layer: layer_id,
        persistent: request.preferences.remember,
    };
    if cache.last_scan.elapsed() >= Duration::from_millis(50) {
        cache.last_scan = Instant::now();
        let center = (
            model::origin(request.source.player[0]).div_euclid(16),
            model::origin(request.source.player[2]).div_euclid(16),
        );
        let before_scan = Instant::now();
        let mut accepted = true;
        game.extras.minimap.update(
            &game.world,
            game.res.facts(),
            center,
            game.settings.render_distance.clamp(2, 32),
            scan_layer,
            Duration::from_micros(400),
        );
        let tiles: Vec<_> = game
            .extras
            .minimap
            .claim_borders_tiles(cache.captured_at)
            .map(|(at, c, h, d)| {
                (
                    at,
                    history::Tile {
                        rgb: [0; 256],
                        color: *c,
                        height: *h,
                        depth: *d,
                    },
                )
            })
            .collect();
        let mut captured = 0;
        for (at, mut tile) in tiles {
            let stamp = tile.fingerprint();
            if cache.seen.get(&at) == Some(&stamp) {
                continue;
            }
            if captured >= 16 {
                accepted = false;
                continue;
            }
            captured += 1;
            if let Some((_, palette)) = &texture_palette {
                tile.rgb = crate::game::claim_borders_terrain_colors::tile(
                    game,
                    at,
                    &tile.height,
                    palette,
                );
            }
            if cache.worker.as_ref().unwrap().tile(&live_context, at, tile) {
                cache.seen.insert(at, stamp);
            } else {
                accepted = false;
            }
        }
        if accepted {
            cache.captured_at = before_scan;
        }
        cache
            .seen
            .retain(|at, _| game.world.chunks.contains_key(at));
    }
    if !request.full {
        cache.full = false;
        if request.draw {
            let [x, z] = request.view.center.map(|p| p.floor() as i32);
            let covered = game.dim == Dim::Nether || game.world.light(x, head, z) >> 4 == 0;
            let layer = request
                .preferences
                .layer
                .live_minimap_ceiling(head, covered)
                .map_or(Layer::Surface, Layer::Below);
            draw_minimap(
                game,
                &mut cache,
                &request,
                layer,
                texture_palette
                    .as_ref()
                    .map(|(_, palette)| palette.as_slice()),
            );
        }
        return true;
    }
    if !request.draw {
        return true;
    }
    let context = history::Context {
        dimension: request.dimension.clone(),
        layer: layer_id,
        ..live_context
    };
    cache.context = Some(context.clone());
    while let Some((token, c, pos)) = cache.worker.as_ref().unwrap().height() {
        if c.scope == context.scope {
            view::waypoint_ready(token, c.dimension, pos);
        }
    }
    if let Some(image) = cache.worker.as_ref().unwrap().take() {
        if image.query.context.scope == context.scope
            && image.query.context.dimension == context.dimension
            && image.query.context.layer == context.layer
            && (image.query.render.aspect - f64::from(request.aspect)).abs() < 0.001
            && image.query.context.persistent == context.persistent
            && image.query.full == request.full
        {
            let size = image.query.render.size;
            let actual = model::View {
                center: image.query.render.center,
                blocks_per_pixel: image.query.render.bpp as f32,
            };
            let aspect = image.query.render.aspect as f32;
            let pixels = image.pixels;
            if let Some(rect) = game
                .gui
                .atlas
                .sprites
                .get("claim_borders/world_map")
                .copied()
            {
                let (w, h) = (game.gui.atlas.width as f32, game.gui.atlas.height as f32);
                game.map_patches.push(horizon_client::font::Patch {
                    x: (rect.u0 * w).round() as u32,
                    y: (rect.v0 * h).round() as u32,
                    width: size as u32,
                    height: size as u32,
                    rgba: pixels,
                });
                view::published(request.dimension.clone(), actual, aspect);
            }
            view::history_status(image.dimensions, image.warning);
        }
    }
    let changed = cache.full != request.full;
    cache.full = request.full;
    if changed
        || cache.last_query.elapsed() >= Duration::from_millis(if request.full { 100 } else { 50 })
    {
        cache.last_query = Instant::now();
        cache.worker.as_ref().unwrap().query(worker::Query {
            context,
            render: history::Render {
                center: request.view.center,
                bpp: f64::from(request.view.blocks_per_pixel),
                aspect: f64::from(request.aspect),
                size: if request.full { 512 } else { 128 },
                shading: request.preferences.shading,
            },
            full: request.full,
            browse_saved_layer: request.preferences.layer == model::Layer::Cave
                && request.dimension != request.source.dimension,
            refresh: request.refresh,
        });
    }
    true
}
