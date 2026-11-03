// Pure waypoint model shared with the injected Horizon module.
use std::collections::{BTreeMap, BTreeSet};
use std::fmt;
use std::fs::{self, File, OpenOptions};
use std::io::{Read, Write};
use std::path::Path;
use std::sync::atomic::{AtomicU64, Ordering};

use serde_json::{Value, json};

pub const MAX_WAYPOINTS_PER_SERVER: usize = 64;
const MAX_SERVERS: usize = 256;
const MAX_FILE_BYTES: u64 = 262_144;
const MAX_NAME_CHARS: usize = 48;
const MAX_WORLD: f64 = 30_000_000.0;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum MarkerKind {
    #[default]
    Pin,
    Home,
    Portal,
    Chest,
    Star,
}
impl MarkerKind {
    pub fn code(self) -> &'static str {
        match self {
            Self::Pin => "pin",
            Self::Home => "home",
            Self::Portal => "portal",
            Self::Chest => "chest",
            Self::Star => "star",
        }
    }
    pub fn label(self) -> &'static str {
        match self {
            Self::Pin => "Метка",
            Self::Home => "Дом",
            Self::Portal => "Портал",
            Self::Chest => "Хранилище",
            Self::Star => "Важное",
        }
    }
    pub fn parse(s: &str) -> Option<Self> {
        Some(match s {
            "pin" => Self::Pin,
            "home" => Self::Home,
            "portal" => Self::Portal,
            "chest" => Self::Chest,
            "star" => Self::Star,
            _ => return None,
        })
    }
    pub fn next(self) -> Self {
        match self {
            Self::Pin => Self::Home,
            Self::Home => Self::Portal,
            Self::Portal => Self::Chest,
            Self::Chest => Self::Star,
            Self::Star => Self::Pin,
        }
    }
}
#[derive(Clone, Debug, PartialEq)]
pub struct Waypoint {
    pub id: u64,
    pub name: String,
    pub dimension: String,
    pub position: [f64; 3],
    /// Opaque RGB. The renderer applies distance-dependent alpha separately.
    pub color: u32,
    pub kind: MarkerKind,
}

impl Waypoint {
    pub fn distance_from(&self, position: [f64; 3]) -> Option<f64> {
        distance(self.position, position)
    }

    pub fn opacity_from(&self, position: [f64; 3]) -> Option<f32> {
        self.distance_from(position).map(opacity)
    }
}

#[derive(Debug)]
pub enum WaypointError {
    Invalid(&'static str),
    NotFound,
    Limit,
    Io(std::io::Error),
}

impl fmt::Display for WaypointError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Invalid(message) => f.write_str(message),
            Self::NotFound => f.write_str("No waypoint with that id on this server."),
            Self::Limit => f.write_str("Waypoint storage limit reached."),
            Self::Io(error) => write!(f, "Cannot read or save waypoints: {error}"),
        }
    }
}

impl From<std::io::Error> for WaypointError {
    fn from(value: std::io::Error) -> Self {
        Self::Io(value)
    }
}

impl std::error::Error for WaypointError {}

#[derive(Clone, Debug)]
pub struct WaypointStore {
    servers: BTreeMap<String, Vec<Waypoint>>,
    next_id: u64,
}

impl Default for WaypointStore {
    fn default() -> Self {
        Self {
            servers: BTreeMap::new(),
            next_id: 1,
        }
    }
}

impl WaypointStore {
    pub fn add(
        &mut self,
        server: &str,
        dimension: &str,
        name: &str,
        position: [f64; 3],
        color: u32,
    ) -> Result<u64, WaypointError> {
        validate_server(server)?;
        let dimension = normalize_dimension(dimension)?;
        let name = validate_name(name)?.to_owned();
        validate_position(position)?;
        validate_color(color)?;
        if self.servers.get(server).map_or(0, Vec::len) >= MAX_WAYPOINTS_PER_SERVER
            || (!self.servers.contains_key(server) && self.servers.len() >= MAX_SERVERS)
        {
            return Err(WaypointError::Limit);
        }
        let id = self.next_id;
        let next = id.checked_add(1).ok_or(WaypointError::Limit)?;
        self.servers
            .entry(server.to_owned())
            .or_default()
            .push(Waypoint {
                id,
                name,
                dimension,
                position,
                color,
                kind: MarkerKind::Pin,
            });
        self.next_id = next;
        Ok(id)
    }

    pub fn update(
        &mut self,
        server: &str,
        id: u64,
        name: &str,
        color: u32,
        kind: MarkerKind,
    ) -> Result<(), WaypointError> {
        let name = validate_name(name)?.to_owned();
        validate_color(color)?;
        let w = self.find_mut(server, id)?;
        w.name = name;
        w.color = color;
        w.kind = kind;
        Ok(())
    }
    pub fn set_kind(
        &mut self,
        server: &str,
        id: u64,
        kind: MarkerKind,
    ) -> Result<(), WaypointError> {
        self.find_mut(server, id)?.kind = kind;
        Ok(())
    }
    pub fn rename(&mut self, server: &str, id: u64, name: &str) -> Result<(), WaypointError> {
        let name = validate_name(name)?.to_owned();
        self.find_mut(server, id)?.name = name;
        Ok(())
    }

    pub fn recolor(&mut self, server: &str, id: u64, color: u32) -> Result<(), WaypointError> {
        validate_color(color)?;
        self.find_mut(server, id)?.color = color;
        Ok(())
    }

    pub fn remove(&mut self, server: &str, id: u64) -> Result<(), WaypointError> {
        let entries = self
            .servers
            .get_mut(server)
            .ok_or(WaypointError::NotFound)?;
        let index = entries
            .iter()
            .position(|w| w.id == id)
            .ok_or(WaypointError::NotFound)?;
        entries.remove(index);
        if entries.is_empty() {
            self.servers.remove(server);
        }
        Ok(())
    }

    pub fn list(&self, server: &str, dimension: &str) -> Vec<&Waypoint> {
        let Ok(dimension) = normalize_dimension(dimension) else {
            return Vec::new();
        };
        self.servers
            .get(server)
            .into_iter()
            .flatten()
            .filter(|w| w.dimension == dimension)
            .collect()
    }

    pub fn all(&self, server: &str) -> Vec<&Waypoint> {
        self.servers.get(server).into_iter().flatten().collect()
    }

    fn find_mut(&mut self, server: &str, id: u64) -> Result<&mut Waypoint, WaypointError> {
        self.servers
            .get_mut(server)
            .and_then(|entries| entries.iter_mut().find(|w| w.id == id))
            .ok_or(WaypointError::NotFound)
    }

    pub fn load(path: &Path) -> Result<Self, WaypointError> {
        let metadata = match fs::symlink_metadata(path) {
            Ok(metadata) => metadata,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                return Ok(Self::default());
            }
            Err(error) => return Err(error.into()),
        };
        if !metadata.is_file() || metadata.len() > MAX_FILE_BYTES {
            return Err(WaypointError::Invalid(
                "Waypoint file is too large or is not a regular file.",
            ));
        }
        let mut file = File::open(path)?;
        let mut bytes = Vec::new();
        Read::by_ref(&mut file)
            .take(MAX_FILE_BYTES + 1)
            .read_to_end(&mut bytes)?;
        if bytes.len() as u64 > MAX_FILE_BYTES {
            return Err(WaypointError::Invalid("Waypoint file is too large."));
        }
        Self::from_bytes(&bytes)
    }

    pub fn save(&self, path: &Path) -> Result<(), WaypointError> {
        // Validate an existing file before replacing it. A corrupt or unknown
        // schema must be repaired by the owner, never silently reset by save.
        match fs::symlink_metadata(path) {
            Ok(_) => {
                Self::load(path)?;
            }
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            Err(error) => return Err(error.into()),
        }
        let bytes = self.to_bytes()?;
        let parent = path
            .parent()
            .filter(|p| !p.as_os_str().is_empty())
            .unwrap_or(Path::new("."));
        fs::create_dir_all(parent)?;
        static SERIAL: AtomicU64 = AtomicU64::new(0);
        let filename = path
            .file_name()
            .ok_or(WaypointError::Invalid("Waypoint file needs a name."))?;
        let temporary = parent.join(format!(
            ".{}.{}.{}.tmp",
            filename.to_string_lossy(),
            std::process::id(),
            SERIAL.fetch_add(1, Ordering::Relaxed)
        ));
        // Only clean up a temporary file created by this save operation.
        let mut file = OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&temporary)?;
        let result = (|| {
            file.write_all(&bytes)?;
            file.sync_all()?;
            drop(file);
            fs::rename(&temporary, path)?;
            Ok(())
        })();
        if result.is_err() {
            let _ = fs::remove_file(&temporary);
        }
        result
    }

    fn to_bytes(&self) -> Result<Vec<u8>, WaypointError> {
        let servers: serde_json::Map<String, Value> = self.servers.iter().map(|(server, entries)| {
            let entries: Vec<Value> = entries.iter().map(|w| json!({
                "id": w.id, "name": w.name, "dimension": w.dimension, "position": w.position, "color": w.color, "kind":w.kind.code(),
            })).collect();
            (server.clone(), Value::Array(entries))
        }).collect();
        let mut bytes = serde_json::to_vec_pretty(
            &json!({"schema": 2, "next_id": self.next_id, "servers": servers}),
        )
        .map_err(|_| WaypointError::Invalid("Cannot encode waypoint storage."))?;
        bytes.push(b'\n');
        if bytes.len() as u64 > MAX_FILE_BYTES {
            return Err(WaypointError::Limit);
        }
        Ok(bytes)
    }

    fn from_bytes(bytes: &[u8]) -> Result<Self, WaypointError> {
        let value: Value = serde_json::from_slice(bytes)
            .map_err(|_| WaypointError::Invalid("Waypoint file is not valid JSON."))?;
        exact_keys(&value, &["schema", "next_id", "servers"])?;
        let schema = value["schema"].as_u64();
        if !matches!(schema, Some(1 | 2)) {
            return Err(WaypointError::Invalid("Unknown waypoint schema."));
        }
        let next_id = value["next_id"]
            .as_u64()
            .filter(|n| *n > 0)
            .ok_or(WaypointError::Invalid("Invalid waypoint sequence."))?;
        let servers = value["servers"]
            .as_object()
            .ok_or(WaypointError::Invalid("Invalid waypoint server list."))?;
        if servers.len() > MAX_SERVERS {
            return Err(WaypointError::Limit);
        }
        let mut store = Self {
            servers: BTreeMap::new(),
            next_id,
        };
        let mut ids = BTreeSet::new();
        for (server, entries) in servers {
            validate_server(server)?;
            let entries = entries
                .as_array()
                .filter(|e| e.len() <= MAX_WAYPOINTS_PER_SERVER)
                .ok_or(WaypointError::Invalid("Invalid waypoint list."))?;
            let mut parsed = Vec::new();
            for entry in entries {
                if schema == Some(1) {
                    exact_keys(entry, &["id", "name", "dimension", "position", "color"])?;
                } else {
                    exact_keys(
                        entry,
                        &["id", "name", "dimension", "position", "color", "kind"],
                    )?;
                }
                let id = entry["id"]
                    .as_u64()
                    .filter(|n| *n > 0 && *n < next_id)
                    .ok_or(WaypointError::Invalid("Invalid waypoint id."))?;
                if !ids.insert(id) {
                    return Err(WaypointError::Invalid("Duplicate waypoint id."));
                }
                let name = validate_name(
                    entry["name"]
                        .as_str()
                        .ok_or(WaypointError::Invalid("Invalid waypoint name."))?,
                )?
                .to_owned();
                let dimension = normalize_dimension(
                    entry["dimension"]
                        .as_str()
                        .ok_or(WaypointError::Invalid("Invalid waypoint dimension."))?,
                )?;
                let coords = entry["position"]
                    .as_array()
                    .filter(|a| a.len() == 3)
                    .ok_or(WaypointError::Invalid("Invalid waypoint position."))?;
                let mut position = [0.0; 3];
                for (i, coordinate) in coords.iter().enumerate() {
                    position[i] = coordinate
                        .as_f64()
                        .ok_or(WaypointError::Invalid("Invalid waypoint coordinate."))?;
                }
                validate_position(position)?;
                let color = entry["color"]
                    .as_u64()
                    .and_then(|n| u32::try_from(n).ok())
                    .ok_or(WaypointError::Invalid("Invalid waypoint color."))?;
                validate_color(color)?;
                parsed.push(Waypoint {
                    id,
                    name,
                    dimension,
                    position,
                    color,
                    kind: if schema == Some(1) {
                        MarkerKind::Pin
                    } else {
                        MarkerKind::parse(entry["kind"].as_str().unwrap_or(""))
                            .ok_or(WaypointError::Invalid("Invalid marker kind."))?
                    },
                });
            }
            if !parsed.is_empty() {
                store.servers.insert(server.clone(), parsed);
            }
        }
        Ok(store)
    }
}

fn exact_keys(value: &Value, keys: &[&str]) -> Result<(), WaypointError> {
    let object = value
        .as_object()
        .ok_or(WaypointError::Invalid("Waypoint object expected."))?;
    if object.len() != keys.len() || keys.iter().any(|key| !object.contains_key(*key)) {
        return Err(WaypointError::Invalid(
            "Unknown or missing waypoint fields.",
        ));
    }
    Ok(())
}

fn validate_server(server: &str) -> Result<(), WaypointError> {
    if server.is_empty() || server.len() > 256 || server.chars().any(char::is_control) {
        return Err(WaypointError::Invalid("Invalid server identity."));
    }
    Ok(())
}

fn validate_name(name: &str) -> Result<&str, WaypointError> {
    let name = name.trim();
    if name.is_empty()
        || name.chars().count() > MAX_NAME_CHARS
        || name.chars().any(|c| c.is_control() || c == '\u{00a7}')
    {
        return Err(WaypointError::Invalid(
            "Name needs 1-48 characters without formatting or control codes.",
        ));
    }
    Ok(name)
}

pub fn normalize_dimension(value: &str) -> Result<String, WaypointError> {
    let value = if value.contains(':') {
        value.to_owned()
    } else {
        format!("minecraft:{value}")
    };
    let (namespace, path) = value
        .split_once(':')
        .ok_or(WaypointError::Invalid("Invalid dimension."))?;
    if value.len() > 256
        || namespace.is_empty()
        || path.is_empty()
        || !namespace
            .bytes()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || b"_.-".contains(&c))
        || !path
            .bytes()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || b"_.-/".contains(&c))
    {
        return Err(WaypointError::Invalid("Invalid dimension."));
    }
    Ok(value)
}

fn validate_position(position: [f64; 3]) -> Result<(), WaypointError> {
    if !position.iter().all(|n| n.is_finite())
        || position[0].abs() > MAX_WORLD
        || position[2].abs() > MAX_WORLD
        || position[1].abs() > 2048.0
    {
        return Err(WaypointError::Invalid(
            "Waypoint position is outside supported world bounds.",
        ));
    }
    Ok(())
}

fn validate_color(color: u32) -> Result<(), WaypointError> {
    if color > 0xFF_FFFF {
        return Err(WaypointError::Invalid("Color must be RGB, without alpha."));
    }
    Ok(())
}

pub fn parse_color(value: &str) -> Result<u32, WaypointError> {
    let value = value.strip_prefix('#').unwrap_or(value);
    if value.len() != 6 || !value.bytes().all(|c| c.is_ascii_hexdigit()) {
        return Err(WaypointError::Invalid("Color must be #RRGGBB."));
    }
    u32::from_str_radix(value, 16).map_err(|_| WaypointError::Invalid("Color must be #RRGGBB."))
}

pub fn distance(a: [f64; 3], b: [f64; 3]) -> Option<f64> {
    if !a.iter().chain(b.iter()).all(|n| n.is_finite()) {
        return None;
    }
    let result = (a[0] - b[0]).hypot(a[1] - b[1]).hypot(a[2] - b[2]);
    result.is_finite().then_some(result)
}

/// Distant markers fade smoothly but remain discoverable (22% minimum alpha).
pub fn opacity(distance: f64) -> f32 {
    if !distance.is_finite() || distance < 0.0 {
        return 0.22;
    }
    (1.0 / (1.0 + distance / 256.0)).clamp(0.22, 1.0) as f32
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_path() -> std::path::PathBuf {
        static SERIAL: AtomicU64 = AtomicU64::new(0);
        std::env::temp_dir().join(format!(
            "horizon-waypoints-test-{}-{}.json",
            std::process::id(),
            SERIAL.fetch_add(1, Ordering::Relaxed)
        ))
    }

    #[test]
    fn names_colors_rename_and_immutable_ids() {
        let mut store = WaypointStore::default();
        let id = store
            .add(
                "example.test:25565",
                "overworld",
                "База",
                [0.0, 64.0, -16.0],
                parse_color("#55AAFF").unwrap(),
            )
            .unwrap();
        store.rename("example.test:25565", id, "Дом друга").unwrap();
        store.recolor("example.test:25565", id, 0xFF0000).unwrap();
        assert_eq!(store.all("example.test:25565")[0].name, "Дом друга");
        assert_eq!(store.all("example.test:25565")[0].color, 0xFF0000);
        store.remove("example.test:25565", id).unwrap();
        assert!(
            store
                .add("example.test:25565", "overworld", "New", [0.0, 0.0, 0.0], 0)
                .unwrap()
                > id
        );
    }

    #[test]
    fn server_and_dimension_isolation() {
        let mut store = WaypointStore::default();
        let id = store
            .add("a", "overworld", "A", [0.0, 64.0, 0.0], 0)
            .unwrap();
        store
            .add("a", "the_nether", "N", [0.0, 64.0, 0.0], 1)
            .unwrap();
        store
            .add("b", "overworld", "B", [0.0, 64.0, 0.0], 2)
            .unwrap();
        assert_eq!(store.list("a", "minecraft:overworld").len(), 1);
        assert_eq!(store.list("a", "the_nether")[0].name, "N");
        assert!(store.rename("b", id, "steal").is_err());
        assert_eq!(store.all("b")[0].name, "B");
    }

    #[test]
    fn rejects_invalid_input_without_mutation() {
        let mut store = WaypointStore::default();
        for (name, position, color) in [
            ("", [0.0, 64.0, 0.0], 0),
            ("bad\nname", [0.0, 64.0, 0.0], 0),
            ("§aFake", [0.0, 64.0, 0.0], 0),
            ("Bad", [f64::NAN, 0.0, 0.0], 0),
            ("Bad", [0.0, 2049.0, 0.0], 0),
            ("Bad", [0.0, 0.0, 0.0], 0xFF00_0000),
        ] {
            assert!(
                store
                    .add("server", "overworld", name, position, color)
                    .is_err()
            );
        }
        assert!(
            store
                .add("server", "bad:dimension:extra", "Bad", [0.0, 0.0, 0.0], 0)
                .is_err()
        );
        assert!(store.all("server").is_empty());
        assert!(parse_color("#GGAAFF").is_err());
        assert!(parse_color("#FF").is_err());
    }

    #[test]
    fn enforces_server_limit_and_removal_reopens_slot() {
        let mut store = WaypointStore::default();
        for _ in 0..MAX_WAYPOINTS_PER_SERVER {
            store
                .add("server", "overworld", "Point", [0.0, 64.0, 0.0], 0)
                .unwrap();
        }
        assert!(matches!(
            store.add("server", "overworld", "Extra", [0.0, 64.0, 0.0], 0),
            Err(WaypointError::Limit)
        ));
        store.remove("server", 1).unwrap();
        assert!(
            store
                .add("server", "overworld", "Extra", [0.0, 64.0, 0.0], 0)
                .is_ok()
        );
    }

    #[test]
    fn persistent_roundtrip_preserves_ids_and_server_keys() {
        let path = temp_path();
        let mut store = WaypointStore::load(&path).unwrap();
        let id = store
            .add(
                "host:25565",
                "overworld",
                "Home",
                [1.5, 64.0, -2.5],
                0xABCDEF,
            )
            .unwrap();
        store.save(&path).unwrap();
        let mut loaded = WaypointStore::load(&path).unwrap();
        assert_eq!(loaded.all("host:25565"), store.all("host:25565"));
        assert!(
            loaded
                .add("host:25565", "overworld", "Next", [0.0, 0.0, 0.0], 0)
                .unwrap()
                > id
        );
        fs::remove_file(path).unwrap();
    }

    #[test]
    fn malformed_or_unknown_file_is_preserved() {
        let path = temp_path();
        let corrupt = b"{broken waypoint data";
        fs::write(&path, corrupt).unwrap();
        assert!(WaypointStore::load(&path).is_err());
        assert!(WaypointStore::default().save(&path).is_err());
        assert_eq!(fs::read(&path).unwrap(), corrupt);
        fs::write(&path, br#"{"schema":99,"next_id":1,"servers":{}}"#).unwrap();
        assert!(WaypointStore::load(&path).is_err());
        fs::remove_file(path).unwrap();
    }

    #[test]
    fn oversized_file_is_preserved() {
        let path = temp_path();
        let bytes = vec![b' '; MAX_FILE_BYTES as usize + 1];
        fs::write(&path, &bytes).unwrap();
        assert!(WaypointStore::load(&path).is_err());
        assert!(WaypointStore::default().save(&path).is_err());
        assert_eq!(fs::metadata(&path).unwrap().len(), MAX_FILE_BYTES + 1);
        fs::remove_file(path).unwrap();
    }

    #[test]
    #[cfg(unix)]
    fn refuses_to_follow_or_replace_symbolic_links() {
        let path = temp_path();
        let target = temp_path();
        std::os::unix::fs::symlink(&target, &path).unwrap();
        assert!(WaypointStore::load(&path).is_err());
        assert!(WaypointStore::default().save(&path).is_err());
        assert!(
            fs::symlink_metadata(&path)
                .unwrap()
                .file_type()
                .is_symlink()
        );
        assert!(!target.exists());
        fs::remove_file(path).unwrap();
    }

    #[test]
    fn rejects_duplicate_ids_and_invalid_sequence() {
        let mut store = WaypointStore::default();
        store
            .add("a", "overworld", "Home", [0.0, 64.0, 0.0], 0)
            .unwrap();
        let mut value: Value = serde_json::from_slice(&store.to_bytes().unwrap()).unwrap();
        value["servers"]["b"] = value["servers"]["a"].clone();
        assert!(WaypointStore::from_bytes(&serde_json::to_vec(&value).unwrap()).is_err());
        value["servers"].as_object_mut().unwrap().remove("b");
        value["next_id"] = json!(1);
        assert!(WaypointStore::from_bytes(&serde_json::to_vec(&value).unwrap()).is_err());
    }

    #[test]
    fn distance_and_opacity_fade_without_disappearing() {
        assert_eq!(distance([0.0, 0.0, 0.0], [3.0, 4.0, 0.0]), Some(5.0));
        assert_eq!(distance([f64::NAN, 0.0, 0.0], [0.0, 0.0, 0.0]), None);
        assert!(opacity(16.0) > opacity(512.0));
        assert!(opacity(512.0) > opacity(2048.0));
        assert_eq!(opacity(1e9), 0.22);
        assert_eq!(opacity(0.0), 1.0);
    }
    #[test]
    fn legacy_markers_migrate_without_losing_ids_and_kinds_roundtrip() {
        let raw=br#"{"schema":1,"next_id":8,"servers":{"test":[{"id":7,"name":"Home","dimension":"minecraft:overworld","position":[1,64,2],"color":123}]}}"#;
        let mut store = WaypointStore::from_bytes(raw).unwrap();
        assert_eq!(store.all("test")[0].kind, MarkerKind::Pin);
        store
            .update("test", 7, "Portal", 0xffcc55, MarkerKind::Portal)
            .unwrap();
        let new = WaypointStore::from_bytes(&store.to_bytes().unwrap()).unwrap();
        assert_eq!(new.all("test")[0].id, 7);
        assert_eq!(new.all("test")[0].position, [1., 64., 2.]);
        assert_eq!(new.all("test")[0].kind, MarkerKind::Portal);
        assert!(store.update("test", 7, "", 0, MarkerKind::Star).is_err());
        assert_eq!(store.all("test")[0].name, "Portal");
    }
}
