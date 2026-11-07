# Explored-map rendering and performance investigation

Base: `9480f7d` (main, v0.22.5). Branch: `investigate/explored-map-performance`.

The investigation below records the original v0.22.5 behavior and map fixes initially packaged as v0.22.6. They are also included in `feature/compact-chat-map-fixes`, combined with `feature/compact-chat`; the combined build is versioned v0.22.6 to match upstream's recent release practice. Source pins and exploration files remain unchanged; the combined changelog/release notes cover both English and Russian. The map-only checks and artifact digest below are historical, not the combined artifact, even though both use v0.22.6.

## Implemented after investigation (0.22.6)

- Fullscreen claims/grid use clipped, screen-space rectangles drawn through `native_fill`, bypassing theme recolouring. Own/friend/foreign precedence and dimension/visibility filters remain intact. The minimap uses the same geometry through a conservative raster adapter.
- Detailed terrain composition copies the west/north height halos from known neighbouring regions before loading the current region. Neighbour loads stay on the worker and preserve the existing 24-region cache and disk validation.
- Full-map queries use a world-aligned 768² backing image with 50% overscan, preserving the old 512² visible texel density. Small fractional drags reuse the same camera key; unchanged queries/revisions no longer clone or publish a full image. A forced refresh handles screen reopening. A changed backing image is 2.25 MiB, so this is bounded buffered-raster reuse, **not** a completed dirty-region GPU tile renderer.
- Broad persistent views use the existing checksummed chunk overviews once their detailed working set exceeds 18 regions, leaving LRU space for incoming terrain and neighbour halos. This trades distant block-level detail for bounded decoding work. It is not a full mip pyramid or a dedicated summary cache.
- The diagnostic harness now asserts corrected border coverage and seam continuity. Its optimized one-claim raster sample improved from 2.461 ms to 0.007 ms; fullscreen rendering uses geometry directly, not this raster adapter. Terrain composition per call measured 6.705 ms versus 5.914 ms previously because neighbour shading adds work; the pan optimization reduces how often it runs rather than claiming a faster individual compose.

Regression tests cover fractional/wide/negative-coordinate outlines, cold saved-region slopes across both axes, overscan density, unchanged publication suppression, terrain invalidation, reopening refresh and wide-view detailed-cache avoidance. Actual game frame-time/visual acceptance remains separate from these CPU/native checks.

### Final fix verification

- `cargo fmt --all --check` and `rustfmt --edition 2024 --check mixins/draimhorizon/*.rs`: pass.
- `cargo clippy --all-targets --locked -- -D warnings`: pass.
- `cargo test --locked`: **219 pass**.
- Official pinned `tools/stage.py` and real injected `horizon-client` `cargo check --no-default-features`: pass. Native compilation reports 15 dead-code warnings; it is not a linked Performance-profile game build.
- `cargo horizon-mod check target/draimhorizons-0.22.6.hnmod --json`: `ok: true`. Source-only artifact: 364938 bytes, SHA-256 `0d6f84dc26e3b2031ba02ff3399658de290da30ed8eb095a72f1a0dc5f0a36bb`.
- All 115 packaged source entries match the working tree. Final stage-plan module sources match the changed working-tree modules.
- Latest optimized diagnostic sample: raster overlay 0.004 ms/call; fullscreen geometry construction 0.060 µs/call for one claim (GPU drawing excluded); terrain compose 6.423 ms/call. Boundary slope remains 122/122/122. These are synthetic measurements, not a live FPS gain.
- The user confirmed the updated version works well in-game. The agent did not install/restart the client or independently measure live frame times. No push or release performed. Full dirty-region GPU tiling, a mip pyramid, summary caching and scan/colour scheduling remain follow-up optimization work rather than claims of this patch.

## Reproduce the confirmed CPU rendering defects

```sh
cargo run --locked --release --example map_diagnostics
```

The harness calls the real shared `claim_map::paint_aspect` and `map_history::Store::compose` functions. All inputs are synthetic; it does not read player saves, write map files or require a client. The original harness characterized the defects; it now asserts corrected behavior, backed by desired-behavior regression tests that failed before the fixes.

Observed on the development host, optimized build, 100 calls per timing sample:

| Probe | Result |
| --- | --- |
| Outline, aspect 1, centre X offset 0 | 2 strong vertical-edge pixels; 2 horizontal-edge pixels |
| Outline, aspect 2, offset 0 | **0 vertical-edge pixels**, 2 horizontal-edge pixels |
| Outline, aspect 2, offset 0.125 blocks | **1 vertical-edge pixel**, 2 horizontal-edge pixels |
| One-claim overlay, 512² pixels | **2.461 ms/call**, game-thread operation |
| Terrain composition, 512², 256 resident chunks, fractional pan | **5.914 ms/call**, background-worker operation |
| Continuous descending slope across X=256, red channel at X=255/256/257 | **122 / 128 / 122**, artificial brighter boundary column |

Timings are one synthetic sample, not game FPS, disk benchmarks or a representative percentile distribution. Do not add worker and game-thread time together and call that a frame time.

## Current pipeline

1. `map_view.rs:220-251` clones the current snapshot and requests the full-map camera. Mouse drag changes the camera every GUI frame (`:815-834`).
2. `map_backend.rs:266-329` scans already-loaded terrain around the player, not the dragged camera. Scanning is requested every 50 ms with a native 400 µs budget; capturing and texture colouring are additional work outside that scanner budget, capped at 16 accepted chunks per pass.
3. `map_backend.rs:415-431` requests a 512² image every 100 ms while the full explored map is visible.
4. `map_worker.rs:115-245` serializes tile ingestion, disk reads/writes, height probes and composition on one worker. It coalesces pending queries and caches the last exact query plus a global tile-batch revision. A pan changes the query, so it misses that cache. Any tile batch also invalidates it, even outside the viewport.
5. `map_history.rs:398-534` constructs a camera-aligned raster, choosing precise terrain or per-chunk summaries.
6. `map_backend.rs:366-408` takes the result, paints claims/grid on the **game thread**, then submits a complete 512² RGBA atlas patch: 1,048,576 bytes per accepted image, approximately 10 MiB/s of patch payload at 10 Hz. This payload count alone does not establish a GPU bottleneck.
7. `map_view.rs:854-886` already geographically reprojects the previous texture every frame. Icons and labels use the current camera separately. Thus map dragging is not simply capped at 10 FPS, but new raster samples replace the reprojected image at roughly 10 Hz and can shimmer or pop. The previous image covers exactly its old view, without a prefetch margin, so newly exposed edges can show background until a result arrives.

## Confirmed visual defects

### Claim border disappears or changes sides during panning

`claim_map.rs:571-594` tests each pixel centre against the nearest chunk edge. The same world-distance threshold is used for all sides. Horizontal world sampling is multiplied by viewport aspect (`View::world_aspect`, `:421-425`), but border coverage is not. A thin vertical border can therefore fit entirely between sample centres. Fractional pan changes which sides get a sample; the harness reproduces absent sides even on a square viewport at some offsets.

This matches the class of incomplete green outlines in the supplied images. The specific screenshot viewport/camera settings were not available, so the harness is not a pixel-for-pixel screenshot reproduction.

**Preferred fix:** draw claim fills and borders as clipped geometry using the same camera as icons, with thickness defined in screen pixels, independent of the terrain raster. Cull to visible chunks and batch by colour. Preserve own/friend/foreign priority, dimension filters and configurable fill/outline. Decide explicitly whether adjacent claims keep internal boundaries; this investigation does not silently change that behavior.

**Smaller fallback:** rasterize projected claim rectangles with conservative pixel coverage and per-axis scale; do not merely multiply the existing threshold by aspect, which leaves sampling-phase problems on the other axis.

### Terrain shading has region-boundary stripes

`map_history.rs:563-572` substitutes the current height for the west/north neighbour at region boundaries. A uniform slope loses one shading contribution at X or Z divisible by 256. The harness proves a brighter vertical column even though both neighbouring regions are resident and complete.

**Fix:** provide a read-only one-column halo from neighbouring regions, or cache world-aligned shaded tiles with neighbour-aware invalidation. Avoid per-pixel disk lookups or increasing LRU churn to fetch neighbours. Use current-height fallback only when the neighbour is genuinely unknown.

This proves one stripe source, not that every line observed during dragging has this cause. Additional candidates needing in-game isolation: nearest-neighbour resampling during image replacement, optional grid, mixed legacy palette/textured chunks, and newly exposed stale-image edges. The inspected pinned HUD `quad`/`rect_uv` preserves float positions; there is no evidence here for blaming integer snapping there.

## Performance findings

### Highest-impact structural issue: camera movement rebuilds terrain

Unlike a world-aligned tile cache, every fractional camera change produces a different 512² raster. Painting claims then walks all 262,144 pixels and performs per-pixel coordinate conversion and claim lookup (`claim_map.rs:558-598`), even when most of the view contains no claims. The measured ~2.46 ms game-thread overlay cost is material for a high-refresh client; the actual spike depends on scene, claim count, host load and build profile.

Simply increasing the 10 Hz query rate would multiply work and is not the recommended fix.

### Precise-view working set exceeds the 24-region LRU

- `map_history.rs:226-234` caps decoded regions at 24 across all contexts.
- `:409-410,441` uses precise data while `step < 8`, equivalent to logical `bpp < 32` for 512² output.
- At logical bpp=16 and aspect=1, a 2048×2048-block aligned explored view covers 8×8 = **64** regions. Fractional alignment can add another row/column, and wide viewports require more regions.
- Each composition visits all intersecting saved regions. A fully explored working set larger than the cache can repeatedly evict and decode itself on successive changing queries.
- A dirty eviction calls `save()` before loading the next region; `atomic()` (`:640-646`) performs `sync_all()` and rename. This stalls the map worker, not directly the game thread.
- At coarser zoom, summaries outside the decoded cache are reopened/read on each compose (`:452-469`); there is no dedicated summary cache.

This is established from the code, not a measured disk-I/O trace. Avoid solving it solely by making the detailed cache unbounded. Introduce LOD and a byte-budgeted visible/prefetch working set.

### Secondary sources of spikes

- The 16-chunk capture limit is checked **after** fingerprinting (`map_backend.rs:299-308`). When a backlog prevents advancing `captured_at`, later passes collect and hash that backlog again. Cache scan-generation/fingerprint state per tile and bound total capture time, not just accepted count.
- Texture colour extraction is on the game thread (`map_backend.rs:309-315`); up to 4096 columns per pass are outside the native scanner budget. Minimap uses a separate live scanner/colour path. Share immutable results where their layer and generation match; never access mutable game chunks from an arbitrary worker.
- `map_view.rs:227` clones cells, marks and mobs with each request. Share immutable snapshots and version them instead of rebuilding/cloning stable collections every frame.
- The worker clones full buffers for its exact-query cache (`map_worker.rs:214,220`). More importantly, one worker handles persistence and presentation: slow flushes delay new map images.

Do not change on-disk checksums or remove `sync_all()` as an initial optimization. Both affect compatibility/durability and require a separate design. Preserve existing validation, identity isolation, failed-write retention and shutdown semantics.

## Java source comparisons

These are source-inspected patterns, not measured Java-vs-Rust performance comparisons. Use the architecture, not their OpenGL calls or copied implementation. The old VoxelMap repository is historical, not evidence for modern VoxelMap. JourneyMap implementation was not publicly inspectable in the checked repository, so no internals are attributed to it.

### FTB Chunks — pinned `83bac199cc462c8e0cce8f8af9049c451fd958d9`

- [Region texture identity, worker baking, main-thread upload](https://github.com/FTBTeam/FTB-Chunks/blob/83bac199cc462c8e0cce8f8af9049c451fd958d9/common/src/main/java/dev/ftb/mods/ftbchunks/client/map/MapRegionTexture.java#L33-L89).
- [Draw a ready region texture as a quad](https://github.com/FTBTeam/FTB-Chunks/blob/83bac199cc462c8e0cce8f8af9049c451fd958d9/common/src/main/java/dev/ftb/mods/ftbchunks/client/gui/map/MapTileWidget.java#L24-L38).
- [Dragging changes scroll positions](https://github.com/FTBTeam/FTB-Chunks/blob/83bac199cc462c8e0cce8f8af9049c451fd958d9/common/src/main/java/dev/ftb/mods/ftbchunks/client/gui/map/LargeMapScreen.java#L246-L259), not a camera-sized terrain raster.

FTB uses whole-region rebakes/uploads, not dirty subrect uploads. Its key advantage here is stable cached regions independent of camera movement. It bakes claim data into regions; a separate geometry overlay is preferable here because it avoids rebaking terrain for claim updates.

### MapWriter — pinned `15cab7a02f6cd55e02f47eea10a1de85ffdba44f`

- [Dirty texture rectangles and toroidal region slots](https://github.com/mapwriter/mapwriter/blob/15cab7a02f6cd55e02f47eea10a1de85ffdba44f/src/main/java/mapwriter/map/MapTexture.java#L97-L159).
- [Partial GPU upload via glTexSubImage2D](https://github.com/mapwriter/mapwriter/blob/15cab7a02f6cd55e02f47eea10a1de85ffdba44f/src/main/java/mapwriter/Texture.java#L124-L130).
- [Incrementally propagate changed areas to coarser zoom regions](https://github.com/mapwriter/mapwriter/blob/15cab7a02f6cd55e02f47eea10a1de85ffdba44f/src/main/java/mapwriter/region/Region.java#L125-L157).
- [Draw terrain with UV coordinates, then a separate overlay](https://github.com/mapwriter/mapwriter/blob/15cab7a02f6cd55e02f47eea10a1de85ffdba44f/src/main/java/mapwriter/map/MapRenderer.java#L114-L130).

This is the closest reference for dirty-chunk uploads, LOD and overlays. Horizon uses wgpu/native atlas patches, not OpenGL: first verify the pinned atlas allocation, patch lifetime and sampler contracts. A fixed atlas slot pool or toroidal surface may fit the existing pipeline better than a new texture-binding pipeline.

### Historical VoxelMap/ZanMinimap — pinned `32fe46cbdd120598e712c0f00e108e3b97ee0377`

[Shift existing pixel rows/columns with arraycopy](https://github.com/mamiyaotaru/voxelmap/blob/32fe46cbdd120598e712c0f00e108e3b97ee0377/mamiyaotaru/GLBufferedImage.java#L99-L115). This provides a smaller intermediate approach: keep a world-aligned backing raster and repaint exposed strips, rather than recomposing all pixels. Modern region tiles plus LOD are the more scalable target.

## Recommended implementation order

1. **Correctness first:** add desired-behavior regression tests for fractional pan, rectangular viewports, negative coordinates, all four claim sides, zoom extremes and region-crossing slopes. Replace per-pixel claims with clipped overlay geometry; fix neighbour shading.
2. **Measure real interaction:** add opt-in bounded timing/counters for scan/capture/colour, compose, cache hits/misses, region reads, flush latency, upload bytes and output age. Compare stationary, slow drag, rapid drag and zoom in an isolated bot-screen client with the same map data. Record frame-time p50/p95/p99, not just FPS.
3. **Decouple camera from content:** world-aligned cached terrain tiles, visible + prefetch coverage, dirty revisions per tile/region, bounded atlas slots. Pan/zoom changes vertices/UVs; upload only new or dirty tile rectangles. Keep the last valid content while work is pending.
4. **Bound zoomed-out work:** incremental mip/LOD levels, summary cache and byte-budgeted LRU. Maintain exploration coverage for narrow paths; average only known samples. No full-resolution region decode when a coarser level suffices.
5. **Bound background latency:** split or prioritize presentation versus persistence, immutable snapshots and per-context generations. Preserve dirty-again updates during an in-flight bake and discard results from old world/layer/resource generations. Bound queues, memory, upload bytes and work per frame.

Acceptance requires no holes or changing claim edges during actual mouse dragging, no shading seam on continuous known terrain, preserved filters and both UI languages, bounded memory at large zoom, and no loss of exploration on reconnect/restart/save failure. Compile checks and synthetic diagnostics cannot establish visual acceptance.

## Original investigation verification (before fixes)

- `cargo run --locked --release --example map_diagnostics`: passes; defects and timings above reproduced.
- `cargo fmt --all --check`: passes.
- `cargo clippy --all-targets --locked -- -D warnings`: passes.
- `cargo test --locked`: **213 tests pass**; existing tests did not cover the reproduced edge cases.
- Pinned original Horizon HUD source hash matches `compatibility.json`.
- No runtime edits; no new native artifact, native client build, installation, live-game profiling or interactive visual acceptance attempted.
