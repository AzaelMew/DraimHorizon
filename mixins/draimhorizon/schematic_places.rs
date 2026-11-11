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
#[cfg(test)]
mod tests {
    use super::*;
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
}
