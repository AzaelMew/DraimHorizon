// Pure settings, persistence and cave-visible outer claim cages.
use serde_json::{Value, json};
use std::collections::BTreeSet;
use std::fs::{self, File, OpenOptions};
use std::io::{Read, Write};
use std::path::Path;
use std::sync::atomic::{AtomicU64, Ordering};

const MAX_FILE_BYTES: u64 = 16_384;
const MAX_CHUNKS: usize = 4096;
const MAX_LINES: usize = 32_768;
pub type Segment = ([f64; 3], [f64; 3]);

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Span {
    Near16,
    Near32,
    Near64,
    FullHeight,
}
impl Span {
    pub fn name(self) -> &'static str {
        match self {
            Self::Near16 => "16",
            Self::Near32 => "32",
            Self::Near64 => "64",
            Self::FullHeight => "full",
        }
    }
    fn parse(value: &str) -> Result<Self, String> {
        match value {
            "16" => Ok(Self::Near16),
            "32" => Ok(Self::Near32),
            "64" => Ok(Self::Near64),
            "full" => Ok(Self::FullHeight),
            _ => Err("Span must be 16, 32, 64 or full.".into()),
        }
    }
    pub fn bounds(self, feet: f64) -> (f64, f64) {
        let radius = match self {
            Self::Near16 => 16.0,
            Self::Near32 => 32.0,
            Self::Near64 => 64.0,
            Self::FullHeight => return (-64.0, 320.0),
        };
        (feet.floor() - radius + 0.05, feet.floor() + radius + 0.05)
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct Settings {
    pub own_visible: bool,
    pub friends_visible: bool,
    pub own_color: u32,
    pub friend_color: u32,
    pub through_walls: bool,
    pub opacity: f32,
    pub line_width: f32,
    pub distance: u16,
    pub span: Span,
}
impl Default for Settings {
    fn default() -> Self {
        Self {
            own_visible: true,
            friends_visible: true,
            own_color: 0x55FF88,
            friend_color: 0x55BBFF,
            through_walls: true,
            opacity: 0.45,
            line_width: 2.0,
            distance: 160,
            span: Span::Near32,
        }
    }
}
impl Settings {
    fn validate(&self) -> Result<(), String> {
        if self.own_color > 0xFFFFFF
            || self.friend_color > 0xFFFFFF
            || !self.opacity.is_finite()
            || !(0.0..=1.0).contains(&self.opacity)
            || !self.line_width.is_finite()
            || !(1.0..=6.0).contains(&self.line_width)
            || ![64, 160, 256, 512].contains(&self.distance)
        {
            return Err("Invalid claim visualization settings.".into());
        }
        Ok(())
    }
    /// Change one action atomically; callers save the candidate before replacing live settings.
    pub fn set(&mut self, key: &str, value: &str) -> Result<(), String> {
        let mut next = self.clone();
        let boolean = || match value {
            "on" | "true" => Ok(true),
            "off" | "false" => Ok(false),
            _ => Err("Use on or off.".to_owned()),
        };
        match key {
            "own" => next.own_visible = boolean()?,
            "friends" => next.friends_visible = boolean()?,
            "throughwalls" | "through_walls" => next.through_walls = boolean()?,
            "owncolor" | "own_color" => next.own_color = parse_color(value)?,
            "friendcolor" | "friend_color" => next.friend_color = parse_color(value)?,
            "opacity" => {
                next.opacity = value
                    .parse()
                    .map_err(|_| "Opacity must be between 0 and 1.")?
            }
            "width" | "line_width" => {
                next.line_width = value
                    .parse()
                    .map_err(|_| "Width must be between 1 and 6.")?
            }
            "distance" => {
                next.distance = value
                    .parse()
                    .map_err(|_| "Distance must be 64, 160, 256 or 512.")?
            }
            "span" => next.span = Span::parse(value)?,
            _ => return Err("Unknown visualization setting.".into()),
        }
        next.validate()?;
        *self = next;
        Ok(())
    }
    pub fn styled_color(&self, friend: bool) -> u32 {
        let alpha = if self.opacity.is_finite() {
            self.opacity.clamp(0.0, 1.0)
        } else {
            0.0
        };
        ((alpha * 255.0).round() as u32) << 24
            | (if friend {
                self.friend_color
            } else {
                self.own_color
            } & 0xFFFFFF)
    }
    pub fn load(path: &Path) -> Result<Self, String> {
        let metadata = match fs::symlink_metadata(path) {
            Ok(value) => value,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                return Ok(Self::default());
            }
            Err(error) => return Err(error.to_string()),
        };
        if !metadata.is_file() || metadata.len() > MAX_FILE_BYTES {
            return Err("Visualization settings must be a small regular file.".into());
        }
        let mut bytes = Vec::new();
        File::open(path)
            .map_err(|e| e.to_string())?
            .take(MAX_FILE_BYTES + 1)
            .read_to_end(&mut bytes)
            .map_err(|e| e.to_string())?;
        if bytes.len() as u64 > MAX_FILE_BYTES {
            return Err("Visualization settings are too large.".into());
        }
        Self::decode(&bytes)
    }
    fn decode(bytes: &[u8]) -> Result<Self, String> {
        let v: Value = serde_json::from_slice(bytes).map_err(|e| e.to_string())?;
        let invalid = || "Invalid visualization settings file.".to_owned();
        if v["schema"].as_u64() != Some(1) || v.as_object().map(|v| v.len()) != Some(10) {
            return Err(invalid());
        }
        let color = |key: &str| v[key].as_str().ok_or_else(invalid).and_then(parse_color);
        let value = Self {
            own_visible: v["own_visible"].as_bool().ok_or_else(invalid)?,
            friends_visible: v["friends_visible"].as_bool().ok_or_else(invalid)?,
            own_color: color("own_color")?,
            friend_color: color("friend_color")?,
            through_walls: v["through_walls"].as_bool().ok_or_else(invalid)?,
            opacity: v["opacity"].as_f64().ok_or_else(invalid)? as f32,
            line_width: v["line_width"].as_f64().ok_or_else(invalid)? as f32,
            distance: v["distance"]
                .as_u64()
                .and_then(|n| u16::try_from(n).ok())
                .ok_or_else(invalid)?,
            span: Span::parse(v["span"].as_str().ok_or_else(invalid)?)?,
        };
        value.validate()?;
        Ok(value)
    }
    pub fn save(&self, path: &Path) -> Result<(), String> {
        self.validate()?;
        // Refuse to replace a corrupt, unknown-schema or symlink file.
        Self::load(path)?;
        let bytes = serde_json::to_vec_pretty(&json!({"schema":1,"own_visible":self.own_visible,"friends_visible":self.friends_visible,"own_color":format!("#{:06X}",self.own_color),"friend_color":format!("#{:06X}",self.friend_color),"through_walls":self.through_walls,"opacity":self.opacity,"line_width":self.line_width,"distance":self.distance,"span":self.span.name()})).map_err(|e|e.to_string())?;
        let parent = path
            .parent()
            .filter(|p| !p.as_os_str().is_empty())
            .unwrap_or(Path::new("."));
        fs::create_dir_all(parent).map_err(|e| e.to_string())?;
        static SERIAL: AtomicU64 = AtomicU64::new(0);
        let filename = path.file_name().ok_or("Settings file needs a name.")?;
        let temporary = parent.join(format!(
            ".{}.{}.{}.tmp",
            filename.to_string_lossy(),
            std::process::id(),
            SERIAL.fetch_add(1, Ordering::Relaxed)
        ));
        let mut file = OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&temporary)
            .map_err(|e| e.to_string())?;
        let result = (|| -> std::io::Result<()> {
            file.write_all(&bytes)?;
            file.sync_all()?;
            drop(file);
            fs::rename(&temporary, path)?;
            Ok(())
        })();
        if result.is_err() {
            let _ = fs::remove_file(&temporary);
        }
        result.map_err(|e| e.to_string())
    }
}
fn parse_color(value: &str) -> Result<u32, String> {
    let hex = value.strip_prefix('#').unwrap_or(value);
    if hex.len() != 6 || !hex.bytes().all(|b| b.is_ascii_hexdigit()) {
        return Err("Color must be #RRGGBB.".into());
    }
    u32::from_str_radix(hex, 16).map_err(|_| "Color must be #RRGGBB.".into())
}

/// Dimension filtering belongs to the adapter. Compute the perimeter before distance
/// culling so adjacent chunks never acquire false walls at the visibility boundary.
#[cfg(test)]
pub fn geometry(
    chunks: impl IntoIterator<Item = (i32, i32)>,
    player: [f64; 3],
    settings: &Settings,
) -> Vec<Segment> {
    geometry_with_spacing(chunks, player, settings, 8)
}
pub fn geometry_with_spacing(
    chunks: impl IntoIterator<Item = (i32, i32)>,
    player: [f64; 3],
    settings: &Settings,
    spacing: u8,
) -> Vec<Segment> {
    if !player.iter().all(|v| v.is_finite()) || settings.validate().is_err() {
        return Vec::new();
    }
    let chunks: BTreeSet<_> = chunks
        .into_iter()
        .take(MAX_CHUNKS)
        .filter(|(x, z)| {
            x.rem_euclid(16) == 0
                && z.rem_euclid(16) == 0
                && (-30_000_000..=29_999_984).contains(x)
                && (-30_000_000..=29_999_984).contains(z)
        })
        .collect();
    let (bottom, top) = settings.span.bounds(player[1]);
    let mut heights = vec![bottom];
    let mut y = bottom + f64::from(spacing);
    while spacing > 0 && y < top {
        heights.push(y);
        y += f64::from(spacing);
    }
    heights.push(top);
    let mut lines = Vec::new();
    let mut posts = BTreeSet::new();
    for &(x, z) in &chunks {
        if (player[0] - player[0].clamp(f64::from(x), f64::from(x + 16)))
            .hypot(player[2] - player[2].clamp(f64::from(z), f64::from(z + 16)))
            > f64::from(settings.distance)
        {
            continue;
        }
        for (neighbor, a, b) in [
            ((x - 16, z), (x, z), (x, z + 16)),
            ((x + 16, z), (x + 16, z), (x + 16, z + 16)),
            ((x, z - 16), (x, z), (x + 16, z)),
            ((x, z + 16), (x, z + 16), (x + 16, z + 16)),
        ] {
            if chunks.contains(&neighbor) {
                continue;
            }
            // Reserve room for the posts of every accepted edge.
            if lines.len() + heights.len() + posts.len() + 2 > MAX_LINES {
                continue;
            }
            for &y in &heights {
                lines.push((
                    [f64::from(a.0), y, f64::from(a.1)],
                    [f64::from(b.0), y, f64::from(b.1)],
                ));
            }
            posts.insert(a);
            posts.insert(b);
        }
    }
    for (x, z) in posts {
        lines.push((
            [f64::from(x), bottom, f64::from(z)],
            [f64::from(x), top, f64::from(z)],
        ));
    }
    lines
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn negative_cave_cage_follows_feet_and_has_no_internal_wall() {
        let lines = geometry(
            [(-32, -16), (-16, -16)],
            [-10.0, -22.3, -8.0],
            &Settings::default(),
        );
        assert!(!lines.is_empty());
        assert!(
            !lines
                .iter()
                .any(|(a, b)| a[0] == -16.0 && b[0] == -16.0 && a[2] != b[2])
        );
        assert!(lines.iter().any(|(a, b)| a[1] == -54.95 && b[1] == 9.05));
        assert!(
            lines
                .iter()
                .any(|(a, b)| (a[1] + 22.95).abs() < 1e-9 && (b[1] + 22.95).abs() < 1e-9)
        );
        assert!(
            lines
                .iter()
                .flat_map(|(a, b)| [a, b])
                .all(|p| (-32.0..=0.0).contains(&p[0]) && (-16.0..=0.0).contains(&p[2]))
        );
    }
    #[test]
    fn distance_uses_chunk_box_and_does_not_create_internal_walls() {
        let settings = Settings {
            distance: 64,
            ..Settings::default()
        };
        assert!(!geometry([(0, 0)], [80.0, 20.0, 8.0], &settings).is_empty());
        assert!(geometry([(0, 0)], [80.01, 20.0, 8.0], &settings).is_empty());
        assert!(geometry([(0, 0)], [f64::NAN, 0.0, 0.0], &settings).is_empty());
        let lines = geometry([(0, 0), (16, 0)], [96.0, 20.0, 8.0], &settings);
        assert!(
            !lines
                .iter()
                .any(|(a, b)| a[0] == 16.0 && b[0] == 16.0 && a[2] != b[2])
        );
    }
    #[test]
    fn span_and_output_are_bounded() {
        let settings = Settings {
            span: Span::FullHeight,
            distance: 512,
            ..Settings::default()
        };
        let lines = geometry(
            (0..4096).map(|i| ((i % 64) * 32, (i / 64) * 32)),
            [256.0, 10.0, 256.0],
            &settings,
        );
        assert!(lines.len() <= MAX_LINES);
        assert!(
            lines
                .iter()
                .flat_map(|(a, b)| [a, b])
                .all(|p| (-64.0..=320.0).contains(&p[1]))
        );
        assert_eq!(Span::Near16.bounds(10.9), (-5.95, 26.05));
    }
    #[test]
    fn setters_reject_invalid_values_without_mutation_and_alpha_clamps() {
        let mut settings = Settings::default();
        let before = settings.clone();
        for (key, value) in [
            ("opacity", "NaN"),
            ("opacity", "2"),
            ("width", "0"),
            ("distance", "999"),
            ("owncolor", "#FFFFFFFF"),
            ("span", "128"),
        ] {
            assert!(settings.set(key, value).is_err());
            assert_eq!(settings, before);
        }
        settings.opacity = 2.0;
        assert_eq!(settings.styled_color(false), 0xFF55FF88);
        settings.opacity = -1.0;
        assert_eq!(settings.styled_color(true), 0x0055BBFF);
    }
    #[test]
    fn persisted_settings_roundtrip_and_corruption_is_preserved() {
        static SERIAL: AtomicU64 = AtomicU64::new(0);
        let dir = std::env::temp_dir().join(format!(
            "claim-visual-test-{}-{}",
            std::process::id(),
            SERIAL.fetch_add(1, Ordering::Relaxed)
        ));
        let path = dir.join("visualization.json");
        let mut settings = Settings::load(&path).unwrap();
        for (key, value) in [
            ("own", "off"),
            ("friends", "off"),
            ("owncolor", "#112233"),
            ("friendcolor", "ABCDEF"),
            ("opacity", "0.6"),
            ("width", "3.5"),
            ("distance", "512"),
            ("span", "full"),
            ("throughwalls", "off"),
        ] {
            settings.set(key, value).unwrap();
        }
        settings.save(&path).unwrap();
        assert_eq!(Settings::load(&path).unwrap(), settings);
        fs::write(&path, b"corrupt").unwrap();
        assert!(settings.save(&path).is_err());
        assert_eq!(fs::read(&path).unwrap(), b"corrupt");
        fs::remove_dir_all(dir).unwrap();
    }
    #[test]
    fn sparse_grid_and_outline_preserve_outer_edges_with_fewer_lines() {
        let settings = Settings::default();
        let dense = geometry_with_spacing([(0, 0)], [8.0, 64.0, 8.0], &settings, 8);
        let sparse = geometry_with_spacing([(0, 0)], [8.0, 64.0, 8.0], &settings, 16);
        let outline = geometry_with_spacing([(0, 0)], [8.0, 64.0, 8.0], &settings, 0);
        assert!(dense.len() > sparse.len());
        assert!(sparse.len() > outline.len());
        assert_eq!(outline.len(), 12);
    }
}
