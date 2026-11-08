// Filling a pond from the bottom up: only cells that rest on something solid are chosen, so
// a placed block never has to fall and every placement has a floor to click on.
use super::claim_borders_navigation::{self as nav, Pos};
/// Farthest eye-to-cell distance the placement driver aims at.
pub const REACH: f64 = 3.7;
/// How far below the player's feet the pond is scanned.
pub const DEPTH: i32 = 12;
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Config {
    pub radius: u8,
}
impl Config {
    pub fn valid(self) -> bool {
        (1..=12).contains(&self.radius)
    }
}
/// Blocks a building block replaces when it is placed into them.
pub fn replaceable(name: &str) -> bool {
    matches!(
        name,
        "minecraft:water" | "minecraft:seagrass" | "minecraft:tall_seagrass"
    )
}
/// A cell to fill and whether the cell below it can carry the new block.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Candidate {
    pub at: Pos,
    pub supported: bool,
}
/// The next cell to fill and whether it is within reach of the player's eye. Lowest first
/// among cells in reach, then nearest; when nothing is in reach the nearest supported cell
/// is returned so the caller can walk there. `None` when no supported cell is left.
pub fn pick(cells: &[Candidate], eye: [f64; 3]) -> Option<(Pos, bool)> {
    let distance = |c: &Candidate| nav::distance(nav::center(c.at), eye);
    let supported = || cells.iter().filter(|c| c.supported);
    supported()
        .filter(|c| distance(c) <= REACH)
        .min_by(|a, b| {
            a.at[1]
                .cmp(&b.at[1])
                .then(distance(a).total_cmp(&distance(b)))
                .then(a.at.cmp(&b.at))
        })
        .map(|c| (c.at, true))
        .or_else(|| {
            supported()
                .min_by(|a, b| distance(a).total_cmp(&distance(b)).then(a.at.cmp(&b.at)))
                .map(|c| (c.at, false))
        })
}
/// For falling blocks dropped into the surface: the nearest cell with something to click on,
/// within reach if there is one, otherwise the nearest overall so the caller can walk there.
pub fn pick_nearest(cells: &[Candidate], eye: [f64; 3]) -> Option<(Pos, bool)> {
    let distance = |c: &Candidate| nav::distance(nav::center(c.at), eye);
    let nearest = cells
        .iter()
        .filter(|c| c.supported)
        .min_by(|a, b| distance(a).total_cmp(&distance(b)).then(a.at.cmp(&b.at)))?;
    Some((nearest.at, distance(nearest) <= REACH))
}
/// Whether a column position lies inside the square around the start.
pub fn inside(config: Config, origin: Pos, at: Pos) -> bool {
    let r = i32::from(config.radius);
    (at[0] - origin[0]).abs() <= r && (at[2] - origin[2]).abs() <= r
}
/// Where to hang over a shore block to click its wall that faces the water cell `target`.
/// A wall facing the water cannot be hit from further inland: the ray meets the block's top
/// first, so the eye has to be past the edge, as when bridging. Returns the point to steer to
/// (0.15 beyond the edge of the block's top) and that block: a solid horizontal neighbour of
/// the target with two open cells above it. The nearest one to the player wins.
pub fn edge_stand(
    target: Pos,
    player: [f64; 3],
    solid: impl Fn(Pos) -> bool,
    open: impl Fn(Pos) -> bool,
) -> Option<([f64; 3], Pos)> {
    [[1, 0], [-1, 0], [0, 1], [0, -1]]
        .into_iter()
        .filter_map(|[dx, dz]| {
            let n = [target[0] + dx, target[1], target[2] + dz];
            let feet = [n[0], n[1] + 1, n[2]];
            (solid(n) && open(feet) && open([feet[0], feet[1] + 1, feet[2]])).then(|| {
                let mut at = nav::center(feet);
                at[0] -= f64::from(dx) * 0.65;
                at[2] -= f64::from(dz) * 0.65;
                (at, n)
            })
        })
        .min_by(|a, b| {
            let d = |at: [f64; 3]| (at[0] - player[0]).hypot(at[2] - player[2]);
            d(a.0).total_cmp(&d(b.0)).then(a.1.cmp(&b.1))
        })
}
#[cfg(test)]
mod tests {
    use super::*;
    fn cell(at: Pos, supported: bool) -> Candidate {
        Candidate { at, supported }
    }
    #[test]
    fn fills_the_lowest_reachable_supported_cell_first() {
        let eye = [0.5, 65.6, 0.5];
        let cells = [
            cell([1, 64, 0], true),
            cell([1, 63, 0], true),
            cell([1, 62, 0], false),
        ];
        assert_eq!(pick(&cells, eye), Some(([1, 63, 0], true)));
        // An unsupported cell is never chosen, even when it is the only one in reach.
        assert_eq!(pick(&[cell([1, 62, 0], false)], eye), None);
        assert_eq!(pick(&[], eye), None);
    }
    #[test]
    fn walks_toward_the_nearest_cell_when_nothing_is_in_reach() {
        let eye = [0.5, 65.6, 0.5];
        let cells = [cell([9, 60, 0], true), cell([6, 60, 0], true)];
        assert_eq!(pick(&cells, eye), Some(([6, 60, 0], false)));
    }
    #[test]
    fn dropping_picks_the_nearest_cell_that_has_an_anchor() {
        let eye = [0.5, 65.6, 0.5];
        let cells = [
            cell([1, 64, 0], false),
            cell([3, 64, 0], true),
            cell([2, 64, 1], true),
        ];
        assert_eq!(pick_nearest(&cells, eye), Some(([2, 64, 1], true)));
        let far = [cell([9, 64, 0], true)];
        assert_eq!(pick_nearest(&far, eye), Some(([9, 64, 0], false)));
        assert_eq!(pick_nearest(&[cell([1, 64, 0], false)], eye), None);
    }
    #[test]
    fn same_layer_prefers_the_nearest_cell() {
        let eye = [0.5, 65.6, 0.5];
        let cells = [cell([3, 64, 0], true), cell([1, 64, 0], true)];
        assert_eq!(pick(&cells, eye), Some(([1, 64, 0], true)));
    }
    #[test]
    fn config_and_area_are_bounded() {
        assert!(Config { radius: 12 }.valid());
        assert!(!Config { radius: 0 }.valid());
        assert!(!Config { radius: 13 }.valid());
        let c = Config { radius: 3 };
        assert!(inside(c, [0, 64, 0], [3, 10, -3]));
        assert!(!inside(c, [0, 64, 0], [4, 64, 0]));
        assert!(replaceable("minecraft:water") && !replaceable("minecraft:lava"));
    }
    #[test]
    fn hangs_over_the_nearest_shore_block_next_to_the_water() {
        let shore = [[0, 64, 0], [0, 64, 1]];
        let solid = |at: Pos| shore.contains(&at);
        let open = |at: Pos| at[1] >= 65;
        // Target [1, 64, 0] touches the shore block [0, 64, 0] on its -x side.
        let (at, n) = edge_stand([1, 64, 0], [0.5, 65.0, 0.5], solid, open).unwrap();
        assert_eq!(n, [0, 64, 0]);
        assert!(
            (at[0] - 1.15).abs() < 1e-9
                && (at[1] - 65.0).abs() < 1e-9
                && (at[2] - 0.5).abs() < 1e-9
        );
        // The player is next to the second block, so that one is chosen for the diagonal cell.
        let (_, n) = edge_stand([1, 64, 1], [0.5, 65.0, 1.5], solid, open).unwrap();
        assert_eq!(n, [0, 64, 1]);
        // Nothing to hang over when the neighbour is not solid or its top is covered.
        assert_eq!(edge_stand([5, 64, 5], [0.5, 65.0, 0.5], solid, open), None);
        assert_eq!(
            edge_stand([1, 64, 0], [0.5, 65.0, 0.5], solid, |_| false),
            None
        );
    }
}
