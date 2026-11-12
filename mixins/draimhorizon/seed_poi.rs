// Found-POI state and click handling, shared by tests and the native seed map. Each POI carries
// whether it is found, when that was set and by whom (empty: this player): the newer setting
// wins, so a friend's mark or unmark (`[DHF1]` frames over the relay) and the player's own merge
// the same way, and an unmark is kept (as not found) so that it reaches friends too.
use super::{claim_borders_map_history::hash, claim_borders_seed_map as catalog};
use serde_json::{Value, json};
use std::{
    collections::BTreeMap,
    fs::{self, OpenOptions},
    io::{Read, Write},
    path::{Path, PathBuf},
    sync::atomic::{AtomicU64, Ordering},
};
const MAX_POINTS: usize = 8192;
const MAX_BYTES: usize = 1_048_576;
/// The relay frames that carry a player's own found POIs to their friends.
pub const PREFIX: &str = "[DHF1] ";
/// POIs a frame carries at most (a frame stays well under the relay's 1 KiB).
const PER_FRAME: usize = 6;
/// How far ahead of this clock a friend's setting may be dated (clocks differ a little).
const CLOCK_SLACK: u64 = 5 * 60 * 1000;
/// Whether a POI is found, when that was set (Unix milliseconds) and by whom (a friend's name;
/// empty for this player).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Entry {
    pub found: bool,
    pub at: u64,
    pub by: String,
}
impl Entry {
    /// The order settings of one POI are merged in: the later one wins; at the same moment a
    /// mark over an unmark, then by name, so every client ends with the same.
    fn rank(&self) -> (u64, bool, &str) {
        (self.at, self.found, self.by.as_str())
    }
}
fn friend_name(n: &str) -> bool {
    !n.is_empty() && n.len() <= 16 && n.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'_')
}

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
    points: BTreeMap<Poi, Entry>,
    /// Counts up at every change (what is sent to friends is made again then).
    revision: u64,
}
impl Found {
    fn empty(scope: &str, seed: i64) -> Result<Self, String> {
        if scope.is_empty() || scope.len() > 1024 {
            return Err("invalid found-POI scope".into());
        }
        Ok(Self {
            scope: scope.into(),
            seed,
            points: BTreeMap::new(),
            revision: 0,
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
        // Schema 1 kept found POIs only (all the player's own); schema 2 adds the state, when it
        // was set and by whom.
        let schema = v["schema"].as_u64();
        if !matches!(schema, Some(1 | 2))
            || v["scope"].as_str() != Some(scope)
            || v["seed"].as_i64() != Some(seed)
        {
            return Err("found-POI version/identity mismatch".into());
        }
        let width = if schema == Some(1) { 4 } else { 7 };
        let points = v["points"]
            .as_array()
            .filter(|p| p.len() <= MAX_POINTS)
            .ok_or("invalid found-POI records")?;
        let mut found = Self::empty(scope, seed)?;
        for p in points {
            let p = p
                .as_array()
                .filter(|p| p.len() == width)
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
            let entry = if width == 4 {
                Entry {
                    found: true,
                    at: 1,
                    by: String::new(),
                }
            } else {
                let by = p[6].as_str().ok_or("invalid found-POI author")?;
                if !by.is_empty() && !friend_name(by) {
                    return Err("invalid found-POI author".into());
                }
                Entry {
                    found: p[4].as_u64().ok_or("invalid found-POI state")? == 1,
                    at: p[5]
                        .as_u64()
                        .filter(|t| *t > 0)
                        .ok_or("invalid found-POI time")?,
                    by: by.into(),
                }
            };
            if found.points.insert(poi, entry).is_some() {
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
        self.points.get(poi).is_some_and(|e| e.found)
    }
    /// The friend who marked a found POI (none: not found, or marked by this player).
    pub fn marked_by(&self, poi: &Poi) -> Option<&str> {
        self.points
            .get(poi)
            .filter(|e| e.found && !e.by.is_empty())
            .map(|e| e.by.as_str())
    }
    pub fn revision(&self) -> u64 {
        self.revision
    }
    /// Marks or unmarks a POI as this player, at `now`; true when it is found now.
    pub fn toggle(&mut self, poi: Poi, now: u64) -> Result<bool, String> {
        let found = !self.contains(&poi);
        // Never dated before what it replaces (a clock set back would lose the click).
        let at = self
            .points
            .get(&poi)
            .map_or(now, |e| now.max(e.at.saturating_add(1)))
            .max(1);
        self.merge(
            poi,
            Entry {
                found,
                at,
                by: String::new(),
            },
        )?;
        Ok(found)
    }
    /// Takes a setting of a POI if it is newer than the one kept; true when it changed.
    fn merge(&mut self, poi: Poi, entry: Entry) -> Result<bool, String> {
        match self.points.get(&poi) {
            Some(old) if old.rank() >= entry.rank() => return Ok(false),
            Some(_) => {}
            None if self.points.len() >= MAX_POINTS => {
                return Err("found-POI limit reached".into());
            }
            None if !entry.found => {
                // An unmark of a POI never marked here changes nothing.
                return Ok(false);
            }
            None => {}
        }
        self.points.insert(poi, entry);
        self.revision = self.revision.wrapping_add(1).max(1);
        Ok(true)
    }
    /// This player's own settings (marks and unmarks) for friends, a few POIs a frame:
    /// `[DHF1] [seed, [[dimension, kind, x, z, found, at], ...]]`. Friends' marks are not sent on.
    pub fn frames(&self) -> Vec<String> {
        self.points
            .iter()
            .filter(|(_, e)| e.by.is_empty() && e.at > 1)
            .collect::<Vec<_>>()
            .chunks(PER_FRAME)
            .map(|rows| {
                let rows: Vec<Value> = rows
                    .iter()
                    .map(|(p, e)| json!([p.dimension, p.id, p.x, p.z, u8::from(e.found), e.at]))
                    .collect();
                format!("{PREFIX}{}", json!([self.seed, rows]))
            })
            .collect()
    }
    /// A friend's frame (the friend named by the relay, never by the frame). The whole frame
    /// is checked before anything of it is taken; true when something changed.
    pub fn receive(&mut self, body: &str, friend: &str, now: u64) -> Result<bool, String> {
        if body.len() > 1024 || !friend_name(friend) {
            return Err("invalid found-POI frame".into());
        }
        let value: Value = serde_json::from_str(
            body.strip_prefix(PREFIX)
                .ok_or("invalid found-POI prefix")?,
        )
        .map_err(|_| "invalid found-POI JSON")?;
        let [seed, rows] = value.as_array().map(Vec::as_slice).unwrap_or_default() else {
            return Err("invalid found-POI frame".into());
        };
        // Another seed: the friend's map is of another world; nothing of it applies here.
        if seed.as_i64() != Some(self.seed) {
            return Ok(false);
        }
        let rows = rows
            .as_array()
            .filter(|r| !r.is_empty() && r.len() <= PER_FRAME)
            .ok_or("invalid found-POI rows")?;
        let mut parsed = Vec::new();
        for row in rows {
            let r = row
                .as_array()
                .filter(|r| r.len() == 6)
                .ok_or("invalid found-POI row")?;
            let int = |v: &Value| v.as_i64().and_then(|v| i32::try_from(v).ok());
            let poi = Poi::new(
                r[0].as_u64()
                    .and_then(|d| usize::try_from(d).ok())
                    .ok_or("invalid POI dimension")?,
                r[1].as_str().ok_or("invalid POI kind")?,
                [
                    f64::from(int(&r[2]).ok_or("invalid POI coordinate")?),
                    f64::from(int(&r[3]).ok_or("invalid POI coordinate")?),
                ],
            )
            .ok_or("invalid found POI")?;
            let found = match r[4].as_u64() {
                Some(0) => false,
                Some(1) => true,
                _ => return Err("invalid found-POI state".into()),
            };
            let at = r[5]
                .as_u64()
                .filter(|t| *t > 1 && *t <= now.saturating_add(CLOCK_SLACK))
                .ok_or("invalid found-POI time")?;
            parsed.push((
                poi,
                Entry {
                    found,
                    at,
                    by: friend.into(),
                },
            ));
        }
        let mut changed = false;
        for (poi, entry) in parsed {
            changed |= self.merge(poi, entry)?;
        }
        Ok(changed)
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
            .map(|(p, e)| json!([p.dimension, p.id, p.x, p.z, u8::from(e.found), e.at, e.by]))
            .collect();
        let bytes =
            json!({"schema":2,"scope":self.scope,"seed":self.seed,"points":points}).to_string();
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

pub struct Cluster<'a> {
    pub count: usize,
    pub found_count: usize,
    pub at: [f32; 2],
    pub mark: &'a catalog::Mark,
}
/// Cluster only equal POI types in world-aligned cells, independent of pan/order.
pub fn clusters<'a>(
    marks: &'a [catalog::Mark],
    dimension: usize,
    view: super::claim_borders_map::View,
    rect: [f32; 4],
    preferences: &catalog::Preferences,
    found: Option<&Found>,
) -> Vec<Cluster<'a>> {
    let mut groups = std::collections::BTreeMap::<(i64, i64, usize), Cluster<'a>>::new();
    if !rect.iter().all(|v| v.is_finite()) || rect[2] <= 0.0 || rect[3] <= 0.0 {
        return Vec::new();
    }
    let cell = 22.0 * 128.0 * f64::from(view.blocks_per_pixel) / f64::from(rect[3]);
    if !cell.is_finite() || cell <= 0.0 {
        return Vec::new();
    }
    for mark in marks.iter().filter(|m| preferences.shows(&m.id)) {
        let Some(poi) = Poi::new(dimension, &mark.id, mark.position) else {
            continue;
        };
        let Some(kind) = catalog::structure_index(&mark.id) else {
            continue;
        };
        let xy = view.pixel_aspect(mark.position, 128.0, rect[2] / rect[3]);
        let at = [
            rect[0] + xy[0] * rect[2] / 128.0,
            rect[1] + xy[1] * rect[3] / 128.0,
        ];
        if !catalog::contains(rect, at) {
            continue;
        }
        let key = (
            (mark.position[0] / cell).floor() as i64,
            (mark.position[1] / cell).floor() as i64,
            kind,
        );
        let is_found = usize::from(found.is_some_and(|f| f.contains(&poi)));
        groups
            .entry(key)
            .and_modify(|c| {
                c.count += 1;
                c.found_count += is_found;
                if mark.position[0] < c.mark.position[0]
                    || mark.position[0] == c.mark.position[0]
                        && mark.position[1] < c.mark.position[1]
                {
                    c.mark = mark;
                    c.at = at;
                }
            })
            .or_insert(Cluster {
                count: 1,
                found_count: is_found,
                at,
                mark,
            });
    }
    let mut groups: Vec<_> = groups.into_values().collect();
    // Dense clusters go underneath rare/individual POIs, not over them.
    groups.sort_by(|a, b| {
        b.count
            .cmp(&a.count)
            .then(a.mark.id.cmp(&b.mark.id))
            .then(a.mark.position[0].total_cmp(&b.mark.position[0]))
            .then(a.mark.position[1].total_cmp(&b.mark.position[1]))
    });
    groups
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
    fn clustering_keeps_poi_types_separate_and_is_stable_while_panning() {
        use super::super::claim_borders_map::View;
        let marks = vec![
            catalog::Mark {
                id: "village_plains".into(),
                position: [8.0, 8.0],
            },
            catalog::Mark {
                id: "mineshaft".into(),
                position: [10.0, 8.0],
            },
            catalog::Mark {
                id: "village_plains".into(),
                position: [12.0, 8.0],
            },
        ];
        let view = View {
            center: [0.0; 2],
            blocks_per_pixel: 64.0,
        };
        let rect = [0.0, 0.0, 1000.0, 400.0];
        let p = catalog::Preferences::default();
        let clusters = clusters(&marks, 0, view, rect, &p, None);
        assert_eq!(clusters.len(), 2);
        assert_eq!(clusters.iter().map(|c| c.count).sum::<usize>(), 3);
        assert!(
            clusters
                .iter()
                .any(|c| c.mark.id == "village_plains" && c.count == 2)
        );
        assert!(
            clusters
                .iter()
                .any(|c| c.mark.id == "mineshaft" && c.count == 1)
        );
        let shifted = super::clusters(
            &marks,
            0,
            View {
                center: [100.0, -80.0],
                ..view
            },
            rect,
            &p,
            None,
        );
        let reversed: Vec<_> = marks.iter().cloned().rev().collect();
        let reversed = super::clusters(&reversed, 0, view, rect, &p, None);
        for other in [&shifted, &reversed] {
            assert_eq!(
                clusters
                    .iter()
                    .map(|c| (&c.mark.id, c.mark.position, c.count))
                    .collect::<Vec<_>>(),
                other
                    .iter()
                    .map(|c| (&c.mark.id, c.mark.position, c.count))
                    .collect::<Vec<_>>()
            );
        }
    }
    #[test]
    fn found_pois_toggle_and_survive_restart_without_crossing_scopes_or_seeds() {
        let dir = Dir::new();
        let mut found = Found::load(&dir.0, "server#account", 42).unwrap();
        assert!(!found.contains(&key()));
        assert!(found.toggle(key(), 1000).unwrap());
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
        assert!(!restored.toggle(key(), 1000).unwrap());
        restored.save(&dir.0).unwrap();
        assert!(
            !Found::load(&dir.0, "server#account", 42)
                .unwrap()
                .contains(&key())
        );
    }
    #[test]
    fn a_mark_reaches_friends_and_the_newer_setting_wins() {
        let now = 1_700_000_000_000;
        let mut alex = Found::empty("server#alex", 42).unwrap();
        let mut sam = Found::empty("server#sam", 42).unwrap();
        assert!(alex.toggle(key(), now).unwrap());
        let frames = alex.frames();
        assert_eq!(frames.len(), 1);
        assert!(frames[0].starts_with(PREFIX));
        assert!(sam.receive(&frames[0], "Alex", now).unwrap());
        assert!(sam.contains(&key()));
        assert_eq!(sam.marked_by(&key()), Some("Alex"));
        assert_eq!(alex.marked_by(&key()), None);
        // A friend's mark is not sent on to others.
        assert!(sam.frames().is_empty());
        // The same frame again changes nothing.
        assert!(!sam.receive(&frames[0], "Alex", now).unwrap());
        // Sam unmarks it later: Alex takes that, and an older frame does not bring it back.
        assert!(!sam.toggle(key(), now + 10).unwrap());
        assert!(!sam.contains(&key()));
        let unmark = sam.frames()[0].clone();
        assert!(alex.receive(&unmark, "Sam", now + 20).unwrap());
        assert!(!alex.contains(&key()));
        assert!(!alex.receive(&frames[0], "Alex", now + 20).unwrap());
        assert!(!alex.contains(&key()));
        // Marked again by Alex: newer than Sam's unmark everywhere.
        assert!(alex.toggle(key(), now + 30).unwrap());
        assert!(sam.receive(&alex.frames()[0], "Alex", now + 30).unwrap());
        assert!(sam.contains(&key()));
        // Sam's own setting was replaced by Alex's newer one: Sam has nothing of his to send.
        assert!(sam.frames().is_empty());
        // An unmark of a POI never marked here is not kept.
        let mut fresh = Found::empty("server#kim", 42).unwrap();
        assert!(!fresh.receive(&unmark, "Sam", now + 40).unwrap());
        assert!(fresh.points.is_empty());
    }
    #[test]
    fn friend_frames_are_checked_whole_and_other_seeds_are_ignored() {
        let now = 1_700_000_000_000;
        let mut found = Found::empty("scope", 42).unwrap();
        let row = |state: &str, at: u64| format!(r#"[0,"village_plains",-120,56,{state},{at}]"#);
        let good = row("1", now);
        assert!(
            !found
                .receive(&format!("{PREFIX}[43,[{good}]]"), "Alex", now)
                .unwrap()
        );
        assert!(found.points.is_empty());
        for bad in [
            format!("[DHO1] [42,[{good}]]"),
            format!("{PREFIX}[42,[]]"),
            format!("{PREFIX}[42]"),
            format!("{PREFIX}[42,[{good},{}]]", row("2", now)),
            format!("{PREFIX}[42,[{good},{}]]", row("1", now + 10 * 60 * 1000)),
            format!("{PREFIX}[42,[{good},{}]]", row("1", 0)),
            format!(r#"{PREFIX}[42,[{good},[0,"fortress",0,0,1,{now}]]]"#),
            format!(r#"{PREFIX}[42,[{good},[0,"village_plains",0,0,1]]]"#),
            format!(
                "{PREFIX}[42,[{}]]",
                vec![good.clone(); PER_FRAME + 1].join(",")
            ),
        ] {
            assert!(found.receive(&bad, "Alex", now).is_err(), "{bad}");
            assert!(found.points.is_empty(), "{bad}");
        }
        for name in ["", "Bad Name", "a_very_long_name_x"] {
            assert!(
                found
                    .receive(&format!("{PREFIX}[42,[{good}]]"), name, now)
                    .is_err()
            );
        }
        assert!(
            found
                .receive(&format!("{PREFIX}[42,[{good}]]"), "Alex", now)
                .unwrap()
        );
    }
    #[test]
    fn older_saves_load_as_own_marks_and_friends_marks_are_kept() {
        let dir = Dir::new();
        let mut found = Found::load(&dir.0, "scope", 42).unwrap();
        found.toggle(key(), 1000).unwrap();
        found.save(&dir.0).unwrap();
        let path = found.path(&dir.0);
        // A save of the first version: found POIs only.
        fs::write(
            &path,
            br#"{"schema":1,"scope":"scope","seed":42,"points":[[0,"village_plains",-120,56]]}"#,
        )
        .unwrap();
        let mut old = Found::load(&dir.0, "scope", 42).unwrap();
        assert!(old.contains(&key()) && old.marked_by(&key()).is_none());
        // Never sent as a new mark (its time is unknown), but unmarking it is.
        assert!(old.frames().is_empty());
        let other = Poi::new(0, "village_desert", [400.0, -32.0]).unwrap();
        let mut friend = Found::empty("scope#friend", 42).unwrap();
        friend.toggle(other.clone(), 1_700_000_000_000).unwrap();
        old.receive(&friend.frames()[0], "Sam", 1_700_000_000_000)
            .unwrap();
        old.save(&dir.0).unwrap();
        let back = Found::load(&dir.0, "scope", 42).unwrap();
        assert!(back.contains(&key()));
        assert_eq!(back.marked_by(&other), Some("Sam"));
        assert!(!old.toggle(key(), 2000).unwrap());
        assert_eq!(old.frames().len(), 1);
    }
    #[test]
    fn a_full_frame_stays_under_the_relay_size() {
        let mut found = Found::empty("scope", -9_223_372_036_854_775_807).unwrap();
        let id = catalog::STRUCTURES
            .iter()
            .filter(|d| d.group.dimension() == 0)
            .max_by_key(|d| d.id.len())
            .unwrap()
            .id;
        for i in 0..PER_FRAME {
            let poi = Poi::new(0, id, [-29_999_999.0 + i as f64, -29_999_999.0]).unwrap();
            found.toggle(poi, u64::MAX / 2).unwrap();
        }
        let frames = found.frames();
        assert_eq!(frames.len(), 1);
        assert!(frames[0].len() <= 1024, "{}", frames[0].len());
    }
    #[test]
    fn corrupt_or_wrong_identity_data_is_preserved_instead_of_overwritten() {
        let dir = Dir::new();
        let mut found = Found::load(&dir.0, "scope", 42).unwrap();
        found.toggle(key(), 1000).unwrap();
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
        found.toggle(key(), 1000).unwrap();
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
                .toggle(
                    Poi::new(0, "village_plains", [x as f64, 0.0]).unwrap(),
                    1000,
                )
                .unwrap();
        }
        assert!(
            found
                .toggle(
                    Poi::new(0, "village_plains", [MAX_POINTS as f64, 0.0]).unwrap(),
                    1000
                )
                .is_err()
        );
        assert!(
            !found
                .toggle(Poi::new(0, "village_plains", [0.0, 0.0]).unwrap(), 1000)
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
        found.toggle(key(), 1000).unwrap();
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
