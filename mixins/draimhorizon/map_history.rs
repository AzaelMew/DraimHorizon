// Bounded in-memory region cache and atomic, versioned explored terrain storage.
use std::{
    collections::{BTreeMap, BTreeSet},
    fs,
    io::{Read, Write},
    path::{Path, PathBuf},
    time::{Duration, Instant},
};
const SIDE: i32 = 16;
const REGION: i32 = SIDE * 16;
const MAX_BYTES: u64 = 550_000;
// Zoomed-out overview texels per region side at the finest level (2 blocks each).
const THUMB: usize = 128;
const THUMB_LEVELS: usize = 3;
// In-memory overview budget; the finest levels are dropped first.
const THUMB_BYTES: usize = 32 << 20;
// Disk/build time one compose may spend on missing overviews before showing
// chunk summaries and asking the worker to refine on the next query.
const THUMB_BUILD: Duration = Duration::from_millis(40);
const MAGIC: &[u8; 8] = b"DHMAP02\0";
const LEGACY_MAGIC: &[u8; 8] = b"DHMAP01\0";
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct Context {
    pub scope: String,
    pub dimension: String,
    // i32::MAX = surface; cave slices are kept apart in 16-block bands.
    pub layer: i32,
    pub persistent: bool,
}
impl Context {
    pub fn id(&self) -> String {
        format!(
            "{:016x}-{}",
            hash(self.identity().as_bytes()),
            u8::from(self.persistent)
        )
    }
    fn identity(&self) -> String {
        serde_json::json!([self.scope, self.dimension, self.layer]).to_string()
    }
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Tile {
    pub color: [u8; 256],
    pub height: [i16; 256],
    pub depth: [u8; 256],
    // ARGB top-texture colour with biome tint; zero means a legacy palette-only column.
    pub rgb: [u32; 256],
}
impl Tile {
    pub fn fingerprint(&self) -> u64 {
        self.color
            .iter()
            .copied()
            .chain(self.height.iter().flat_map(|h| h.to_le_bytes()))
            .chain(self.depth.iter().copied())
            .chain(self.rgb.iter().flat_map(|c| c.to_le_bytes()))
            .fold(0xcbf29ce484222325, |h, b| {
                (h ^ u64::from(b)).wrapping_mul(0x100000001b3)
            })
    }
    fn bytes(&self) -> Vec<u8> {
        let mut out = Vec::with_capacity(2048);
        out.extend(self.color);
        for h in self.height {
            out.extend(h.to_le_bytes());
        }
        out.extend(self.depth);
        for color in self.rgb {
            out.extend(color.to_le_bytes());
        }
        out
    }
    fn decode(data: &[u8]) -> Self {
        let mut t = Self {
            color: [0; 256],
            height: [0; 256],
            depth: [0; 256],
            rgb: [0; 256],
        };
        t.color.copy_from_slice(&data[..256]);
        for (i, h) in t.height.iter_mut().enumerate() {
            *h = i16::from_le_bytes([data[256 + i * 2], data[257 + i * 2]]);
        }
        t.depth.copy_from_slice(&data[768..1024]);
        if data.len() == 2048 {
            for (i, c) in t.rgb.iter_mut().enumerate() {
                *c = u32::from_le_bytes(data[1024 + i * 4..1028 + i * 4].try_into().unwrap());
            }
        }
        t
    }
}
pub fn hash(bytes: &[u8]) -> u64 {
    bytes.iter().fold(0xcbf29ce484222325, |h, b| {
        (h ^ u64::from(*b)).wrapping_mul(0x100000001b3)
    })
}
#[derive(Default)]
struct Region {
    tiles: BTreeMap<u16, Tile>,
    summary: Vec<u8>,
    dirty: bool,
    used: u64,
}
impl Region {
    fn encode(&self, at: (i32, i32)) -> Vec<u8> {
        let mut out = Vec::new();
        out.extend(MAGIC);
        out.extend(at.0.to_le_bytes());
        out.extend(at.1.to_le_bytes());
        out.extend(&self.summary);
        out.extend(hash(&self.summary).to_le_bytes());
        out.extend((self.tiles.len() as u16).to_le_bytes());
        for (id, t) in &self.tiles {
            out.extend(id.to_le_bytes());
            out.extend(t.bytes());
        }
        let checksum = hash(&out);
        out.extend(checksum.to_le_bytes());
        out
    }
    fn decode(bytes: &[u8], at: (i32, i32)) -> Result<Self, String> {
        if bytes.len() < 1058
            || bytes.len() as u64 > MAX_BYTES
            || (&bytes[..8] != MAGIC && &bytes[..8] != LEGACY_MAGIC)
        {
            return Err("invalid map region header".into());
        }
        let entry_size = if &bytes[..8] == LEGACY_MAGIC {
            1026
        } else {
            2050
        };
        let rx = i32::from_le_bytes(bytes[8..12].try_into().unwrap());
        let rz = i32::from_le_bytes(bytes[12..16].try_into().unwrap());
        let count = u16::from_le_bytes(bytes[1048..1050].try_into().unwrap()) as usize;
        if (rx, rz) != at
            || count > 256
            || bytes.len() != 1058 + count * entry_size
            || hash(&bytes[..bytes.len() - 8])
                != u64::from_le_bytes(bytes[bytes.len() - 8..].try_into().unwrap())
        {
            return Err("corrupt map region".into());
        }
        let mut region = Self {
            summary: bytes[16..1040].to_vec(),
            ..Self::default()
        };
        for entry in bytes[1050..bytes.len() - 8].chunks_exact(entry_size) {
            let id = u16::from_le_bytes([entry[0], entry[1]]);
            if id >= 256 || region.tiles.insert(id, Tile::decode(&entry[2..])).is_some() {
                return Err("duplicate map chunk".into());
            }
        }
        Ok(region)
    }
    fn column(&self, x: i32, z: i32) -> Option<(u8, i16, u8, u32)> {
        let id =
            ((z.div_euclid(16).rem_euclid(SIDE)) * SIDE + x.div_euclid(16).rem_euclid(SIDE)) as u16;
        let t = self.tiles.get(&id)?;
        let i = (z.rem_euclid(16) * 16 + x.rem_euclid(16)) as usize;
        Some((t.color[i], t.height[i], t.depth[i], t.rgb[i]))
    }
}
struct Catalog {
    context: Context,
    dir: PathBuf,
    regions: BTreeSet<(i32, i32)>,
}
// Shaded RGBA overview of one region at 2, 4 and 8 blocks per texel.
struct Thumb {
    levels: [Option<Vec<u8>>; THUMB_LEVELS],
    used: u64,
}
impl Thumb {
    fn bytes(&self) -> usize {
        self.levels.iter().flatten().map(Vec::len).sum()
    }
}
type ThumbKey = (String, (i32, i32), bool);
pub struct Store {
    root: PathBuf,
    catalogs: BTreeMap<String, Catalog>,
    cache: BTreeMap<(String, (i32, i32)), Region>,
    thumbs: BTreeMap<ThumbKey, Thumb>,
    thumb_clock: u64,
    palette: Vec<u32>,
    clock: u64,
    pub warning: Option<String>,
    // The last compose showed placeholders while overviews are still being built.
    pub pending: bool,
}
impl Store {
    pub fn new(root: PathBuf, palette: Vec<u32>) -> Self {
        Self {
            root,
            catalogs: BTreeMap::new(),
            cache: BTreeMap::new(),
            thumbs: BTreeMap::new(),
            thumb_clock: 0,
            palette,
            clock: 0,
            warning: None,
            pending: false,
        }
    }
    fn catalog(&mut self, c: &Context) -> Result<String, String> {
        let id = c.id();
        if self.catalogs.contains_key(&id) {
            return Ok(id);
        }
        let dir = self
            .root
            .join(format!("{:016x}", hash(c.identity().as_bytes())));
        let mut regions = BTreeSet::new();
        if c.persistent && dir.exists() {
            let metadata = dir.join("context.json");
            if metadata.exists() && read_identity(&metadata)? != c.identity() {
                return Err("map identity mismatch".into());
            }
            for entry in fs::read_dir(&dir).map_err(|e| e.to_string())? {
                let entry = entry.map_err(|e| e.to_string())?;
                let name = entry.file_name();
                let name = name.to_string_lossy();
                if let Some((x, z)) = name.strip_suffix(".region").and_then(|s| s.split_once('_'))
                    && let (Ok(x), Ok(z)) = (x.parse::<i32>(), z.parse::<i32>())
                {
                    regions.insert((x, z));
                }
            }
            if !metadata.exists() && !regions.is_empty() {
                return Err("map identity is missing".into());
            }
        }
        self.catalogs.insert(
            id.clone(),
            Catalog {
                context: c.clone(),
                dir,
                regions,
            },
        );
        Ok(id)
    }
    fn load(&mut self, id: &str, at: (i32, i32)) -> Result<(), String> {
        let key = (id.to_owned(), at);
        self.clock = self.clock.wrapping_add(1);
        if let Some(r) = self.cache.get_mut(&key) {
            r.used = self.clock;
            return Ok(());
        }
        while self.cache.len() >= 24 {
            let oldest = self
                .cache
                .iter()
                .min_by_key(|(_, r)| r.used)
                .map(|(k, _)| k.clone())
                .unwrap();
            self.save(&oldest)?;
            self.cache.remove(&oldest);
        }
        let catalog = &self.catalogs[id];
        let file = catalog.dir.join(format!("{}_{}.region", at.0, at.1));
        let region = if catalog.context.persistent && file.exists() {
            let bytes = read_limited(&file)?;
            Region::decode(&bytes, at)?
        } else {
            Region {
                summary: vec![0; 1024],
                ..Region::default()
            }
        };
        self.cache.insert(
            key,
            Region {
                used: self.clock,
                ..region
            },
        );
        Ok(())
    }
    pub fn insert(&mut self, c: &Context, chunk: (i32, i32), tile: Tile) -> Result<(), String> {
        let id = self.catalog(c)?;
        let at = (chunk.0.div_euclid(SIDE), chunk.1.div_euclid(SIDE));
        self.load(&id, at)?;
        let local = (chunk.1.rem_euclid(SIDE) * SIDE + chunk.0.rem_euclid(SIDE)) as u16;
        let r = self.cache.get_mut(&(id.clone(), at)).unwrap();
        if r.tiles.get(&local) == Some(&tile) {
            return Ok(());
        }
        let mut rgb = [0u32; 3];
        let mut n = 0;
        for (i, color) in tile.color.iter().copied().enumerate() {
            if let Some(&col) = self.palette.get(usize::from(color & 0x7f))
                && color & 0x7f != 0
            {
                let col = if tile.rgb[i] != 0 { tile.rgb[i] } else { col };
                for (i, shift) in [16, 8, 0].iter().enumerate() {
                    rgb[i] += col >> shift & 255;
                }
                n += 1;
            }
        }
        let i = usize::from(local) * 4;
        r.summary[i..i + 4].fill(0);
        if n > 0 {
            for (ch, value) in rgb.iter().enumerate() {
                r.summary[i + ch] = value.checked_div(n).unwrap_or(0) as u8;
            }
            r.summary[i + 3] = 255;
        }
        r.tiles.insert(local, tile);
        r.dirty = c.persistent;
        self.catalogs.get_mut(&id).unwrap().regions.insert(at);
        self.refresh_thumbs(&id, at, local);
        Ok(())
    }
    // Repaint the changed chunk, and the chunks whose relief shading reads it,
    // in already built overviews; overviews missing their finest level are rebuilt later.
    fn refresh_thumbs(&mut self, id: &str, at: (i32, i32), local: u16) {
        let region = &self.cache[&(id.to_owned(), at)];
        let column = |x: i32, z: i32| {
            ((x.div_euclid(REGION), z.div_euclid(REGION)) == at)
                .then(|| region.column(x, z))
                .flatten()
        };
        let (cx, cz) = (usize::from(local) % 16, usize::from(local) / 16);
        let side = THUMB / 16;
        for shading in [false, true] {
            let key = (id.to_owned(), at, shading);
            let Some(thumb) = self.thumbs.get_mut(&key) else {
                continue;
            };
            let Some(fine) = thumb.levels[0].as_mut() else {
                self.thumbs.remove(&key);
                continue;
            };
            let (x1, y1) = ((cx + 2).min(16) * side, (cz + 2).min(16) * side);
            for ty in cz * side..y1 {
                for tx in cx * side..x1 {
                    if tx >= (cx + 1) * side && ty >= (cz + 1) * side {
                        continue;
                    }
                    let i = (ty * THUMB + tx) * 4;
                    fine[i..i + 4].copy_from_slice(&thumb_texel(
                        &column,
                        at,
                        tx,
                        ty,
                        &self.palette,
                        shading,
                    ));
                }
            }
            for level in 1..THUMB_LEVELS {
                let [coarse, finer] = [level, level - 1].map(|l| thumb.levels[l].take());
                if let (Some(mut coarse), Some(finer)) = (coarse, finer) {
                    let (from, to) = (THUMB >> (level - 1), THUMB >> level);
                    let scale = side >> level;
                    downsample(
                        &finer,
                        from,
                        &mut coarse,
                        to,
                        [cx * scale, cz * scale],
                        [x1 >> level, y1 >> level],
                    );
                    thumb.levels[level] = Some(coarse);
                    thumb.levels[level - 1] = Some(finer);
                } else {
                    self.thumbs.remove(&key);
                    break;
                }
            }
        }
    }
    // Make an overview of `level` (or failing that a coarser one) available for compose.
    fn ensure_thumb(
        &mut self,
        id: &str,
        at: (i32, i32),
        level: usize,
        shading: bool,
        persistent: bool,
        started: Instant,
    ) -> Option<usize> {
        let key = (id.to_owned(), at, shading);
        let have = self
            .thumbs
            .get(&key)
            .and_then(|t| (level..THUMB_LEVELS).find(|&l| t.levels[l].is_some()));
        if have != Some(level) {
            if started.elapsed() >= THUMB_BUILD {
                self.pending = true;
            } else {
                let built = if let Some(region) = self.cache.get(&(id.to_owned(), at)) {
                    Some(Ok(build_thumb(region, at, &self.palette, shading)))
                } else if persistent {
                    let file = self.catalogs[id]
                        .dir
                        .join(format!("{}_{}.region", at.0, at.1));
                    Some(
                        read_limited(&file)
                            .and_then(|bytes| Region::decode(&bytes, at))
                            .map(|region| build_thumb(&region, at, &self.palette, shading)),
                    )
                } else {
                    None
                };
                match built {
                    Some(Ok(levels)) => {
                        self.thumbs.insert(
                            key.clone(),
                            Thumb {
                                levels: levels.map(Some),
                                used: 0,
                            },
                        );
                    }
                    Some(Err(e)) => self.warning = Some(e),
                    None => {}
                }
            }
        }
        let thumb = self.thumbs.get_mut(&key)?;
        thumb.used = self.thumb_clock;
        (level..THUMB_LEVELS).find(|&l| thumb.levels[l].is_some())
    }
    // Keep overviews within THUMB_BYTES without dropping ones used by the current compose.
    fn trim_thumbs(&mut self) {
        let mut total: usize = self.thumbs.values().map(Thumb::bytes).sum();
        for level in 0..THUMB_LEVELS {
            if total <= THUMB_BYTES {
                return;
            }
            let mut order: Vec<_> = self
                .thumbs
                .iter()
                .filter(|(_, t)| t.used != self.thumb_clock && t.levels[level].is_some())
                .map(|(k, t)| (t.used, k.clone()))
                .collect();
            order.sort();
            for (_, key) in order {
                if total <= THUMB_BYTES {
                    return;
                }
                if level + 1 == THUMB_LEVELS {
                    total -= self.thumbs.remove(&key).map_or(0, |t| t.bytes());
                } else if let Some(t) = self.thumbs.get_mut(&key) {
                    total -= t.levels[level].take().map_or(0, |l| l.len());
                }
            }
        }
    }
    fn save(&mut self, key: &(String, (i32, i32))) -> Result<(), String> {
        let r = &self.cache[key];
        if !r.dirty {
            return Ok(());
        }
        let cat = &self.catalogs[&key.0];
        fs::create_dir_all(&cat.dir).map_err(|e| e.to_string())?;
        let meta = cat.dir.join("context.json");
        if meta.exists() {
            if read_identity(&meta)? != cat.context.identity() {
                return Err("map identity mismatch".into());
            }
        } else {
            atomic(&meta, cat.context.identity().as_bytes())?;
        }
        atomic(
            &cat.dir.join(format!("{}_{}.region", key.1.0, key.1.1)),
            &r.encode(key.1),
        )?;
        self.cache.get_mut(key).unwrap().dirty = false;
        Ok(())
    }
    pub fn flush(&mut self) -> Result<(), String> {
        let keys: Vec<_> = self.cache.keys().cloned().collect();
        for key in keys {
            self.save(&key)?;
        }
        Ok(())
    }
    pub fn dimensions(&self, scope: &str) -> Vec<String> {
        let mut dims: BTreeSet<String> = [
            "minecraft:overworld",
            "minecraft:the_nether",
            "minecraft:the_end",
        ]
        .into_iter()
        .map(str::to_owned)
        .collect();
        dims.extend(
            self.catalogs
                .values()
                .filter(|c| c.context.scope == scope)
                .map(|c| c.context.dimension.clone()),
        );
        // Read only small identity files, not terrain, to discover saved custom dimensions.
        if let Ok(entries) = fs::read_dir(&self.root) {
            for entry in entries.take(4096).flatten() {
                let file = entry.path().join("context.json");
                if file.metadata().is_ok_and(|m| m.len() < 4096)
                    && let Ok(raw) = fs::read_to_string(file)
                    && let Ok(v) = serde_json::from_str::<serde_json::Value>(&raw)
                    && v[0].as_str() == Some(scope)
                    && let Some(d) = v[1].as_str()
                {
                    dims.insert(d.to_owned());
                }
            }
        }
        dims.into_iter().collect()
    }
    pub fn saved_context(&mut self, c: &Context) -> Context {
        if !c.persistent || c.layer == i32::MAX {
            return c.clone();
        }
        if let Ok(id) = self.catalog(c)
            && !self.catalogs[&id].regions.is_empty()
        {
            return c.clone();
        }
        if let Some(found) = self.catalogs.values().find(|v| {
            v.context.scope == c.scope
                && v.context.dimension == c.dimension
                && v.context.persistent
                && v.context.layer != i32::MAX
                && !v.regions.is_empty()
        }) {
            return found.context.clone();
        }
        if let Ok(entries) = fs::read_dir(&self.root) {
            for entry in entries.take(4096).flatten() {
                let meta = entry.path().join("context.json");
                if meta.metadata().is_ok_and(|m| m.len() < 4096)
                    && let Ok(raw) = fs::read_to_string(meta)
                    && let Ok(v) = serde_json::from_str::<serde_json::Value>(&raw)
                    && v[0].as_str() == Some(c.scope.as_str())
                    && v[1].as_str() == Some(c.dimension.as_str())
                    && let Some(layer) = v[2].as_i64().and_then(|n| i32::try_from(n).ok())
                    && layer != i32::MAX
                {
                    return Context { layer, ..c.clone() };
                }
            }
        }
        c.clone()
    }
    pub fn height_at(&mut self, c: &Context, x: i32, z: i32) -> Result<Option<i16>, String> {
        let id = self.catalog(c)?;
        let at = (x.div_euclid(REGION), z.div_euclid(REGION));
        if !self.catalogs[&id].regions.contains(&at) {
            return Ok(None);
        }
        self.load(&id, at)?;
        Ok(self.cache[&(id, at)]
            .column(x, z)
            .filter(|c| c.0 & 0x7f != 0)
            .map(|c| c.1))
    }
    pub fn compose(&mut self, c: &Context, spec: &Render) -> Result<Vec<u8>, String> {
        let id = self.catalog(c)?;
        let size = spec.size.clamp(64, 768);
        let mut image = vec![0; size * size * 4];
        for y in 0..size {
            for x in 0..size {
                let i = (y * size + x) * 4;
                let shade = if (x / 16 + y / 16) % 2 == 0 { 20 } else { 23 };
                image[i..i + 4].copy_from_slice(&[shade, shade + 5, shade + 10, 255]);
            }
        }
        let span = 128.0 * spec.bpp;
        let step = span / size as f64;
        if !span.is_finite()
            || !(0.25..=786432.0).contains(&spec.bpp)
            || !spec.aspect.is_finite()
            || !(0.1..=32.0).contains(&spec.aspect)
            || !spec
                .center
                .iter()
                .all(|p| p.is_finite() && p.abs() <= 30_000_000.0)
        {
            return Err("invalid map view".into());
        }
        let low = [
            spec.center[0] - span * spec.aspect / 2.0,
            spec.center[1] - span / 2.0,
        ];
        let regions: Vec<_> = self.catalogs[&id]
            .regions
            .iter()
            .copied()
            .filter(|&(rx, rz)| {
                let x = f64::from(rx) * f64::from(REGION);
                let z = f64::from(rz) * f64::from(REGION);
                x + f64::from(REGION) > low[0]
                    && z + f64::from(REGION) > low[1]
                    && x < low[0] + span * spec.aspect
                    && z < low[1] + span
            })
            .collect();
        // Keep the visible detailed working set within the 24-region LRU,
        // leaving room for neighbour halos and incoming exploration.
        let precise = step < 8.0 && (regions.len() <= 18 || !c.persistent);
        // Wider views sample shaded per-region overviews of matching density;
        // only the farthest zooms fall back to one colour per chunk.
        let finest = step.min(step * spec.aspect);
        let overview = (!precise && step < 16.0).then(|| {
            (0..THUMB_LEVELS)
                .rev()
                .find(|&l| f64::from(2u32 << l) <= finest)
                .unwrap_or(0)
        });
        self.pending = false;
        self.thumb_clock = self.thumb_clock.wrapping_add(1);
        let started = Instant::now();
        let mut regions = regions;
        if overview.is_some() {
            // Build the middle of the view first when the time budget runs out.
            regions.sort_by(|a, b| {
                let distance = |r: &(i32, i32)| {
                    let x = (f64::from(r.0) + 0.5) * f64::from(REGION) - spec.center[0];
                    let z = (f64::from(r.1) + 0.5) * f64::from(REGION) - spec.center[1];
                    x * x + z * z
                };
                distance(a).total_cmp(&distance(b))
            });
        }
        for at in regions {
            let key = (id.clone(), at);
            let from = [
                (f64::from(at.0) * f64::from(REGION) - low[0]) / (step * spec.aspect),
                (f64::from(at.1) * f64::from(REGION) - low[1]) / step,
            ];
            let to = [
                from[0] + f64::from(REGION) / (step * spec.aspect),
                from[1] + f64::from(REGION) / step,
            ];
            if let Some(level) = overview
                && let Some(level) =
                    self.ensure_thumb(&id, at, level, spec.shading, c.persistent, started)
                && let Some(texels) =
                    self.thumbs[&(id.clone(), at, spec.shading)].levels[level].as_deref()
            {
                let side = THUMB >> level;
                let texel = f64::from(REGION) / side as f64;
                let origin = [at.0, at.1].map(|v| f64::from(v) * f64::from(REGION));
                for y in
                    (from[1].floor().max(0.0) as usize)..(to[1].ceil().max(0.0) as usize).min(size)
                {
                    let wz = low[1] + (y as f64 + 0.5) * step - origin[1];
                    if !(0.0..f64::from(REGION)).contains(&wz) {
                        continue;
                    }
                    let ty = ((wz / texel) as usize).min(side - 1);
                    for x in (from[0].floor().max(0.0) as usize)
                        ..(to[0].ceil().max(0.0) as usize).min(size)
                    {
                        let wx = low[0] + (x as f64 + 0.5) * step * spec.aspect - origin[0];
                        if !(0.0..f64::from(REGION)).contains(&wx) {
                            continue;
                        }
                        let tx = ((wx / texel) as usize).min(side - 1);
                        let t = (ty * side + tx) * 4;
                        if texels[t + 3] != 0 {
                            let i = (y * size + x) * 4;
                            image[i..i + 4].copy_from_slice(&texels[t..t + 4]);
                        }
                    }
                }
                continue;
            }
            // Copy only the two height borders before loading the current region.
            // Neighbour loads may evict it; never retain references across load().
            let mut west = [None; REGION as usize];
            let mut north = [None; REGION as usize];
            if precise && spec.shading {
                for (neighbour, vertical, edge) in [
                    ((at.0 - 1, at.1), true, &mut west),
                    ((at.0, at.1 - 1), false, &mut north),
                ] {
                    if !self.catalogs[&id].regions.contains(&neighbour) {
                        continue;
                    }
                    if let Err(e) = self.load(&id, neighbour) {
                        self.warning = Some(e);
                        continue;
                    }
                    let region = &self.cache[&(id.clone(), neighbour)];
                    for (i, value) in edge.iter_mut().enumerate() {
                        let (x, z) = if vertical {
                            (at.0 * REGION - 1, at.1 * REGION + i as i32)
                        } else {
                            (at.0 * REGION + i as i32, at.1 * REGION - 1)
                        };
                        *value = region.column(x, z).filter(|p| p.0 & 0x7f != 0).map(|p| p.1);
                    }
                }
            }
            let summary;
            if precise {
                if let Err(e) = self.load(&id, at) {
                    self.warning = Some(e);
                    continue;
                }
                summary = Vec::new();
            } else if let Some(r) = self.cache.get(&key) {
                summary = r.summary.clone();
            } else {
                let path = self.catalogs[&id]
                    .dir
                    .join(format!("{}_{}.region", at.0, at.1));
                if !c.persistent {
                    continue;
                }
                let loaded = (|| {
                    let mut file = fs::File::open(path).map_err(|e| e.to_string())?;
                    let mut prefix = [0; 1048];
                    file.read_exact(&mut prefix).map_err(|e| e.to_string())?;
                    if (&prefix[..8] != MAGIC && &prefix[..8] != LEGACY_MAGIC)
                        || hash(&prefix[16..1040])
                            != u64::from_le_bytes(prefix[1040..1048].try_into().unwrap())
                    {
                        return Err("invalid map overview".to_owned());
                    }
                    Ok(prefix[16..1040].to_vec())
                })();
                match loaded {
                    Ok(data) => summary = data,
                    Err(e) => {
                        self.warning = Some(e);
                        continue;
                    }
                }
            }
            if !precise {
                // Splat occupied chunks into the overview so narrow explored paths survive zoom-out.
                for cz in 0..SIDE {
                    for cx in 0..SIDE {
                        let i = (cz * SIDE + cx) as usize * 4;
                        let color = &summary[i..i + 4];
                        if color[3] == 0 {
                            continue;
                        }
                        let wx = f64::from(at.0) * f64::from(REGION) + f64::from(cx * 16);
                        let wz = f64::from(at.1) * f64::from(REGION) + f64::from(cz * 16);
                        let left = (wx - low[0]) / (step * spec.aspect);
                        let top = (wz - low[1]) / step;
                        let right = left + 16.0 / (step * spec.aspect);
                        let bottom = top + 16.0 / step;
                        for y in (top.floor().max(0.0) as usize)
                            ..(bottom.ceil().max(0.0) as usize).min(size)
                        {
                            for x in (left.floor().max(0.0) as usize)
                                ..(right.ceil().max(0.0) as usize).min(size)
                            {
                                let i = (y * size + x) * 4;
                                image[i..i + 4].copy_from_slice(color);
                            }
                        }
                    }
                }
                continue;
            }
            let r = self.cache.get(&key);
            for y in (from[1].floor().max(0.0) as usize)..(to[1].ceil().max(0.0) as usize).min(size)
            {
                for x in
                    (from[0].floor().max(0.0) as usize)..(to[0].ceil().max(0.0) as usize).min(size)
                {
                    let wx = (low[0] + (x as f64 + 0.5) * step * spec.aspect).floor() as i32;
                    let wz = (low[1] + (y as f64 + 0.5) * step).floor() as i32;
                    if (wx.div_euclid(REGION), wz.div_euclid(REGION)) != at {
                        continue;
                    }
                    let rgba = r.and_then(|r| {
                        shade_column(
                            |x, z| {
                                if x == at.0 * REGION - 1 {
                                    west[z.rem_euclid(REGION) as usize].map(|h| (1, h, 0, 0))
                                } else if z == at.1 * REGION - 1 {
                                    north[x.rem_euclid(REGION) as usize].map(|h| (1, h, 0, 0))
                                } else {
                                    r.column(x, z)
                                }
                            },
                            wx,
                            wz,
                            &self.palette,
                            spec.shading,
                        )
                    });
                    if let Some(color) = rgba {
                        let i = (y * size + x) * 4;
                        image[i..i + 4].copy_from_slice(&color);
                    }
                }
            }
        }
        self.trim_thumbs();
        Ok(image)
    }
}
#[derive(Clone, Debug, PartialEq)]
pub struct Render {
    pub center: [f64; 2],
    pub bpp: f64,
    pub aspect: f64,
    pub size: usize,
    pub shading: bool,
}
fn thumb_texel(
    column: &impl Fn(i32, i32) -> Option<(u8, i16, u8, u32)>,
    at: (i32, i32),
    tx: usize,
    ty: usize,
    palette: &[u32],
    shading: bool,
) -> [u8; 4] {
    let scale = REGION as usize / THUMB;
    let mut sum = [0u32; 3];
    let mut n = 0;
    for dz in 0..scale {
        for dx in 0..scale {
            let x = at.0 * REGION + (tx * scale + dx) as i32;
            let z = at.1 * REGION + (ty * scale + dz) as i32;
            if let Some(rgba) = shade_column(column, x, z, palette, shading) {
                for (s, c) in sum.iter_mut().zip(rgba) {
                    *s += u32::from(c);
                }
                n += 1;
            }
        }
    }
    if n == 0 {
        return [0; 4];
    }
    let [r, g, b] = sum.map(|s| (s / n) as u8);
    [r, g, b, 255]
}
// Alpha-aware 2x2 box filter of `src` into `dst` over the destination texels [from, to).
fn downsample(
    src: &[u8],
    src_side: usize,
    dst: &mut [u8],
    dst_side: usize,
    from: [usize; 2],
    to: [usize; 2],
) {
    for y in from[1]..to[1].min(dst_side) {
        for x in from[0]..to[0].min(dst_side) {
            let mut sum = [0u32; 3];
            let mut n = 0;
            for (dx, dy) in [(0, 0), (1, 0), (0, 1), (1, 1)] {
                let i = ((y * 2 + dy) * src_side + x * 2 + dx) * 4;
                if src[i + 3] != 0 {
                    for (s, c) in sum.iter_mut().zip(&src[i..i + 3]) {
                        *s += u32::from(*c);
                    }
                    n += 1;
                }
            }
            let i = (y * dst_side + x) * 4;
            dst[i..i + 4].copy_from_slice(&if n == 0 {
                [0; 4]
            } else {
                let [r, g, b] = sum.map(|s| (s / n) as u8);
                [r, g, b, 255]
            });
        }
    }
}
fn build_thumb(
    region: &Region,
    at: (i32, i32),
    palette: &[u32],
    shading: bool,
) -> [Vec<u8>; THUMB_LEVELS] {
    // Dense columns make the 65k-column relief pass cheap; neighbours outside the region read as flat.
    let side = REGION as usize;
    let mut dense = vec![None; side * side];
    for (&id, t) in &region.tiles {
        let (cx, cz) = (usize::from(id) % 16 * 16, usize::from(id) / 16 * 16);
        for i in 0..256 {
            dense[(cz + i / 16) * side + cx + i % 16] =
                Some((t.color[i], t.height[i], t.depth[i], t.rgb[i]));
        }
    }
    let column = |x: i32, z: i32| {
        let (lx, lz) = (x - at.0 * REGION, z - at.1 * REGION);
        if (0..REGION).contains(&lx) && (0..REGION).contains(&lz) {
            dense[lz as usize * side + lx as usize]
        } else {
            None
        }
    };
    let mut levels: [Vec<u8>; THUMB_LEVELS] =
        std::array::from_fn(|l| vec![0; (THUMB >> l) * (THUMB >> l) * 4]);
    for ty in 0..THUMB {
        for tx in 0..THUMB {
            let i = (ty * THUMB + tx) * 4;
            levels[0][i..i + 4]
                .copy_from_slice(&thumb_texel(&column, at, tx, ty, palette, shading));
        }
    }
    for l in 1..THUMB_LEVELS {
        let (finer, coarse) = levels.split_at_mut(l);
        let side = THUMB >> l;
        downsample(
            &finer[l - 1],
            side * 2,
            &mut coarse[0],
            side,
            [0, 0],
            [side, side],
        );
    }
    levels
}
fn shade_column(
    column: impl Fn(i32, i32) -> Option<(u8, i16, u8, u32)>,
    x: i32,
    z: i32,
    palette: &[u32],
    shading: bool,
) -> Option<[u8; 4]> {
    let (c, h, d, rgb) = column(x, z)?;
    let id = usize::from(c & 0x7f);
    if id == 0 {
        return None;
    }
    let color = if rgb != 0 { rgb } else { *palette.get(id)? };
    let mut brightness = 1.0f32;
    if shading {
        let north = column(x, z - 1)
            .filter(|p| p.0 & 0x7f != 0)
            .map_or(h, |p| p.1);
        let west = column(x - 1, z)
            .filter(|p| p.0 & 0x7f != 0)
            .map_or(h, |p| p.1);
        brightness = (1.0
            + f32::from(h.saturating_sub(north).clamp(-12, 12)) * 0.065
            + f32::from(h.saturating_sub(west).clamp(-12, 12)) * 0.045)
            .clamp(0.42, 1.2);
        if id == 12 {
            brightness *= 1.0 - (f32::from(d) / 64.0).min(0.32);
        }
        if c & 0x80 != 0 {
            brightness *= 0.65;
        }
    }
    let channel =
        |shift: u32| ((color >> shift & 255u32) as f32 * brightness).clamp(0.0, 255.0) as u8;
    Some([channel(16), channel(8), channel(0), 255])
}

pub fn compose_minimap(
    tiles: &BTreeMap<(i32, i32), Tile>,
    center: [f64; 2],
    bpp: f32,
    palette: &[u32],
    shading: bool,
) -> Vec<u8> {
    const SIZE: usize = 128;
    let column = |x: i32, z: i32| {
        let tile = tiles.get(&(x >> 4, z >> 4))?;
        let i = ((z & 15) * 16 + (x & 15)) as usize;
        Some((tile.color[i], tile.height[i], tile.depth[i], tile.rgb[i]))
    };
    let mut image = Vec::with_capacity(SIZE * SIZE * 4);
    for z in 0..SIZE {
        let wz = (center[1] + (z as f64 - 63.5) * f64::from(bpp)).floor() as i32;
        for x in 0..SIZE {
            let wx = (center[0] + (x as f64 - 63.5) * f64::from(bpp)).floor() as i32;
            let shade = if (x / 16 + z / 16) % 2 == 0 { 20 } else { 23 };
            image.extend(shade_column(column, wx, wz, palette, shading).unwrap_or([
                shade,
                shade + 5,
                shade + 10,
                255,
            ]));
        }
    }
    image
}
fn read_identity(path: &Path) -> Result<String, String> {
    let file = fs::File::open(path).map_err(|e| e.to_string())?;
    let mut data = String::new();
    file.take(16385)
        .read_to_string(&mut data)
        .map_err(|e| e.to_string())?;
    if data.len() > 16384 {
        return Err("map identity is too large".into());
    }
    Ok(data)
}
fn read_limited(path: &Path) -> Result<Vec<u8>, String> {
    let file = fs::File::open(path).map_err(|e| e.to_string())?;
    if file.metadata().map_err(|e| e.to_string())?.len() > MAX_BYTES {
        return Err("map region too large".into());
    }
    let mut data = Vec::new();
    file.take(MAX_BYTES + 1)
        .read_to_end(&mut data)
        .map_err(|e| e.to_string())?;
    Ok(data)
}
fn atomic(path: &Path, bytes: &[u8]) -> Result<(), String> {
    let temp = path.with_extension("pending");
    let mut file = fs::File::create(&temp).map_err(|e| e.to_string())?;
    file.write_all(bytes)
        .and_then(|_| file.sync_all())
        .map_err(|e| e.to_string())?;
    fs::rename(temp, path).map_err(|e| e.to_string())
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn live_minimap_uses_the_passed_slice_and_its_texture_colors() {
        let center = 4 * (64 * 128 + 64);
        for (height, rgb, expected) in [
            (65, 0xff881100, [136, 17, 0, 255]),
            (66, 0xff115500, [17, 85, 0, 255]),
            (75, 0xff001188, [0, 17, 136, 255]),
        ] {
            let tile = Tile {
                color: [1; 256],
                height: [height; 256],
                depth: [0; 256],
                rgb: [rgb; 256],
            };
            let image = compose_minimap(
                &BTreeMap::from([((0, 0), tile)]),
                [8.0, 8.0],
                1.0,
                &[],
                false,
            );
            assert_eq!(image[center..center + 4], expected);
        }
    }
    use std::sync::atomic::{AtomicU64, Ordering};
    struct Dir(PathBuf);
    impl Dir {
        fn new() -> Self {
            static NEXT: AtomicU64 = AtomicU64::new(0);
            Self(std::env::temp_dir().join(format!(
                "horizon-atlas-{}-{}",
                std::process::id(),
                NEXT.fetch_add(1, Ordering::Relaxed)
            )))
        }
    }
    impl Drop for Dir {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }
    fn context() -> Context {
        Context {
            scope: "example:25565#account".into(),
            dimension: "minecraft:overworld".into(),
            layer: i32::MAX,
            persistent: true,
        }
    }
    fn tile(c: u8, h: i16) -> Tile {
        Tile {
            color: [c; 256],
            height: [h; 256],
            depth: [0; 256],
            rgb: [0; 256],
        }
    }
    #[test]
    fn texture_colours_survive_disk_roundtrip_and_override_the_map_palette() {
        let dir = Dir::new();
        let c = context();
        let mut t = tile(1, 64);
        t.rgb = [0xff345678; 256];
        let mut changed = t.clone();
        changed.rgb[0] = 0xffabcdef;
        assert_ne!(t.fingerprint(), changed.fingerprint());
        let mut store = Store::new(dir.0.clone(), palette());
        store.insert(&c, (0, 0), t).unwrap();
        store.flush().unwrap();
        drop(store);
        let mut restored = Store::new(dir.0.clone(), palette());
        assert_eq!(
            center_pixel(&restored.compose(&c, &view([8.0, 8.0])).unwrap(), 128),
            [0x34, 0x56, 0x78, 255]
        );
    }
    #[test]
    fn reads_v1_palette_regions_and_writes_v2_without_losing_chunks() {
        let mut region = Region {
            summary: vec![0; 1024],
            ..Region::default()
        };
        region.tiles.insert(0, tile(1, 64));
        let mut old = region.encode((-1, 2))[..1050].to_vec();
        old[..8].copy_from_slice(LEGACY_MAGIC);
        old.extend(0u16.to_le_bytes());
        old.extend(&tile(1, 64).bytes()[..1024]);
        let sum = hash(&old);
        old.extend(sum.to_le_bytes());
        let loaded = Region::decode(&old, (-1, 2)).unwrap();
        assert_eq!(loaded.tiles.len(), 1);
        assert_eq!(loaded.tiles[&0].rgb, [0; 256]);
        let encoded = loaded.encode((-1, 2));
        assert_eq!(&encoded[..8], MAGIC);
        assert!(Region::decode(&encoded, (-1, 2)).is_ok());
        assert!(Region::decode(&old, (0, 2)).is_err());
    }
    fn palette() -> Vec<u32> {
        vec![0, 0x669944, 0x735533]
    }
    fn view(center: [f64; 2]) -> Render {
        Render {
            center,
            bpp: 0.25,
            aspect: 2.0,
            size: 128,
            shading: true,
        }
    }
    fn center_pixel(image: &[u8], size: usize) -> [u8; 4] {
        image[((size / 2) * size + size / 2) * 4..((size / 2) * size + size / 2) * 4 + 4]
            .try_into()
            .unwrap()
    }
    #[test]
    fn shading_is_continuous_across_saved_region_boundaries() {
        for axis in 0..2 {
            for boundary in [-256, 0, 256] {
                let dir = Dir::new();
                let c = context();
                let mut store = Store::new(dir.0.clone(), vec![0, 0x808080]);
                for chunk in (boundary / 16 - 1)..=(boundary / 16) {
                    let mut t = tile(1, 0);
                    for z in 0..16 {
                        for x in 0..16 {
                            let along = chunk * 16 + if axis == 0 { x } else { z };
                            t.height[(z * 16 + x) as usize] = (1000 - along) as i16;
                        }
                    }
                    let at = if axis == 0 { (chunk, 0) } else { (0, chunk) };
                    store.insert(&c, at, t).unwrap();
                }
                store.flush().unwrap();
                // Exercise cold neighbours as well as the in-memory path.
                drop(store);
                let mut store = Store::new(dir.0.clone(), vec![0, 0x808080]);
                let mut center = [8.0; 2];
                center[axis] = f64::from(boundary);
                let image = store
                    .compose(
                        &c,
                        &Render {
                            center,
                            bpp: 0.5,
                            aspect: 1.0,
                            size: 64,
                            shading: true,
                        },
                    )
                    .unwrap();
                let channel = |i: usize| {
                    let (x, z) = if axis == 0 { (i, 32) } else { (32, i) };
                    image[(z * 64 + x) * 4]
                };
                assert_eq!(channel(31), channel(32), "axis={axis} boundary={boundary}");
                assert_eq!(channel(32), channel(33));
                assert!(channel(32) < 128, "slope should still be shaded");
            }
        }
    }
    #[test]
    fn wide_explored_views_do_not_thrash_the_detailed_region_cache() {
        let dir = Dir::new();
        let c = context();
        let mut store = Store::new(dir.0.clone(), palette());
        for z in -4..4 {
            for x in -4..4 {
                store.insert(&c, (x * 16, z * 16), tile(1, 64)).unwrap();
            }
        }
        store.flush().unwrap();
        drop(store);
        let mut store = Store::new(dir.0.clone(), palette());
        let spec = Render {
            center: [0.0, 0.0],
            bpp: 16.0,
            aspect: 1.0,
            size: 512,
            shading: true,
        };
        let image = store.compose(&c, &spec).unwrap();
        assert!(image.as_chunks::<4>().0.contains(&[0x66, 0x99, 0x44, 255]));
        assert!(
            store.cache.is_empty(),
            "wide views should use checked overviews, not decode/evict all 64 regions"
        );
    }
    // Chunk with a red west half and a blue east half; its chunk summary is purple.
    fn split_tile() -> Tile {
        let mut t = tile(1, 64);
        for (i, rgb) in t.rgb.iter_mut().enumerate() {
            *rgb = if i % 16 < 8 { 0xffd01010 } else { 0xff1010d0 };
        }
        t
    }
    fn wide_view() -> Render {
        // 25 explored regions in view at 2.5 blocks per pixel.
        Render {
            center: [128.0, 128.0],
            bpp: 10.0,
            aspect: 1.0,
            size: 512,
            shading: false,
        }
    }
    fn saved_grid(dir: &Dir, c: &Context) {
        let mut store = Store::new(dir.0.clone(), palette());
        for z in -2..3 {
            for x in -2..3 {
                store.insert(c, (x * 16, z * 16), split_tile()).unwrap();
            }
        }
        store.flush().unwrap();
    }
    // Recompose like the worker does until the time-budgeted overviews are complete.
    fn compose_settled(store: &mut Store, c: &Context, spec: &Render) -> Vec<u8> {
        for _ in 0..100 {
            let image = store.compose(c, spec).unwrap();
            if !store.pending {
                return image;
            }
        }
        panic!("overviews never finished building");
    }
    #[test]
    fn zoomed_out_explored_map_keeps_block_detail_instead_of_chunk_averages() {
        let dir = Dir::new();
        let c = context();
        saved_grid(&dir, &c);
        let mut store = Store::new(dir.0.clone(), palette());
        let image = compose_settled(&mut store, &c, &wide_view());
        let pixels = image.as_chunks::<4>().0;
        assert!(pixels.contains(&[0xd0, 0x10, 0x10, 255]));
        assert!(pixels.contains(&[0x10, 0x10, 0xd0, 255]));
        assert!(
            !pixels.contains(&[0x70, 0x10, 0x70, 255]),
            "chunk-average placeholders must be replaced by overviews"
        );
        assert!(
            store.cache.is_empty(),
            "overviews are built without evicting the detailed region cache"
        );
    }
    #[test]
    fn live_terrain_updates_already_built_overviews() {
        let dir = Dir::new();
        let c = context();
        saved_grid(&dir, &c);
        let mut store = Store::new(dir.0.clone(), palette());
        store.compose(&c, &wide_view()).unwrap();
        let mut green = tile(1, 64);
        green.rgb = [0xff10d010; 256];
        store.insert(&c, (0, 0), green).unwrap();
        let image = compose_settled(&mut store, &c, &wide_view());
        assert!(image.as_chunks::<4>().0.contains(&[0x10, 0xd0, 0x10, 255]));
    }
    #[test]
    fn overview_memory_stays_bounded() {
        let mut store = Store::new(PathBuf::new(), palette());
        let region = Region {
            summary: vec![0; 1024],
            ..Region::default()
        };
        for x in 0..600 {
            let levels = build_thumb(&region, (x, 0), &palette(), false);
            store.thumbs.insert(
                ("id".into(), (x, 0), false),
                Thumb {
                    levels: levels.map(Some),
                    used: x as u64,
                },
            );
        }
        store.thumb_clock = 599;
        store.trim_thumbs();
        let total: usize = store.thumbs.values().map(Thumb::bytes).sum();
        assert!(total <= THUMB_BYTES);
        assert!(store.thumbs[&("id".into(), (599, 0), false)].levels[0].is_some());
        assert!(store.thumbs[&("id".into(), (0, 0), false)].levels[0].is_none());
        assert!(store.thumbs[&("id".into(), (0, 0), false)].levels[2].is_some());
    }
    #[test]
    fn explored_negative_chunks_survive_restart_and_zoom_out() {
        let dir = Dir::new();
        let c = context();
        // A crash before the first metadata rename can leave an empty context directory.
        fs::create_dir_all(
            dir.0
                .join(format!("{:016x}", hash(c.identity().as_bytes()))),
        )
        .unwrap();
        let mut store = Store::new(dir.0.clone(), palette());
        store.insert(&c, (-17, -1), tile(1, 72)).unwrap();
        store.flush().unwrap();
        drop(store);
        let mut restored = Store::new(dir.0.clone(), palette());
        let img = restored.compose(&c, &view([-264.0, -8.0])).unwrap();
        assert_eq!(center_pixel(&img, 128), [102, 153, 68, 255]);
        let far = Render {
            center: [-264.0, -8.0],
            bpp: 64.0,
            size: 512,
            ..view([0.0, 0.0])
        };
        assert!(
            restored
                .compose(&c, &far)
                .unwrap()
                .as_chunks::<4>()
                .0
                .contains(&[102, 153, 68, 255])
        );
        assert!(restored.dimensions(&c.scope).contains(&c.dimension));
    }
    #[test]
    fn server_account_dimension_and_cave_layers_do_not_mix() {
        let dir = Dir::new();
        let mut store = Store::new(dir.0.clone(), palette());
        let base = context();
        store.insert(&base, (0, 0), tile(1, 64)).unwrap();
        store.flush().unwrap();
        for c in [
            Context {
                scope: "another server".into(),
                ..base.clone()
            },
            Context {
                scope: "example:25565#other-account".into(),
                ..base.clone()
            },
            Context {
                dimension: "minecraft:the_nether".into(),
                ..base.clone()
            },
            Context {
                layer: 4,
                ..base.clone()
            },
        ] {
            assert_ne!(
                center_pixel(&store.compose(&c, &view([8.0, 8.0])).unwrap(), 128),
                [102, 153, 68, 255]
            );
            store.insert(&c, (0, 0), tile(2, 12)).unwrap();
            store.flush().unwrap();
            assert_eq!(
                center_pixel(&store.compose(&c, &view([8.0, 8.0])).unwrap(), 128),
                [115, 85, 51, 255]
            );
        }
        assert_eq!(
            center_pixel(&store.compose(&base, &view([8.0, 8.0])).unwrap(), 128),
            [102, 153, 68, 255]
        );
    }
    #[test]
    fn lru_eviction_saves_dirty_regions_without_losing_old_terrain() {
        let dir = Dir::new();
        let c = context();
        let mut store = Store::new(dir.0.clone(), palette());
        for x in 0..30 {
            store.insert(&c, (x * 16, 0), tile(1, x as i16)).unwrap();
        }
        assert!(store.cache.len() <= 24);
        store.flush().unwrap();
        drop(store);
        let mut store = Store::new(dir.0.clone(), palette());
        for x in [0, 15, 29] {
            assert_eq!(
                center_pixel(
                    &store
                        .compose(&c, &view([x as f64 * 256.0 + 8.0, 8.0]))
                        .unwrap(),
                    128
                ),
                [102, 153, 68, 255]
            );
        }
    }
    #[test]
    fn disabled_recording_never_creates_disk_files() {
        let dir = Dir::new();
        let c = Context {
            persistent: false,
            ..context()
        };
        let mut store = Store::new(dir.0.clone(), palette());
        store.insert(&c, (0, 0), tile(1, 64)).unwrap();
        store.flush().unwrap();
        assert_eq!(
            center_pixel(&store.compose(&c, &view([8.0, 8.0])).unwrap(), 128),
            [102, 153, 68, 255]
        );
        assert!(!dir.0.exists());
    }
    #[test]
    fn corrupt_or_truncated_region_is_isolated_and_never_overwritten() {
        let dir = Dir::new();
        let c = context();
        let mut store = Store::new(dir.0.clone(), palette());
        store.insert(&c, (0, 0), tile(1, 64)).unwrap();
        store.insert(&c, (16, 0), tile(2, 64)).unwrap();
        store.flush().unwrap();
        let file = store.catalogs[&c.id()].dir.join("0_0.region");
        let original = fs::read(&file).unwrap();
        drop(store);
        for bytes in [&original[..12], &original[..original.len() - 1]] {
            fs::write(&file, bytes).unwrap();
            let mut store = Store::new(dir.0.clone(), palette());
            assert!(store.insert(&c, (0, 0), tile(2, 64)).is_err());
            assert_ne!(
                center_pixel(&store.compose(&c, &view([8.0, 8.0])).unwrap(), 128),
                [102, 153, 68, 255]
            );
            assert!(store.warning.is_some());
            assert_eq!(
                center_pixel(&store.compose(&c, &view([264.0, 8.0])).unwrap(), 128),
                [115, 85, 51, 255]
            );
            store.flush().unwrap();
            assert_eq!(fs::read(&file).unwrap(), bytes);
        }
    }
    #[test]
    fn failed_flush_keeps_dirty_data_for_retry() {
        let dir = Dir::new();
        let c = context();
        let mut store = Store::new(dir.0.clone(), palette());
        store.insert(&c, (0, 0), tile(1, 64)).unwrap();
        fs::write(&dir.0, b"not a directory").unwrap();
        assert!(store.flush().is_err());
        assert!(store.cache.values().any(|r| r.dirty));
        fs::remove_file(&dir.0).unwrap();
        store.flush().unwrap();
        assert!(store.cache.values().all(|r| !r.dirty));
    }
    #[test]
    fn versioned_region_rejects_corruption_duplicate_records_and_wrong_coordinates() {
        let mut r = Region {
            summary: vec![0; 1024],
            ..Region::default()
        };
        r.tiles.insert(0, tile(1, 64));
        let bytes = r.encode((-1, 2));
        assert_eq!(
            Region::decode(&bytes, (-1, 2)).unwrap().tiles[&0],
            tile(1, 64)
        );
        assert!(Region::decode(&bytes, (0, 2)).is_err());
        let mut corrupt = bytes.clone();
        corrupt[1052] ^= 1;
        assert!(Region::decode(&corrupt, (-1, 2)).is_err());
        let mut duplicate = bytes[..bytes.len() - 8].to_vec();
        duplicate[1048..1050].copy_from_slice(&2u16.to_le_bytes());
        duplicate.extend(&bytes[1050..bytes.len() - 8]);
        duplicate.extend(hash(&duplicate).to_le_bytes());
        assert!(Region::decode(&duplicate, (-1, 2)).is_err());
    }
    #[test]
    fn saved_surface_never_falls_back_to_caves_after_restart() {
        let dir = Dir::new();
        let c = Context {
            dimension: "custom:moon".into(),
            layer: 5,
            ..context()
        };
        let mut store = Store::new(dir.0.clone(), palette());
        store.insert(&c, (0, 0), tile(1, 88)).unwrap();
        store.flush().unwrap();
        drop(store);
        let mut store = Store::new(dir.0.clone(), palette());
        let surface = Context {
            layer: i32::MAX,
            ..c.clone()
        };
        assert_eq!(store.saved_context(&surface), surface);
        let wanted = Context {
            layer: 6,
            ..c.clone()
        };
        assert_eq!(store.saved_context(&wanted), c);
        assert_eq!(store.saved_context(&surface), surface);
        // A cached cave must not become a surface after the first lookup either.
        assert_eq!(store.saved_context(&surface), surface);
        assert!(store.dimensions(&c.scope).contains(&"custom:moon".into()));
    }
}
