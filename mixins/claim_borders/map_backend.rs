// Native terrain scan on the game thread, disk storage and composition on one worker.
use super::*;
use crate::game::{
    claim_borders_map as model, claim_borders_map_history as history,
    claim_borders_map_view as view, claim_borders_map_worker as worker,
};
use std::{
    collections::BTreeMap,
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
    if let Some(handle) = &state().lock().unwrap_or_else(|p| p.into_inner()).worker {
        handle.flush();
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
    let scan_layer = match request.preferences.layer {
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
                        color: *c,
                        height: *h,
                        depth: *d,
                    },
                )
            })
            .collect();
        for (at, tile) in tiles {
            let stamp = tile.fingerprint();
            if cache.seen.get(&at) == Some(&stamp) {
                continue;
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
    if !request.draw {
        return true;
    }
    let context = history::Context {
        dimension: request.dimension.clone(),
        layer: if request.dimension == request.source.dimension {
            layer_id
        } else {
            i32::MAX
        },
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
            let mut pixels = image.pixels;
            model::paint_aspect(
                &mut pixels,
                size,
                model::View {
                    center: actual.center,
                    blocks_per_pixel: actual.blocks_per_pixel * 128.0 / size as f32,
                },
                &request.dimension,
                &request.source,
                request.preferences,
                aspect,
            );
            let sprite = if request.full {
                "claim_borders/world_map"
            } else {
                MINIMAP_SPRITE
            };
            if let Some(rect) = game.gui.atlas.sprites.get(sprite).copied() {
                let (w, h) = (game.gui.atlas.width as f32, game.gui.atlas.height as f32);
                game.map_patches.push(horizon_client::font::Patch {
                    x: (rect.u0 * w).round() as u32,
                    y: (rect.v0 * h).round() as u32,
                    width: size as u32,
                    height: size as u32,
                    rgba: pixels,
                });
                if game.extras.minimap.image.is_empty() {
                    game.extras.minimap.image = vec![0; 128 * 128 * 4];
                }
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
            browse_saved_layer: request.dimension != request.source.dimension,
        });
    }
    true
}
