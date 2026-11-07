// One background worker coalesces terrain updates and draws the latest requested view.
use super::claim_borders_map_history::{Context, Render, Store, Tile};
use std::{
    collections::BTreeMap,
    path::PathBuf,
    sync::{Arc, Condvar, Mutex},
    time::{Duration, Instant},
};
#[derive(Clone, PartialEq)]
pub struct Query {
    pub context: Context,
    pub render: Render,
    pub full: bool,
    pub browse_saved_layer: bool,
    pub refresh: bool,
}
pub struct Image {
    pub query: Query,
    pub pixels: Vec<u8>,
    pub dimensions: Vec<String>,
    pub warning: Option<String>,
}
#[derive(Default)]
struct Pending {
    probes: std::collections::VecDeque<(u64, Context, [f64; 2])>,
    heights: std::collections::VecDeque<(u64, Context, [f64; 3])>,
    tiles: BTreeMap<(Context, (i32, i32)), Tile>,
    query: Option<Query>,
    output: Option<Image>,
    flush: bool,
    flush_id: u64,
    flushed: u64,
    stopping: bool,
}
pub struct Handle {
    shared: Arc<(Mutex<Pending>, Condvar)>,
}
impl Handle {
    pub fn spawn(root: PathBuf, palette: Vec<u32>) -> Result<Self, String> {
        let shared = Arc::new((Mutex::new(Pending::default()), Condvar::new()));
        let worker = shared.clone();
        std::thread::Builder::new()
            .name("claim-map-history".into())
            .spawn(move || run(worker, root, palette))
            .map_err(|e| e.to_string())?;
        Ok(Self { shared })
    }
    // A full inbox leaves the producer's fingerprint unchanged, so it retries next frame.
    pub fn tile(&self, context: &Context, chunk: (i32, i32), tile: Tile) -> bool {
        let mut p = self.shared.0.lock().unwrap_or_else(|e| e.into_inner());
        let key = (context.clone(), chunk);
        if p.tiles.len() >= 2048 && !p.tiles.contains_key(&key) {
            return false;
        }
        p.tiles.insert(key, tile);
        self.shared.1.notify_one();
        true
    }
    pub fn query(&self, mut query: Query) {
        if query.full && query.render.size == 512 {
            // 50% overscan at the original texel density. Camera centres are
            // world-aligned, so sub-cell dragging reuses exactly the same query.
            let bpp = query.render.bpp;
            for axis in 0..2 {
                let aspect = if axis == 0 { query.render.aspect } else { 1.0 };
                let cell = 32.0 * bpp * aspect;
                if cell.is_finite() && cell > 0.0 {
                    query.render.center[axis] = ((query.render.center[axis] / cell).round() * cell)
                        .clamp(-29_999_984.0, 29_999_984.0);
                }
            }
            query.render.bpp *= 1.5;
            query.render.size = 768;
        }
        let mut p = self.shared.0.lock().unwrap_or_else(|e| e.into_inner());
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
    pub fn probe(&self, token: u64, context: Context, position: [f64; 2]) -> bool {
        let mut p = self.shared.0.lock().unwrap_or_else(|e| e.into_inner());
        if p.probes.len() >= 8 {
            return false;
        }
        p.probes.push_back((token, context, position));
        self.shared.1.notify_all();
        true
    }
    pub fn height(&self) -> Option<(u64, Context, [f64; 3])> {
        self.shared
            .0
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .heights
            .pop_front()
    }
    pub fn flush(&self) {
        let mut p = self.shared.0.lock().unwrap_or_else(|e| e.into_inner());
        p.flush = true;
        p.flush_id = p.flush_id.wrapping_add(1);
        self.shared.1.notify_all();
    }
    pub fn finish(&self) {
        let mut p = self.shared.0.lock().unwrap_or_else(|e| e.into_inner());
        p.flush = true;
        p.flush_id = p.flush_id.wrapping_add(1);
        let target = p.flush_id;
        self.shared.1.notify_all();
        // Only application shutdown waits; normal rendering never waits for disk I/O.
        let _guard = self
            .shared
            .1
            .wait_timeout_while(p, Duration::from_secs(3), |p| p.flushed < target)
            .unwrap_or_else(|e| e.into_inner());
    }
}
impl Drop for Handle {
    fn drop(&mut self) {
        let mut p = self.shared.0.lock().unwrap_or_else(|e| e.into_inner());
        p.stopping = true;
        self.shared.1.notify_one();
    }
}
fn run(shared: Arc<(Mutex<Pending>, Condvar)>, root: PathBuf, palette: Vec<u32>) {
    let mut store = Store::new(root, palette);
    let mut last_flush = Instant::now();
    let mut warning = None;
    let mut last_dimensions = Instant::now() - Duration::from_secs(60);
    let mut dimensions = Vec::new();
    let mut dimension_scope = String::new();
    let mut revision = 0u64;
    let mut cached: Option<(Query, u64, Context)> = None;
    let mut failed = BTreeMap::new();
    let mut last_retry = Instant::now();
    loop {
        let (mut tiles, query, flush, flush_id, stopping) = {
            let mut p = shared.0.lock().unwrap_or_else(|e| e.into_inner());
            if p.tiles.is_empty()
                && p.query.is_none()
                && p.probes.is_empty()
                && !p.flush
                && !p.stopping
            {
                p = shared
                    .1
                    .wait_timeout(p, Duration::from_millis(500))
                    .unwrap_or_else(|e| e.into_inner())
                    .0;
            }
            (
                std::mem::take(&mut p.tiles),
                p.query.take(),
                std::mem::take(&mut p.flush),
                p.flush_id,
                p.stopping,
            )
        };
        if flush || stopping || last_retry.elapsed() >= Duration::from_secs(1) {
            let mut retry = std::mem::take(&mut failed);
            retry.extend(tiles);
            tiles = retry;
            last_retry = Instant::now();
        }
        if !tiles.is_empty() {
            revision = revision.wrapping_add(1);
        }
        for ((context, chunk), tile) in tiles {
            if let Err(e) = store.insert(&context, chunk, tile.clone()) {
                warning = Some(e);
                if failed.len() < 2048 {
                    failed.insert((context, chunk), tile);
                }
            }
        }
        if flush || stopping || last_flush.elapsed() >= Duration::from_secs(1) {
            if let Err(e) = store.flush() {
                warning = Some(e);
            }
            last_flush = Instant::now();
            let mut p = shared.0.lock().unwrap_or_else(|e| e.into_inner());
            p.flushed = p.flushed.max(flush_id);
            shared.1.notify_all();
        }
        let probes = {
            let mut p = shared.0.lock().unwrap_or_else(|e| e.into_inner());
            std::mem::take(&mut p.probes)
        };
        for (token, context, position) in probes {
            let saved = store.saved_context(&context);
            let height = store
                .height_at(
                    &saved,
                    position[0].floor() as i32,
                    position[1].floor() as i32,
                )
                .ok()
                .flatten()
                .map_or(64.0, |h| f64::from(h) + 1.0);
            let mut p = shared.0.lock().unwrap_or_else(|e| e.into_inner());
            if p.heights.len() >= 8 {
                p.heights.pop_front();
            }
            p.heights
                .push_back((token, context, [position[0], height, position[1]]));
        }
        if let Some(mut query) = query {
            let refresh = std::mem::take(&mut query.refresh);
            if dimension_scope != query.context.scope
                || last_dimensions.elapsed() > Duration::from_secs(10)
            {
                dimensions = store.dimensions(&query.context.scope);
                dimension_scope = query.context.scope.clone();
                last_dimensions = Instant::now();
            }
            let render_context = if query.browse_saved_layer {
                store.saved_context(&query.context)
            } else {
                query.context.clone()
            };
            if !refresh
                && cached.as_ref().is_some_and(|(old, at, context)| {
                    *old == query && *at == revision && *context == render_context
                })
            {
                continue;
            }
            let composed = store.compose(&render_context, &query.render);
            match composed {
                Ok(pixels) => {
                    cached = Some((query.clone(), revision, render_context));
                    if let Some(error) = store.warning.take() {
                        warning = Some(error);
                    }
                    shared.0.lock().unwrap_or_else(|e| e.into_inner()).output = Some(Image {
                        query,
                        pixels,
                        dimensions: dimensions.clone(),
                        warning: warning.clone(),
                    });
                }
                Err(e) => {
                    warning = Some(e);
                    let size = query.render.size.clamp(64, 768);
                    let mut pixels = vec![0; size * size * 4];
                    for p in pixels.as_chunks_mut::<4>().0 {
                        p.copy_from_slice(&[24, 31, 44, 255]);
                    }
                    shared.0.lock().unwrap_or_else(|e| e.into_inner()).output = Some(Image {
                        query,
                        pixels,
                        dimensions: dimensions.clone(),
                        warning: warning.clone(),
                    });
                }
            }
        }
        if stopping {
            let _ = store.flush();
            break;
        }
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicU64, Ordering};
    fn await_image(handle: &Handle) -> Image {
        let deadline = Instant::now() + Duration::from_secs(2);
        loop {
            if let Some(image) = handle.take() {
                return image;
            }
            assert!(
                Instant::now() < deadline,
                "map worker did not return an image"
            );
            std::thread::sleep(Duration::from_millis(2));
        }
    }
    #[test]
    fn full_map_requests_reserve_a_stable_pan_margin() {
        let context = Context {
            scope: "pan-margin-test".into(),
            dimension: "minecraft:overworld".into(),
            layer: i32::MAX,
            persistent: false,
        };
        let handle =
            Handle::spawn(std::env::temp_dir().join("unused-map-pan-test"), vec![0]).unwrap();
        let query = Query {
            context,
            render: Render {
                center: [-100.25, 70.125],
                bpp: 2.0,
                aspect: 2.0,
                size: 512,
                shading: true,
            },
            full: true,
            browse_saved_layer: false,
            refresh: false,
        };
        handle.query(query.clone());
        let image = await_image(&handle);
        assert_eq!(
            image.query.render.size, 768,
            "keep the original visible texel density while overscanning"
        );
        assert_eq!(image.query.render.bpp, 3.0);
        for axis in 0..2 {
            let aspect = if axis == 0 { 2.0 } else { 1.0 };
            let margin = 64.0 * (image.query.render.bpp - query.render.bpp) * aspect;
            assert!((image.query.render.center[axis] - query.render.center[axis]).abs() < margin);
        }
        assert_eq!(image.pixels.len(), 768 * 768 * 4);
    }
    #[test]
    fn fractional_pan_does_not_republish_unchanged_terrain() {
        let context = Context {
            scope: "pan-reuse-test".into(),
            dimension: "minecraft:overworld".into(),
            layer: i32::MAX,
            persistent: false,
        };
        let handle = Handle::spawn(
            std::env::temp_dir().join("unused-map-reuse-test"),
            vec![0, 0x809040],
        )
        .unwrap();
        let mut query = Query {
            context: context.clone(),
            render: Render {
                center: [1.0, 1.0],
                bpp: 2.0,
                aspect: 2.0,
                size: 512,
                shading: true,
            },
            full: true,
            browse_saved_layer: false,
            refresh: false,
        };
        handle.query(query.clone());
        await_image(&handle);
        query.render.center[0] += 0.125;
        handle.query(query.clone());
        let deadline = Instant::now() + Duration::from_secs(2);
        while handle.shared.0.lock().unwrap().query.is_some() {
            assert!(Instant::now() < deadline);
            std::thread::sleep(Duration::from_millis(2));
        }
        // A flush is a worker barrier on the following iteration. It does not
        // advance terrain revision and must not cause a duplicate atlas upload.
        handle.finish();
        assert!(
            handle.take().is_none(),
            "fractional pan republished the backing image"
        );
        assert!(handle.tile(
            &context,
            (0, 0),
            Tile {
                color: [1; 256],
                height: [64; 256],
                depth: [0; 256],
                rgb: [0; 256],
            }
        ));
        handle.query(query.clone());
        let changed = await_image(&handle);
        assert!(
            changed
                .pixels
                .as_chunks::<4>()
                .0
                .contains(&[128, 144, 64, 255])
        );
        // Reopening clears the GUI publication, not the terrain itself.
        query.refresh = true;
        handle.query(query.clone());
        await_image(&handle);
        query.refresh = false;
        query.render.center[0] += 128.0;
        handle.query(query);
        await_image(&handle);
    }
    #[test]
    fn exit_barrier_persists_pending_tiles_before_returning() {
        static NEXT: AtomicU64 = AtomicU64::new(0);
        let root = std::env::temp_dir().join(format!(
            "horizon-atlas-worker-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        let context = Context {
            scope: "worker-test".into(),
            dimension: "minecraft:overworld".into(),
            layer: i32::MAX,
            persistent: true,
        };
        let handle = Handle::spawn(root.clone(), vec![0, 0x55aa77]).unwrap();
        assert!(handle.tile(
            &context,
            (-1, 0),
            Tile {
                color: [1; 256],
                height: [64; 256],
                rgb: [0; 256],
                depth: [0; 256]
            }
        ));
        handle.finish();
        let mut restored = Store::new(root.clone(), vec![0, 0x55aa77]);
        let image = restored
            .compose(
                &context,
                &Render {
                    center: [-8.0, 8.0],
                    bpp: 0.25,
                    aspect: 1.0,
                    size: 64,
                    shading: true,
                },
            )
            .unwrap();
        assert!(image.as_chunks::<4>().0.contains(&[85, 170, 119, 255]));
        drop(handle);
        let _ = std::fs::remove_dir_all(root);
    }
}
