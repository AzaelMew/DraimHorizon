// Farm plots: fields the player marks by hand (`/farm pos1`, `/farm pos2`, `/farm save`), kept
// per world across sessions. The farm works every enabled plot of the world it is in, wherever
// the player stands when it starts (after a reconnect too); without any, the square around the
// player as before. Pure data: the commands, drawing and menu are in the views.
pub type Pos = [i32; 3];
/// Most plots kept, over all worlds.
pub const MAX: usize = 64;
/// The largest side of a plot, in blocks, and the most levels it spans: the farm looks at
/// every cell of its enabled plots twice a second.
pub const MAX_SIDE: i32 = 64;
pub const MAX_HEIGHT: i32 = 12;
/// The most cells the enabled plots of a world may have together (a cell is a column of a level:
/// the farm reads each twice a second on the game thread). A flat 64×64 plot has 12 288.
pub const MAX_CELLS: i64 = 16_384;
/// A box of crop cells (where the crops grow, one above the farmland), corners included. The
/// farm looks for beds at every height from `lo[1]` to `hi[1]`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Bounds {
    pub lo: Pos,
    pub hi: Pos,
}
impl Bounds {
    /// The box between two corners marked by the player (each the crop cell they stood in),
    /// with a block of unevenness above and below.
    pub fn between(a: Pos, b: Pos) -> Self {
        Self {
            lo: [a[0].min(b[0]), a[1].min(b[1]) - 1, a[2].min(b[2])],
            hi: [a[0].max(b[0]), a[1].max(b[1]) + 1, a[2].max(b[2])],
        }
    }
    /// The square field around a start (the farm without plots): `radius` to each side and
    /// `reach_y` up and down.
    pub fn around(origin: Pos, radius: i32, reach_y: i32) -> Self {
        Self {
            lo: [origin[0] - radius, origin[1] - reach_y, origin[2] - radius],
            hi: [origin[0] + radius, origin[1] + reach_y, origin[2] + radius],
        }
    }
    #[cfg(test)]
    pub fn contains(&self, at: Pos) -> bool {
        (0..3).all(|i| self.lo[i] <= at[i] && at[i] <= self.hi[i])
    }
    /// The middle cell, at the height of the middle level.
    pub fn center(&self) -> Pos {
        [0, 1, 2].map(|i| self.lo[i] + (self.hi[i] - self.lo[i]) / 2)
    }
    /// Half the width and half the depth, rounded down (the radius of a square field): how far
    /// the field reaches from its middle.
    pub fn half(&self) -> (i32, i32) {
        (
            (self.hi[0] - self.lo[0] + 1) / 2,
            (self.hi[2] - self.lo[2] + 1) / 2,
        )
    }
    pub fn columns(&self) -> i64 {
        i64::from(self.hi[0] - self.lo[0] + 1) * i64::from(self.hi[2] - self.lo[2] + 1)
    }
    /// The cells the farm reads: every column at every height of the box.
    pub fn cells(&self) -> i64 {
        self.columns() * i64::from(self.hi[1] - self.lo[1] + 1)
    }
    /// Whether a point of the world is by the field: within `margin` blocks of it across and
    /// within `up_down` blocks of its levels.
    pub fn near(&self, at: [f64; 3], margin: f64, up_down: f64) -> bool {
        let inside = |v: f64, lo: i32, hi: i32, m: f64| {
            v >= f64::from(lo) - m && v <= f64::from(hi) + 1.0 + m
        };
        inside(at[0], self.lo[0], self.hi[0], margin)
            && inside(at[2], self.lo[2], self.hi[2], margin)
            && inside(at[1], self.lo[1], self.hi[1], up_down)
    }
    /// Why the box cannot be a plot, if it cannot.
    pub fn too_large(&self) -> Option<&'static str> {
        let side = (self.hi[0] - self.lo[0] + 1).max(self.hi[2] - self.lo[2] + 1);
        if side > MAX_SIDE {
            Some("side")
        } else if self.hi[1] - self.lo[1] - 1 > MAX_HEIGHT {
            Some("height")
        } else {
            None
        }
    }
}
/// A plot the player marked: the world it is in, its name, its box, whether the farm works it
/// and whether it is drawn.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Plot {
    pub scope: String,
    pub name: String,
    pub bounds: Bounds,
    pub enabled: bool,
    pub visible: bool,
}
/// The enabled plots of a world, as the farm works them.
pub fn enabled_in<'a>(list: &'a [Plot], scope: &'a str) -> impl Iterator<Item = Bounds> + 'a {
    list.iter()
        .filter(move |p| p.scope == scope && p.enabled)
        .map(|p| p.bounds)
}
/// The cells of a world's enabled plots together.
pub fn enabled_cells(list: &[Plot], scope: &str) -> i64 {
    enabled_in(list, scope).map(|b| b.cells()).sum()
}
/// A name for a new plot of a world that none of its plots has yet: "Field 1", "Field 2"...
pub fn next_name(list: &[Plot], scope: &str, base: &str) -> String {
    (1..)
        .map(|n| format!("{base} {n}"))
        .find(|name| !list.iter().any(|p| p.scope == scope && &p.name == name))
        .unwrap_or_else(|| base.to_owned())
}
fn clean(text: &str) -> String {
    text.chars()
        .filter(|c| !matches!(c, '\t' | '\n' | '\r'))
        .take(64)
        .collect()
}
/// One plot a line: scope, name, the two corners, enabled, visible; tab-separated.
pub fn encode(list: &[Plot]) -> String {
    list.iter()
        .take(MAX)
        .map(|p| {
            let [a, b, c] = p.bounds.lo;
            let [d, e, f] = p.bounds.hi;
            format!(
                "{}\t{}\t{a}\t{b}\t{c}\t{d}\t{e}\t{f}\t{}\t{}",
                clean(&p.scope),
                clean(&p.name),
                u8::from(p.enabled),
                u8::from(p.visible)
            )
        })
        .collect::<Vec<_>>()
        .join("\n")
}
/// The plots kept by `encode`; a line that does not read is left out.
pub fn decode(text: &str) -> Vec<Plot> {
    text.lines()
        .filter_map(|line| {
            let parts: Vec<&str> = line.split('\t').collect();
            let [scope, name, a, b, c, d, e, f, enabled, visible] = parts[..] else {
                return None;
            };
            let n = |s: &str| s.parse::<i32>().ok();
            let bounds = Bounds {
                lo: [n(a)?, n(b)?, n(c)?],
                hi: [n(d)?, n(e)?, n(f)?],
            };
            if (0..3).any(|i| bounds.lo[i] > bounds.hi[i]) || bounds.too_large().is_some() {
                return None;
            }
            Some(Plot {
                scope: scope.to_owned(),
                name: name.to_owned(),
                bounds,
                enabled: enabled == "1",
                visible: visible == "1",
            })
        })
        .take(MAX)
        .collect()
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn two_corners_make_a_box_whatever_their_order_and_height() {
        let b = Bounds::between([10, 70, -3], [4, 66, 5]);
        assert_eq!(b.lo, [4, 65, -3]);
        assert_eq!(b.hi, [10, 71, 5]);
        assert_eq!(Bounds::between([4, 66, 5], [10, 70, -3]), b);
        assert!(b.contains([4, 66, 5]) && b.contains([10, 70, -3]));
        assert!(b.contains([7, 65, 0]) && b.contains([7, 71, 0]));
        assert!(!b.contains([11, 68, 0]) && !b.contains([7, 72, 0]));
        assert_eq!(b.columns(), 7 * 9);
        assert_eq!(b.center(), [7, 68, 1]);
        assert!(b.near([3.5, 68.0, 0.0], 1.0, 0.0));
        assert!(!b.near([1.5, 68.0, 0.0], 1.0, 0.0));
    }
    #[test]
    fn the_square_around_a_start_is_the_old_field() {
        let b = Bounds::around([0, 64, 0], 6, 1);
        assert_eq!(b.lo, [-6, 63, -6]);
        assert_eq!(b.hi, [6, 65, 6]);
        assert_eq!(b.half(), (6, 6));
    }
    #[test]
    fn plots_are_kept_and_read_back_and_bad_lines_are_dropped() {
        let list = vec![
            Plot {
                scope: "play.example\toverworld".into(),
                name: "Wheat\tfield".into(),
                bounds: Bounds::between([0, 64, 0], [9, 64, 9]),
                enabled: true,
                visible: false,
            },
            Plot {
                scope: "play.example overworld".into(),
                name: "Field 2".into(),
                bounds: Bounds::between([-20, 70, 5], [-10, 66, 12]),
                enabled: false,
                visible: true,
            },
        ];
        let back = decode(&encode(&list));
        assert_eq!(back.len(), 2);
        // Tabs in names and scopes are dropped, not split on.
        assert_eq!(back[0].scope, "play.exampleoverworld");
        assert_eq!(back[0].name, "Wheatfield");
        assert_eq!(back[0].bounds, list[0].bounds);
        assert!(back[0].enabled && !back[0].visible);
        assert_eq!(back[1], list[1]);
        assert!(decode("a\tb\t1\t2\t3").is_empty());
        assert!(decode("a\tb\t5\t0\t0\t0\t0\t0\t1\t1").is_empty());
        assert!(decode("a\tb\t0\t0\t0\t500\t1\t0\t1\t1").is_empty());
    }
    #[test]
    fn plots_are_bounded_in_size_and_named_apart() {
        assert!(
            Bounds::between([0, 64, 0], [63, 64, 63])
                .too_large()
                .is_none()
        );
        assert_eq!(
            Bounds::between([0, 64, 0], [64, 64, 0]).too_large(),
            Some("side")
        );
        assert_eq!(
            Bounds::between([0, 60, 0], [5, 73, 5]).too_large(),
            Some("height")
        );
        let plot = |scope: &str, name: &str, enabled| Plot {
            scope: scope.into(),
            name: name.into(),
            bounds: Bounds::between([0, 64, 0], [9, 64, 9]),
            enabled,
            visible: true,
        };
        let list = vec![
            plot("w", "Field 1", true),
            plot("w", "Field 2", false),
            plot("x", "Field 3", true),
        ];
        assert_eq!(next_name(&list, "w", "Field"), "Field 3");
        assert_eq!(next_name(&list, "x", "Field"), "Field 1");
        assert_eq!(enabled_in(&list, "w").count(), 1);
        // A flat 10×10 plot: three levels (the marked one, one above, one below).
        assert_eq!(enabled_cells(&list, "w"), 300);
        assert_eq!(enabled_cells(&list, "y"), 0);
        // The largest flat plot fits the budget on its own.
        assert!(Bounds::between([0, 64, 0], [63, 64, 63]).cells() <= MAX_CELLS);
    }
}
