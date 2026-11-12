// Placed schematic previews: where each one stands in which world, kept across sessions, and the
// small transforms the on-screen editor makes (move with the camera, turn, show or hide).
pub type Pos = [i32; 3];
/// One placed preview: the world it belongs to (`scope`), the file, where its origin stands and
/// how it is turned (quarter turns clockwise seen from above), and whether it is drawn.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Placement {
    pub scope: String,
    pub file: String,
    pub origin: Pos,
    pub rotation: u8,
    pub visible: bool,
}
/// At most this many previews are kept, over all worlds.
pub const MAX: usize = 64;
fn clean(text: &str) -> String {
    text.chars()
        .filter(|c| !matches!(c, '\t' | '\n' | '\r'))
        .collect()
}
/// One line per preview: scope, file, x, y, z, rotation, shown (1 or 0), tab separated.
pub fn encode(list: &[Placement]) -> String {
    list.iter()
        .take(MAX)
        .map(|p| {
            format!(
                "{}\t{}\t{}\t{}\t{}\t{}\t{}",
                clean(&p.scope),
                clean(&p.file),
                p.origin[0],
                p.origin[1],
                p.origin[2],
                p.rotation % 4,
                u8::from(p.visible)
            )
        })
        .collect::<Vec<_>>()
        .join("\n")
}
/// The previews kept by `encode`; broken lines are left out.
pub fn decode(text: &str) -> Vec<Placement> {
    text.lines()
        .filter_map(|line| {
            let parts: Vec<&str> = line.split('\t').collect();
            let [scope, file, x, y, z, rotation, visible] = parts[..] else {
                return None;
            };
            Some(Placement {
                scope: scope.to_owned(),
                file: file.to_owned(),
                origin: [x.parse().ok()?, y.parse().ok()?, z.parse().ok()?],
                rotation: rotation.parse::<u8>().ok()? % 4,
                visible: visible == "1",
            })
        })
        .filter(|p| !p.file.is_empty())
        .take(MAX)
        .collect()
}
/// A world kept by an older version, `server#account#dimension#generation` (or
/// `save#dimension#generation` in single player), as it is kept now: `server#dimension`
/// (`save#dimension`). The generation counted up with every resource reload, so previews kept
/// under it were not found again after a restart. A world already kept the new way is unchanged.
pub fn stable_world(scope: &str) -> String {
    let parts: Vec<&str> = scope.rsplitn(4, '#').collect();
    let numeric = |s: &str| !s.is_empty() && s.bytes().all(|b| b.is_ascii_digit());
    match parts[..] {
        // generation, dimension, account (32 hex digits), server
        [generation, dimension, account, server]
            if numeric(generation)
                && account.len() == 32
                && account.bytes().all(|b| b.is_ascii_hexdigit()) =>
        {
            format!("{server}#{dimension}")
        }
        [generation, dimension, ..] if numeric(generation) && parts.len() >= 3 => {
            let save = scope.rsplitn(3, '#').nth(2).unwrap_or_default();
            format!("{save}#{dimension}")
        }
        _ => scope.to_owned(),
    }
}
/// Where a block of the file (relative to its origin) stands for a preview, turned as the
/// builder turns it (`Blueprint::transformed`).
pub fn place(relative: Pos, origin: Pos, rotation: u8) -> Pos {
    let mut at = relative;
    for _ in 0..rotation % 4 {
        at = [-at[2], at[1], at[0]];
    }
    [
        at[0].saturating_add(origin[0]),
        at[1].saturating_add(origin[1]),
        at[2].saturating_add(origin[2]),
    ]
}
/// The block of the file (relative to its origin) that stands at `world` for a preview: the
/// inverse of `place`.
pub fn unplace(world: Pos, origin: Pos, rotation: u8) -> Pos {
    let mut at = [
        world[0].wrapping_sub(origin[0]),
        world[1].wrapping_sub(origin[1]),
        world[2].wrapping_sub(origin[2]),
    ];
    for _ in 0..rotation % 4 {
        at = [at[2], at[1], -at[0]];
    }
    at
}
/// The corners of a preview: the box of the file's blocks, placed.
pub fn bounds(min: Pos, max: Pos, origin: Pos, rotation: u8) -> (Pos, Pos) {
    let a = place(min, origin, rotation);
    let b = place(max, origin, rotation);
    (
        [a[0].min(b[0]), a[1].min(b[1]), a[2].min(b[2])],
        [a[0].max(b[0]), a[1].max(b[1]), a[2].max(b[2])],
    )
}
/// The origin moved with the camera: `forward` along the way the player looks (snapped to the
/// nearest axis), `right` across it, `up` straight up.
pub fn nudge(origin: Pos, yaw: f32, forward: i32, right: i32, up: i32) -> Pos {
    // Facing south (yaw 0) is +z; west is yaw 90; north 180; east -90.
    let quarter = ((yaw.rem_euclid(360.0) + 45.0) / 90.0).floor() as i32 % 4;
    let (fx, fz) = match quarter {
        0 => (0, 1),
        1 => (-1, 0),
        2 => (0, -1),
        _ => (1, 0),
    };
    // To the right of facing south is west (-x).
    let (rx, rz) = (-fz, fx);
    [
        origin[0] + fx * forward + rx * right,
        origin[1] + up,
        origin[2] + fz * forward + rz * right,
    ]
}
/// Which layers of a preview are drawn, each player's own choice: all of them, only one, or
/// all up to one (to see inside a build while placing it). Layers count up from the bottom of
/// each preview (layer 0 is its lowest block), so one choice fits every shown preview.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum LayerMode {
    #[default]
    All,
    Single,
    Below,
}
impl LayerMode {
    pub fn next(self) -> Self {
        match self {
            Self::All => Self::Single,
            Self::Single => Self::Below,
            Self::Below => Self::All,
        }
    }
    /// Whether the layer `layer` (counted from the preview's bottom) is drawn.
    pub fn shows(self, chosen: i32, layer: i32) -> bool {
        match self {
            Self::All => true,
            Self::Single => layer == chosen,
            Self::Below => layer <= chosen,
        }
    }
    /// The setting as kept: `all`, `single:5`, `below:5`.
    pub fn encode(self, layer: i32) -> String {
        match self {
            Self::All => "all".into(),
            Self::Single => format!("single:{layer}"),
            Self::Below => format!("below:{layer}"),
        }
    }
    /// The kept setting read back; anything unknown is every layer. The layer stays within a
    /// preview's height (`MAX_LAYER`).
    pub fn decode(text: &str) -> (Self, i32) {
        let (mode, layer) = text.trim().split_once(':').unwrap_or((text.trim(), "0"));
        let layer = layer.parse::<i32>().unwrap_or(0).clamp(0, MAX_LAYER);
        match mode {
            "single" => (Self::Single, layer),
            "below" => (Self::Below, layer),
            _ => (Self::All, layer),
        }
    }
}
/// The highest layer a preview can have (a schematic is at most 256 blocks to a side).
pub const MAX_LAYER: i32 = 255;
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn worlds_kept_by_older_versions_read_as_now() {
        let account = "0123456789abcdef0123456789abcdef";
        assert_eq!(
            stable_world(&format!(
                "play.example:25565#{account}#minecraft:overworld#7"
            )),
            "play.example:25565#minecraft:overworld"
        );
        assert_eq!(
            stable_world("/home/me/saves/My#World#minecraft:the_nether#3"),
            "/home/me/saves/My#World#minecraft:the_nether"
        );
        for now in [
            "play.example:25565#minecraft:overworld",
            "/home/me/saves/World#minecraft:overworld",
        ] {
            assert_eq!(stable_world(now), now);
        }
    }
    #[test]
    fn unplace_finds_the_files_block_for_every_turn() {
        let origin = [100, 64, -20];
        for rotation in 0..4 {
            for rel in [[0, 0, 0], [3, 1, -2], [-5, 7, 9]] {
                let world = place(rel, origin, rotation);
                assert_eq!(unplace(world, origin, rotation), rel, "turn {rotation}");
            }
        }
    }
    #[test]
    fn previews_are_kept_and_read_back() {
        let list = vec![
            Placement {
                scope: "play.example\toverworld".into(),
                file: "house.litematic".into(),
                origin: [100, 64, -20],
                rotation: 3,
                visible: true,
            },
            Placement {
                scope: "other".into(),
                file: "tower.schem".into(),
                origin: [-5, 70, 8],
                rotation: 0,
                visible: false,
            },
        ];
        let back = decode(&encode(&list));
        assert_eq!(back.len(), 2);
        // A tab in a name cannot break the line.
        assert_eq!(back[0].scope, "play.exampleoverworld");
        assert_eq!(back[0].origin, [100, 64, -20]);
        assert_eq!(back[0].rotation, 3);
        assert!(back[0].visible && !back[1].visible);
        assert_eq!(back[1], list[1]);
        assert!(decode("broken\tline\n\n").is_empty());
    }
    #[test]
    fn placing_turns_like_the_builder_and_moves_with_the_camera() {
        // A quarter turn clockwise seen from above: east (+x) goes to south (+z).
        assert_eq!(place([1, 0, 0], [0, 0, 0], 1), [0, 0, 1]);
        assert_eq!(place([1, 2, 0], [10, 60, 10], 2), [9, 62, 10]);
        assert_eq!(
            bounds([0, 0, 0], [3, 1, 1], [0, 0, 0], 1),
            ([-1, 0, 0], [0, 1, 3])
        );
        // Looking south, forward is +z and right is -x; looking east, forward is +x.
        assert_eq!(nudge([0, 0, 0], 0.0, 1, 0, 0), [0, 0, 1]);
        assert_eq!(nudge([0, 0, 0], 0.0, 0, 1, 0), [-1, 0, 0]);
        assert_eq!(nudge([0, 0, 0], -90.0, 1, 0, 0), [1, 0, 0]);
        assert_eq!(nudge([0, 0, 0], 179.0, 1, 0, -1), [0, -1, -1]);
    }
    #[test]
    fn layers_show_all_one_or_up_to_one_and_read_back() {
        assert!(LayerMode::All.shows(3, 40));
        assert!(LayerMode::Single.shows(3, 3) && !LayerMode::Single.shows(3, 2));
        assert!(LayerMode::Below.shows(3, 0) && LayerMode::Below.shows(3, 3));
        assert!(!LayerMode::Below.shows(3, 4));
        for (mode, layer) in [
            (LayerMode::All, 0),
            (LayerMode::Single, 7),
            (LayerMode::Below, 12),
        ] {
            assert_eq!(LayerMode::decode(&mode.encode(layer)), (mode, layer));
        }
        assert_eq!(LayerMode::decode("single:-4"), (LayerMode::Single, 0));
        assert_eq!(
            LayerMode::decode("below:9999"),
            (LayerMode::Below, MAX_LAYER)
        );
        assert_eq!(LayerMode::decode("junk"), (LayerMode::All, 0));
        assert_eq!(LayerMode::All.next().next().next(), LayerMode::All);
    }
}
