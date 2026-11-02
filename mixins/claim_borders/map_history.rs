// Bounded in-memory region cache and atomic, versioned explored terrain storage.
use std::{
    collections::{BTreeMap, BTreeSet},
    fs,
    io::{Read, Write},
    path::{Path, PathBuf},
};
const SIDE: i32 = 16;
const REGION: i32 = SIDE * 16;
const MAX_BYTES: u64 = 280_000;
const MAGIC: &[u8; 8] = b"DHMAP01\0";
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
}
impl Tile {
    pub fn fingerprint(&self) -> u64 {
        self.color
            .iter()
            .copied()
            .chain(self.height.iter().flat_map(|h| h.to_le_bytes()))
            .chain(self.depth.iter().copied())
            .fold(0xcbf29ce484222325, |h, b| {
                (h ^ u64::from(b)).wrapping_mul(0x100000001b3)
            })
    }
    fn bytes(&self) -> Vec<u8> {
        let mut out = Vec::with_capacity(1024);
        out.extend(self.color);
        for h in self.height {
            out.extend(h.to_le_bytes());
        }
        out.extend(self.depth);
        out
    }
    fn decode(data: &[u8]) -> Self {
        let mut t = Self {
            color: [0; 256],
            height: [0; 256],
            depth: [0; 256],
        };
        t.color.copy_from_slice(&data[..256]);
        for (i, h) in t.height.iter_mut().enumerate() {
            *h = i16::from_le_bytes([data[256 + i * 2], data[257 + i * 2]]);
        }
        t.depth.copy_from_slice(&data[768..1024]);
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
        if bytes.len() < 1058 || bytes.len() as u64 > MAX_BYTES || &bytes[..8] != MAGIC {
            return Err("invalid map region header".into());
        }
        let rx = i32::from_le_bytes(bytes[8..12].try_into().unwrap());
        let rz = i32::from_le_bytes(bytes[12..16].try_into().unwrap());
        let count = u16::from_le_bytes(bytes[1048..1050].try_into().unwrap()) as usize;
        if (rx, rz) != at
            || count > 256
            || bytes.len() != 1058 + count * 1026
            || hash(&bytes[..bytes.len() - 8])
                != u64::from_le_bytes(bytes[bytes.len() - 8..].try_into().unwrap())
        {
            return Err("corrupt map region".into());
        }
        let mut region = Self {
            summary: bytes[16..1040].to_vec(),
            ..Self::default()
        };
        for entry in bytes[1050..bytes.len() - 8].as_chunks::<1026>().0.iter() {
            let id = u16::from_le_bytes([entry[0], entry[1]]);
            if id >= 256 || region.tiles.insert(id, Tile::decode(&entry[2..])).is_some() {
                return Err("duplicate map chunk".into());
            }
        }
        Ok(region)
    }
    fn column(&self, x: i32, z: i32) -> Option<(u8, i16, u8)> {
        let id =
            ((z.div_euclid(16).rem_euclid(SIDE)) * SIDE + x.div_euclid(16).rem_euclid(SIDE)) as u16;
        let t = self.tiles.get(&id)?;
        let i = (z.rem_euclid(16) * 16 + x.rem_euclid(16)) as usize;
        Some((t.color[i], t.height[i], t.depth[i]))
    }
}
struct Catalog {
    context: Context,
    dir: PathBuf,
    regions: BTreeSet<(i32, i32)>,
}
pub struct Store {
    root: PathBuf,
    catalogs: BTreeMap<String, Catalog>,
    cache: BTreeMap<(String, (i32, i32)), Region>,
    palette: Vec<u32>,
    clock: u64,
    pub warning: Option<String>,
}
impl Store {
    pub fn new(root: PathBuf, palette: Vec<u32>) -> Self {
        Self {
            root,
            catalogs: BTreeMap::new(),
            cache: BTreeMap::new(),
            palette,
            clock: 0,
            warning: None,
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
        for color in tile.color {
            if let Some(&col) = self.palette.get(usize::from(color & 0x7f))
                && color & 0x7f != 0
            {
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
        Ok(())
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
        if !c.persistent {
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
        let size = spec.size.clamp(64, 512);
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
            || !(0.25..=524288.0).contains(&spec.bpp)
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
        for at in regions {
            let key = (id.clone(), at);
            let precise = step < 8.0;
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
                    if &prefix[..8] != MAGIC
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
            let from = [
                (f64::from(at.0) * f64::from(REGION) - low[0]) / (step * spec.aspect),
                (f64::from(at.1) * f64::from(REGION) - low[1]) / step,
            ];
            let to = [
                from[0] + f64::from(REGION) / (step * spec.aspect),
                from[1] + f64::from(REGION) / step,
            ];
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
                    let rgba = r.and_then(|r| shade(r, wx, wz, &self.palette, spec.shading));
                    if let Some(color) = rgba {
                        let i = (y * size + x) * 4;
                        image[i..i + 4].copy_from_slice(&color);
                    }
                }
            }
        }
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
fn shade(r: &Region, x: i32, z: i32, palette: &[u32], shading: bool) -> Option<[u8; 4]> {
    let (c, h, d) = r.column(x, z)?;
    let id = usize::from(c & 0x7f);
    if id == 0 {
        return None;
    }
    let color = *palette.get(id)?;
    let mut brightness = 1.0f32;
    if shading {
        let north = if z.rem_euclid(REGION) > 0 {
            r.column(x, z - 1).map_or(h, |p| p.1)
        } else {
            h
        };
        let west = if x.rem_euclid(REGION) > 0 {
            r.column(x - 1, z).map_or(h, |p| p.1)
        } else {
            h
        };
        brightness = (1.0
            + f32::from(h.saturating_sub(north).clamp(-12, 12)) * 0.035
            + f32::from(h.saturating_sub(west).clamp(-12, 12)) * 0.02)
            .clamp(0.65, 1.2);
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
        }
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
    fn browsing_saved_dimension_finds_its_cave_layer_after_restart() {
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
        let wanted = Context {
            layer: i32::MAX,
            ..c.clone()
        };
        assert_eq!(store.saved_context(&wanted), c);
        assert!(store.dimensions(&c.scope).contains(&"custom:moon".into()));
    }
}
