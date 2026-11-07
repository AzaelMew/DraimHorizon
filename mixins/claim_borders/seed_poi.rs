// Local found-POI state and click handling, shared by tests and the native seed map.
use super::{claim_borders_map_history::hash, claim_borders_seed_map as catalog};
use serde_json::{Value, json};
use std::{
    collections::BTreeSet,
    fs::{self, OpenOptions},
    io::{Read, Write},
    path::{Path, PathBuf},
    sync::atomic::{AtomicU64, Ordering},
};
const MAX_POINTS: usize = 8192;
const MAX_BYTES: usize = 1_048_576;

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct Poi {
    dimension: usize,
    id: String,
    x: i32,
    z: i32,
}
impl Poi {
    pub fn new(dimension: usize, id: &str, position: [f64; 2]) -> Option<Self> {
        let def = &catalog::STRUCTURES[catalog::structure_index(id)?];
        if dimension >= catalog::DIMENSIONS.len()
            || def.group.dimension() != dimension
            || !position
                .iter()
                .all(|p| p.is_finite() && p.abs() <= 30_000_000.0)
        {
            return None;
        }
        Some(Self {
            dimension,
            id: def.id.into(),
            x: position[0].floor() as i32,
            z: position[1].floor() as i32,
        })
    }
}
pub struct Found {
    scope: String,
    seed: i64,
    points: BTreeSet<Poi>,
}
impl Found {
    fn empty(scope: &str, seed: i64) -> Result<Self, String> {
        if scope.is_empty() || scope.len() > 1024 {
            return Err("invalid found-POI scope".into());
        }
        Ok(Self {
            scope: scope.into(),
            seed,
            points: BTreeSet::new(),
        })
    }
    fn path(&self, root: &Path) -> PathBuf {
        let identity = json!([self.scope, self.seed]).to_string();
        root.join("seed-found")
            .join(format!("{:016x}.json", hash(identity.as_bytes())))
    }
    fn read(path: &Path) -> Result<Option<Vec<u8>>, String> {
        let meta = match fs::symlink_metadata(path) {
            Ok(m) => m,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(None),
            Err(e) => return Err(e.to_string()),
        };
        if !meta.is_file() || meta.file_type().is_symlink() || meta.len() > MAX_BYTES as u64 {
            return Err("invalid found-POI file".into());
        }
        let mut bytes = Vec::new();
        fs::File::open(path)
            .map_err(|e| e.to_string())?
            .take(MAX_BYTES as u64 + 1)
            .read_to_end(&mut bytes)
            .map_err(|e| e.to_string())?;
        if bytes.len() > MAX_BYTES {
            return Err("found-POI file too large".into());
        }
        Ok(Some(bytes))
    }
    fn decode(bytes: &[u8], scope: &str, seed: i64) -> Result<Self, String> {
        if bytes.len() > MAX_BYTES {
            return Err("found-POI file too large".into());
        }
        let v: Value = serde_json::from_slice(bytes).map_err(|_| "invalid found-POI JSON")?;
        if v["schema"] != 1
            || v["scope"].as_str() != Some(scope)
            || v["seed"].as_i64() != Some(seed)
        {
            return Err("found-POI version/identity mismatch".into());
        }
        let points = v["points"]
            .as_array()
            .filter(|p| p.len() <= MAX_POINTS)
            .ok_or("invalid found-POI records")?;
        let mut found = Self::empty(scope, seed)?;
        for p in points {
            let p = p
                .as_array()
                .filter(|p| p.len() == 4)
                .ok_or("invalid found POI")?;
            let dimension = p[0]
                .as_u64()
                .and_then(|d| usize::try_from(d).ok())
                .ok_or("invalid POI dimension")?;
            let x = p[2]
                .as_i64()
                .and_then(|x| i32::try_from(x).ok())
                .ok_or("invalid POI coordinate")?;
            let z = p[3]
                .as_i64()
                .and_then(|z| i32::try_from(z).ok())
                .ok_or("invalid POI coordinate")?;
            let poi = Poi::new(
                dimension,
                p[1].as_str().ok_or("invalid POI kind")?,
                [f64::from(x), f64::from(z)],
            )
            .ok_or("invalid found POI")?;
            if !found.points.insert(poi) {
                return Err("duplicate found POI".into());
            }
        }
        Ok(found)
    }
    pub fn load(root: &Path, scope: &str, seed: i64) -> Result<Self, String> {
        match fs::symlink_metadata(root.join("seed-found")) {
            Ok(meta) if !meta.is_dir() || meta.file_type().is_symlink() => {
                return Err("invalid found-POI directory".into());
            }
            Err(e) if e.kind() != std::io::ErrorKind::NotFound => return Err(e.to_string()),
            _ => {}
        }
        let empty = Self::empty(scope, seed)?;
        match Self::read(&empty.path(root))? {
            Some(bytes) => Self::decode(&bytes, scope, seed),
            None => Ok(empty),
        }
    }
    pub fn contains(&self, poi: &Poi) -> bool {
        self.points.contains(poi)
    }
    pub fn toggle(&mut self, poi: Poi) -> Result<bool, String> {
        if self.points.remove(&poi) {
            return Ok(false);
        }
        if self.points.len() >= MAX_POINTS {
            return Err("found-POI limit reached".into());
        }
        self.points.insert(poi);
        Ok(true)
    }
    pub fn save(&self, root: &Path) -> Result<(), String> {
        let path = self.path(root);
        let dir = path.parent().ok_or("invalid found-POI path")?;
        fs::create_dir_all(dir).map_err(|e| e.to_string())?;
        if fs::symlink_metadata(dir)
            .map_err(|e| e.to_string())?
            .file_type()
            .is_symlink()
        {
            return Err("found-POI directory is a symlink".into());
        }
        if let Some(bytes) = Self::read(&path)? {
            Self::decode(&bytes, &self.scope, self.seed)?;
        }
        let points: Vec<_> = self
            .points
            .iter()
            .map(|p| json!([p.dimension, p.id, p.x, p.z]))
            .collect();
        let bytes =
            json!({"schema":1,"scope":self.scope,"seed":self.seed,"points":points}).to_string();
        if bytes.len() > MAX_BYTES {
            return Err("found-POI file too large".into());
        }
        static NEXT: AtomicU64 = AtomicU64::new(0);
        let pending = path.with_extension(format!(
            "pending-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        let mut options = OpenOptions::new();
        options.write(true).create_new(true);
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt;
            options.mode(0o600);
        }
        let mut file = options.open(&pending).map_err(|e| e.to_string())?;
        let result = (|| {
            file.write_all(bytes.as_bytes())?;
            file.sync_all()?;
            drop(file);
            fs::rename(&pending, &path)
        })();
        if result.is_err() {
            let _ = fs::remove_file(&pending);
        }
        result.map_err(|e| e.to_string())
    }
}
pub struct Click {
    poi: Poi,
    start: [f32; 2],
    moved: bool,
}
impl Click {
    pub fn new(poi: Poi, start: [f32; 2]) -> Self {
        Self {
            poi,
            start,
            moved: false,
        }
    }
    pub fn update(&mut self, point: [f32; 2]) {
        self.moved |= !point.iter().all(|p| p.is_finite())
            || (point[0] - self.start[0]).hypot(point[1] - self.start[1]) > 5.0;
    }
    pub fn finish(mut self, point: [f32; 2]) -> Option<Poi> {
        self.update(point);
        (!self.moved).then_some(self.poi)
    }
}
#[derive(Clone)]
pub struct Hit {
    pub rect: [f32; 4],
    pub poi: Poi,
    pub count: usize,
}
pub fn target(hits: &[Hit], point: [f32; 2], blocked: bool) -> Option<Poi> {
    if blocked {
        return None;
    }
    hits.iter()
        .rev()
        .find(|h| catalog::contains(h.rect, point))
        .filter(|h| h.count == 1)
        .map(|h| h.poi.clone())
}
/// HudBatch packs colour as little-endian RGBA; leave alpha unchanged.
pub fn dim_color(color: u32, found: bool) -> u32 {
    if !found {
        return color;
    }
    let [r, g, b, a] = color.to_le_bytes();
    let dim = |v: u8| (u16::from(v) * 55 / 100) as u8;
    u32::from_le_bytes([dim(r), dim(g), dim(b), a])
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{
        fs,
        path::PathBuf,
        sync::atomic::{AtomicU64, Ordering},
    };
    struct Dir(PathBuf);
    impl Dir {
        fn new() -> Self {
            static NEXT: AtomicU64 = AtomicU64::new(0);
            Self(std::env::temp_dir().join(format!(
                "draim-seed-found-{}-{}",
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
    fn key() -> Poi {
        Poi::new(0, "village_plains", [-120.0, 56.0]).unwrap()
    }
    #[test]
    fn found_pois_toggle_and_survive_restart_without_crossing_scopes_or_seeds() {
        let dir = Dir::new();
        let mut found = Found::load(&dir.0, "server#account", 42).unwrap();
        assert!(!found.contains(&key()));
        assert!(found.toggle(key()).unwrap());
        found.save(&dir.0).unwrap();
        let mut restored = Found::load(&dir.0, "server#account", 42).unwrap();
        assert!(restored.contains(&key()));
        assert!(
            !Found::load(&dir.0, "other#account", 42)
                .unwrap()
                .contains(&key())
        );
        assert!(
            !Found::load(&dir.0, "server#account", 43)
                .unwrap()
                .contains(&key())
        );
        assert!(!restored.contains(&Poi::new(1, "fortress", [-120.0, 56.0]).unwrap()));
        assert!(!restored.contains(&Poi::new(0, "village_desert", [-120.0, 56.0]).unwrap()));
        assert!(!restored.toggle(key()).unwrap());
        restored.save(&dir.0).unwrap();
        assert!(
            !Found::load(&dir.0, "server#account", 42)
                .unwrap()
                .contains(&key())
        );
    }
    #[test]
    fn corrupt_or_wrong_identity_data_is_preserved_instead_of_overwritten() {
        let dir = Dir::new();
        let mut found = Found::load(&dir.0, "scope", 42).unwrap();
        found.toggle(key()).unwrap();
        found.save(&dir.0).unwrap();
        let path = found.path(&dir.0);
        for bytes in [
            b"not json".as_slice(),
            br#"{"schema":99,"scope":"scope","seed":42,"points":[]}"#,
            br#"{"schema":1,"scope":"other","seed":42,"points":[]}"#,
        ] {
            fs::write(&path, bytes).unwrap();
            assert!(Found::load(&dir.0, "scope", 42).is_err());
            assert!(found.save(&dir.0).is_err());
            assert_eq!(fs::read(&path).unwrap(), bytes);
        }
    }
    #[test]
    fn failed_save_keeps_the_in_memory_edit_for_retry() {
        let dir = Dir::new();
        fs::create_dir_all(&dir.0).unwrap();
        let blocked = dir.0.join("file-not-directory");
        fs::write(&blocked, b"keep").unwrap();
        let mut found = Found::load(&dir.0, "scope", 42).unwrap();
        found.toggle(key()).unwrap();
        assert!(found.save(&blocked).is_err());
        assert!(found.contains(&key()));
        found.save(&dir.0).unwrap();
        assert!(Found::load(&dir.0, "scope", 42).unwrap().contains(&key()));
        assert_eq!(fs::read(blocked).unwrap(), b"keep");
    }
    #[test]
    fn invalid_pois_and_oversized_or_duplicate_records_are_rejected() {
        assert!(Poi::new(0, "fortress", [0.0, 0.0]).is_none());
        assert!(Poi::new(3, "village_plains", [0.0, 0.0]).is_none());
        assert!(Poi::new(0, "unknown", [0.0, 0.0]).is_none());
        assert!(Poi::new(0, "village_plains", [f64::NAN, 0.0]).is_none());
        assert!(Poi::new(0, "village_plains", [30_000_001.0, 0.0]).is_none());
        let duplicate = br#"{"schema":1,"scope":"scope","seed":42,"points":[[0,"village_plains",0,0],[0,"village_plains",0,0]]}"#;
        assert!(Found::decode(duplicate, "scope", 42).is_err());
        assert!(Found::decode(&vec![b' '; MAX_BYTES + 1], "scope", 42).is_err());
        let dir = Dir::new();
        let mut found = Found::load(&dir.0, "scope", 42).unwrap();
        for x in 0..MAX_POINTS {
            found
                .toggle(Poi::new(0, "village_plains", [x as f64, 0.0]).unwrap())
                .unwrap();
        }
        assert!(
            found
                .toggle(Poi::new(0, "village_plains", [MAX_POINTS as f64, 0.0]).unwrap())
                .is_err()
        );
        assert!(
            !found
                .toggle(Poi::new(0, "village_plains", [0.0, 0.0]).unwrap())
                .unwrap()
        );
    }
    #[cfg(unix)]
    #[test]
    fn symlinked_found_directories_cannot_redirect_loading_or_saving() {
        use std::os::unix::fs::symlink;
        let dir = Dir::new();
        let real = dir.0.join("real");
        let linked = dir.0.join("linked");
        let mut found = Found::load(&real, "scope", 42).unwrap();
        found.toggle(key()).unwrap();
        found.save(&real).unwrap();
        fs::create_dir_all(&linked).unwrap();
        symlink(real.join("seed-found"), linked.join("seed-found")).unwrap();
        let before = fs::read(found.path(&real)).unwrap();
        assert!(Found::load(&linked, "scope", 42).is_err());
        assert!(found.save(&linked).is_err());
        assert_eq!(fs::read(found.path(&real)).unwrap(), before);
    }
    #[test]
    fn click_toggles_only_on_release_and_drag_cancellation_is_sticky() {
        let click = Click::new(key(), [20.0, 20.0]);
        assert_eq!(click.finish([22.0, 21.0]), Some(key()));
        let mut drag = Click::new(key(), [20.0, 20.0]);
        drag.update([30.0, 20.0]);
        assert_eq!(drag.finish([20.0, 20.0]), None);
        assert_eq!(Click::new(key(), [20.0, 20.0]).finish([26.0, 20.0]), None);
    }
    #[test]
    fn hit_testing_ignores_clusters_and_blocked_map_regions() {
        let hits = vec![Hit {
            rect: [10.0, 10.0, 22.0, 22.0],
            poi: key(),
            count: 1,
        }];
        assert_eq!(target(&hits, [20.0, 20.0], false), Some(key()));
        assert_eq!(target(&hits, [20.0, 20.0], true), None);
        assert_eq!(target(&hits, [35.0, 20.0], false), None);
        assert_eq!(
            target(
                &[Hit {
                    count: 2,
                    ..hits[0].clone()
                }],
                [20.0, 20.0],
                false
            ),
            None
        );
    }
    #[test]
    fn found_dimming_preserves_alpha_and_unfound_colours() {
        assert_eq!(dim_color(0x80ff_ffff, false), 0x80ff_ffff);
        assert_eq!(dim_color(0x80ff_ffff, true), 0x808c_8c8c);
        assert_eq!(dim_color(0x0000_0000, true), 0);
    }
}
