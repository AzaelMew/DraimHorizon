// Straight, bounded excavation and floor building. World actions remain server-authoritative.
use super::claim_borders_navigation as nav;
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Direction {
    #[default]
    North,
    East,
    South,
    West,
}
impl Direction {
    /// Minecraft yaw: 0 = south, 90 = west, 180 = north, -90 = east.
    pub fn from_yaw(yaw: f32) -> Self {
        let yaw = yaw.rem_euclid(360.0);
        match ((yaw + 45.0) / 90.0).floor() as u32 % 4 {
            0 => Self::South,
            1 => Self::West,
            2 => Self::North,
            _ => Self::East,
        }
    }
    pub fn vector(self) -> [i32; 2] {
        match self {
            Self::North => [0, -1],
            Self::East => [1, 0],
            Self::South => [0, 1],
            Self::West => [-1, 0],
        }
    }
    /// The direction of a unit horizontal step.
    pub fn from_step(dx: i32, dz: i32) -> Option<Self> {
        match (dx, dz) {
            (0, -1) => Some(Self::North),
            (1, 0) => Some(Self::East),
            (0, 1) => Some(Self::South),
            (-1, 0) => Some(Self::West),
            _ => None,
        }
    }
    pub fn next(self) -> Self {
        match self {
            Self::North => Self::East,
            Self::East => Self::South,
            Self::South => Self::West,
            Self::West => Self::North,
        }
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    Tunnel,
    Bridge,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Config {
    pub kind: Kind,
    pub direction: Direction,
    pub width: u8,
    pub height: u8,
    pub length: u16,
    pub fill_floor: bool,
}
impl Config {
    pub fn valid(self) -> bool {
        (1..=64).contains(&self.length)
            && (1..=3).contains(&self.width)
            && (2..=3).contains(&self.height)
            && (self.kind != Kind::Bridge || self.width == 1)
    }
}
#[derive(Clone, Copy, Debug)]
pub struct Block {
    pub cell: nav::Cell,
    pub diggable: bool,
    pub falling: bool,
}
impl Default for Block {
    fn default() -> Self {
        Self {
            cell: nav::Cell::Unknown,
            diggable: false,
            falling: false,
        }
    }
}
/// Cheap, plentiful blocks that are spent before anything else.
const COMMON: [&str; 15] = [
    "cobblestone",
    "cobbled_deepslate",
    "stone",
    "deepslate",
    "dirt",
    "netherrack",
    "andesite",
    "granite",
    "diorite",
    "tuff",
    "blackstone",
    "sandstone",
    "smooth_stone",
    "stone_bricks",
    "basalt",
];
/// Blocks that fall when unsupported: fine to dump into water, never for a floor.
pub fn gravity(name: &str) -> bool {
    let name = name.strip_prefix("minecraft:").unwrap_or(name);
    matches!(name, "sand" | "red_sand" | "gravel") || name.ends_with("_concrete_powder")
}
/// Whether a block item is safe to spend on a floor or a bridge. The name filter keeps out
/// everything with a function (containers, redstone parts, workstations), hazards, valuables
/// and shapes that are not a plain cube. `Game` data adds the collision/block-entity facts.
pub fn material(name: &str) -> bool {
    let Some(name) = name.strip_prefix("minecraft:") else {
        return false;
    };
    if gravity(name) {
        return false;
    }
    const DENY: &[&str] = &[
        "shulker",
        "chest",
        "barrel",
        "furnace",
        "smoker",
        "hopper",
        "dropper",
        "dispenser",
        "piston",
        "observer",
        "tnt",
        "command",
        "structure",
        "spawner",
        "bedrock",
        "barrier",
        "obsidian",
        "beacon",
        "conduit",
        "anvil",
        "table",
        "lectern",
        "jukebox",
        "note_block",
        "bookshelf",
        "bell",
        "campfire",
        "lantern",
        "torch",
        "magma",
        "cactus",
        "slime",
        "honey",
        "ice",
        "snow",
        "sponge",
        "sculk",
        "bee",
        "frogspawn",
        "infested",
        "lodestone",
        "respawn",
        "reinforced",
        "debris",
        "_ore",
        "raw_",
        "button",
        "plate",
        "slab",
        "stairs",
        "wall",
        "fence",
        "gate",
        "door",
        "sign",
        "pane",
        "bars",
        "carpet",
        "glazed",
        "head",
        "skull",
    ];
    if DENY.iter().any(|d| name.contains(d)) {
        return false;
    }
    // Storage blocks of metals, gems and redstone are valuables, not building stock.
    if name.ends_with("_block") {
        return matches!(name, "dripstone_block" | "moss_block");
    }
    COMMON.contains(&name)
        || name.ends_with("_planks")
        || name.ends_with("_log")
        || name.ends_with("_wood")
        || name.ends_with("_bricks")
        || name.ends_with("_sandstone")
        || name.ends_with("_terracotta")
        || name.ends_with("_concrete")
        || name.ends_with("_stone")
        || name.starts_with("polished_")
        || name.starts_with("smooth_")
        || matches!(
            name,
            "mossy_cobblestone"
                | "coarse_dirt"
                | "end_stone"
                | "red_sandstone"
                | "terracotta"
                | "bricks"
                | "mud"
                | "packed_mud"
                | "calcite"
                | "smooth_basalt"
                | "prismarine"
                | "clay"
        )
}
/// What a water fill may spend: the building blocks plus plain sand and gravel.
pub fn fill_material(name: &str) -> bool {
    material(name)
        || name
            .strip_prefix("minecraft:")
            .is_some_and(|n| matches!(n, "sand" | "red_sand" | "gravel"))
}
/// Spending order: the cheap, plentiful blocks first, anything else only when nothing better
/// is carried.
pub fn rank(name: &str) -> u8 {
    let name = name.strip_prefix("minecraft:").unwrap_or(name);
    u8::from(!(COMMON.contains(&name) || matches!(name, "sand" | "red_sand" | "gravel")))
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Stop {
    Unknown,
    Hazard,
    Falling,
    Protected,
    Floor,
    Blocked,
    Material,
    Refused,
    Timeout,
    Manual,
    Tool,
}
/// Stations ahead of the player that a tunnel clears before walking on.
pub const LOOKAHEAD: u16 = 2;
enum Scan {
    Stop(Stop),
    Break(nav::Pos),
    Clear(Option<nav::Pos>),
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Action {
    Break(nav::Pos),
    Place(nav::Pos),
    Walk(nav::Pos),
    Done,
    Stop(Stop),
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Phase {
    Ready,
    Align,
    Walking,
    Digging,
    Placing,
    Done,
    Stopped(Stop),
}
#[derive(Clone, Debug)]
pub struct Job {
    pub config: Config,
    pub origin: nav::Pos,
    pub completed: u16,
    pub broken: u16,
    pub placed: u16,
    pub active: bool,
    pub phase: Phase,
    pub preview: bool,
}
impl Job {
    pub fn new(config: Config, origin: nav::Pos, preview: bool) -> Option<Self> {
        config.valid().then_some(Self {
            config,
            origin,
            completed: 0,
            broken: 0,
            placed: 0,
            active: !preview,
            phase: Phase::Align,
            preview,
        })
    }
    pub fn station(&self, step: u16) -> nav::Pos {
        let [x, z] = self.config.direction.vector();
        [
            self.origin[0] + x * i32::from(step),
            self.origin[1],
            self.origin[2] + z * i32::from(step),
        ]
    }
    /// What a single station asks for, before any decision about order.
    fn scan(&self, at: nav::Pos, block: &mut impl FnMut(nav::Pos) -> Block) -> Scan {
        let [dx, dz] = self.config.direction.vector();
        let side = [-dz, dx];
        let mut breaking = None;
        let mut floor = None;
        let offsets: Vec<i32> = if self.config.width == 3 {
            vec![0, -1, 1]
        } else if self.config.width == 2 {
            vec![0, -1]
        } else {
            vec![0]
        };
        for offset in offsets {
            let x = at[0] + side[0] * offset;
            let z = at[2] + side[1] * offset;
            let under = [x, at[1] - 1, z];
            match block(under).cell {
                nav::Cell::Solid => {}
                nav::Cell::Air if self.config.fill_floor || self.config.kind == Kind::Bridge => {
                    floor.get_or_insert(under);
                }
                nav::Cell::Unknown => return Scan::Stop(Stop::Unknown),
                nav::Cell::Hazard => return Scan::Stop(Stop::Hazard),
                _ => return Scan::Stop(Stop::Floor),
            }
            for y in (0..i32::from(self.config.height)).rev() {
                let pos = [x, at[1] + y, z];
                let b = block(pos);
                match b.cell {
                    nav::Cell::Air => continue,
                    nav::Cell::Unknown => return Scan::Stop(Stop::Unknown),
                    nav::Cell::Hazard => return Scan::Stop(Stop::Hazard),
                    _ => {}
                }
                if self.config.kind == Kind::Bridge {
                    return Scan::Stop(Stop::Blocked);
                }
                if b.falling || block([x, pos[1] + 1, z]).falling {
                    return Scan::Stop(Stop::Falling);
                }
                if !b.diggable {
                    return Scan::Stop(Stop::Protected);
                }
                for delta in [
                    [1, 0, 0],
                    [-1, 0, 0],
                    [0, 1, 0],
                    [0, -1, 0],
                    [0, 0, 1],
                    [0, 0, -1],
                ] {
                    match block([pos[0] + delta[0], pos[1] + delta[1], pos[2] + delta[2]]).cell {
                        nav::Cell::Unknown => return Scan::Stop(Stop::Unknown),
                        nav::Cell::Hazard => return Scan::Stop(Stop::Hazard),
                        _ => {}
                    }
                }
                breaking.get_or_insert(pos);
            }
        }
        match breaking {
            Some(pos) => Scan::Break(pos),
            None => Scan::Clear(floor),
        }
    }
    pub fn next(&self, mut block: impl FnMut(nav::Pos) -> Block) -> Action {
        if self.completed >= self.config.length {
            return Action::Done;
        }
        let at = self.station(self.completed + 1);
        let floor = match self.scan(at, &mut block) {
            Scan::Stop(reason) => return Action::Stop(reason),
            Scan::Break(pos) => return Action::Break(pos),
            Scan::Clear(floor) => floor,
        };
        // The next stations are within reach before the player gets there: clear them
        // first so the walk is one continuous run instead of dig, stop, step, repeat.
        // A station that is not safe to dig only ends the look-ahead; it is reported
        // when the player actually arrives.
        if self.config.kind == Kind::Tunnel {
            for ahead in 2..=1 + LOOKAHEAD {
                if self.completed + ahead > self.config.length {
                    break;
                }
                match self.scan(self.station(self.completed + ahead), &mut block) {
                    Scan::Break(pos) => return Action::Break(pos),
                    Scan::Clear(_) => {}
                    Scan::Stop(_) => break,
                }
            }
        }
        if let Some(pos) = floor {
            Action::Place(pos)
        } else {
            Action::Walk(at)
        }
    }
    pub fn stop(&mut self, reason: Stop) {
        self.active = false;
        self.phase = Phase::Stopped(reason);
    }
    pub fn resume(&mut self) {
        if self.completed < self.config.length {
            self.active = true;
            self.preview = false;
            self.phase = Phase::Align;
        }
    }
    pub fn edge(&self) -> [f64; 3] {
        let mut at = nav::center(self.station(self.completed));
        let [x, z] = self.config.direction.vector();
        at[0] += f64::from(x) * 0.65;
        at[2] += f64::from(z) * 0.65;
        at
    }
    pub fn edge_safe(&self, position: [f64; 3]) -> bool {
        let at = nav::center(self.station(self.completed));
        let [x, z] = self.config.direction.vector();
        let forward = (position[0] - at[0]) * f64::from(x) + (position[2] - at[2]) * f64::from(z);
        let side = (position[0] - at[0]) * f64::from(-z) + (position[2] - at[2]) * f64::from(x);
        (-0.3..=0.78).contains(&forward) && side.abs() < 0.22 && (position[1] - at[1]).abs() < 0.3
    }
}
#[derive(Clone, Debug)]
pub struct Placement {
    pub target: nav::Pos,
    pub anchor: Option<nav::Pos>,
    pub material: String,
    pub sequence: Option<i32>,
    pub started_ms: u64,
    pub edge: bool,
}
impl Placement {
    pub fn acknowledge(&self, sequence: i32, name: &str, solid: bool) -> Option<bool> {
        self.sequence
            .filter(|s| sequence >= *s)
            .map(|_| solid && name == self.material)
    }
}
pub fn steer(position: [f64; 3], target: [f64; 3], yaw: f32) -> nav::Input {
    let dx = target[0] - position[0];
    let dz = target[2] - position[2];
    if dx.hypot(dz) < 0.06 {
        return nav::Input::default();
    }
    let yaw = f64::from(yaw).to_radians();
    let forward = -dx * yaw.sin() + dz * yaw.cos();
    let left = dx * yaw.cos() + dz * yaw.sin();
    let sector = (left.atan2(forward) / (std::f64::consts::PI / 4.0)).round() as i32;
    let (forward, strafe) = match sector.rem_euclid(8) {
        0 => (1., 0.),
        1 => (1., 1.),
        2 => (0., 1.),
        3 => (-1., 1.),
        4 => (-1., 0.),
        5 => (-1., -1.),
        6 => (0., -1.),
        _ => (1., -1.),
    };
    nav::Input {
        forward,
        strafe,
        jump: false,
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    fn config(kind: Kind) -> Config {
        Config {
            kind,
            direction: Direction::East,
            width: 1,
            height: 2,
            length: 2,
            fill_floor: true,
        }
    }
    fn air(pos: nav::Pos) -> Block {
        Block {
            cell: if pos[1] < 0 {
                nav::Cell::Solid
            } else {
                nav::Cell::Air
            },
            diggable: true,
            falling: false,
        }
    }
    #[test]
    fn tunnel_clears_top_down_and_preflights_the_whole_section() {
        let job = Job::new(config(Kind::Tunnel), [0, 0, 0], false).unwrap();
        let wall = |p: nav::Pos| {
            if p[0] == 1 && (0..2).contains(&p[1]) {
                Block {
                    cell: nav::Cell::Solid,
                    diggable: true,
                    falling: false,
                }
            } else {
                air(p)
            }
        };
        assert_eq!(job.next(wall), Action::Break([1, 1, 0]));
        assert_eq!(
            job.next(|p| if p == [2, 0, 0] {
                Block {
                    cell: nav::Cell::Hazard,
                    ..air(p)
                }
            } else {
                wall(p)
            }),
            Action::Stop(Stop::Hazard)
        );
        assert_eq!(
            job.next(|p| if p == [1, 2, 0] {
                Block {
                    falling: true,
                    ..air(p)
                }
            } else {
                wall(p)
            }),
            Action::Stop(Stop::Falling)
        );
        assert_eq!(
            job.next(|p| if p == [1, 0, 0] {
                Block {
                    diggable: false,
                    ..wall(p)
                }
            } else {
                wall(p)
            }),
            Action::Stop(Stop::Protected)
        );
    }
    #[test]
    fn bridging_never_advances_over_an_unconfirmed_gap_or_unknown_chunk() {
        let job = Job::new(config(Kind::Bridge), [0, 0, 0], false).unwrap();
        assert_eq!(
            job.next(|p| if p == [1, -1, 0] {
                air([0, 1, 0])
            } else {
                air(p)
            }),
            Action::Place([1, -1, 0])
        );
        assert_eq!(
            job.next(|p| if p == [1, -1, 0] {
                Block::default()
            } else {
                air(p)
            }),
            Action::Stop(Stop::Unknown)
        );
        assert_eq!(job.next(air), Action::Walk([1, 0, 0]));
        let place = Placement {
            target: [1, -1, 0],
            anchor: Some([0, -1, 0]),
            material: "minecraft:cobblestone".into(),
            sequence: Some(8),
            started_ms: 0,
            edge: true,
        };
        assert_eq!(place.acknowledge(7, "minecraft:cobblestone", true), None);
        assert_eq!(place.acknowledge(8, "minecraft:air", false), Some(false));
        assert_eq!(
            place.acknowledge(8, "minecraft:cobblestone", true),
            Some(true)
        );
    }
    #[test]
    fn tunnel_clears_the_stations_ahead_before_walking_on() {
        let job = Job::new(
            Config {
                length: 4,
                ..config(Kind::Tunnel)
            },
            [0, 0, 0],
            false,
        )
        .unwrap();
        let solid = |_: nav::Pos| Block {
            cell: nav::Cell::Solid,
            diggable: true,
            falling: false,
        };
        // Station 1 is open, stations 2 and 3 are rock: dig them from here.
        let wall = |p: nav::Pos| {
            if (2..=3).contains(&p[0]) && (0..2).contains(&p[1]) {
                solid(p)
            } else {
                air(p)
            }
        };
        assert_eq!(job.next(wall), Action::Break([2, 1, 0]));
        // Four stations away is beyond the look-ahead.
        let far = |p: nav::Pos| {
            if p[0] == 4 && (0..2).contains(&p[1]) {
                solid(p)
            } else {
                air(p)
            }
        };
        assert_eq!(job.next(far), Action::Walk([1, 0, 0]));
        // An unsafe station ahead never stops the walk and is not dug.
        let protected = |p: nav::Pos| {
            if p[0] == 2 && (0..2).contains(&p[1]) {
                Block {
                    diggable: false,
                    ..solid(p)
                }
            } else {
                air(p)
            }
        };
        assert_eq!(job.next(protected), Action::Walk([1, 0, 0]));
        // A bridge cannot dig ahead.
        let bridge = Job::new(config(Kind::Bridge), [0, 0, 0], false).unwrap();
        assert_eq!(bridge.next(air), Action::Walk([1, 0, 0]));
    }
    #[test]
    fn direction_from_unit_steps() {
        for d in [
            Direction::North,
            Direction::East,
            Direction::South,
            Direction::West,
        ] {
            let [x, z] = d.vector();
            assert_eq!(Direction::from_step(x, z), Some(d));
        }
        assert_eq!(Direction::from_step(1, 1), None);
        assert_eq!(Direction::from_step(0, 0), None);
    }
    #[test]
    fn tunnel_direction_tracks_player_yaw() {
        assert_eq!(Direction::from_yaw(0.0), Direction::South);
        assert_eq!(Direction::from_yaw(90.0), Direction::West);
        assert_eq!(Direction::from_yaw(180.0), Direction::North);
        assert_eq!(Direction::from_yaw(-90.0), Direction::East);
        assert_eq!(Direction::from_yaw(359.0), Direction::South);
        assert_eq!(Direction::from_yaw(-179.0), Direction::North);
    }
    #[test]
    fn edge_movement_is_bounded_and_materials_exclude_gravity_and_containers() {
        let job = Job::new(config(Kind::Bridge), [0, 0, 0], false).unwrap();
        assert!(job.edge_safe(job.edge()));
        assert!(!job.edge_safe([1.4, 0., 0.5]));
        assert!(!job.edge_safe([1.1, 0., 0.8]));
        assert!(material("minecraft:cobblestone"));
        assert!(material("minecraft:oak_planks"));
        for name in [
            "minecraft:sand",
            "minecraft:chest",
            "minecraft:tnt",
            "minecraft:gravel",
        ] {
            assert!(!material(name));
        }
        // Everything a player might carry: building stock is accepted, the rest never.
        for name in [
            "minecraft:red_sandstone",
            "minecraft:polished_andesite",
            "minecraft:white_concrete",
            "minecraft:cut_sandstone",
            "minecraft:oak_log",
            "minecraft:white_terracotta",
            "minecraft:deepslate_bricks",
            "minecraft:end_stone",
        ] {
            assert!(material(name), "{name}");
        }
        for name in [
            "minecraft:white_shulker_box",
            "minecraft:ender_chest",
            "minecraft:barrel",
            "minecraft:diamond_block",
            "minecraft:netherite_block",
            "minecraft:redstone_block",
            "minecraft:diamond_ore",
            "minecraft:ancient_debris",
            "minecraft:obsidian",
            "minecraft:stone_slab",
            "minecraft:polished_blackstone_button",
            "minecraft:oak_stairs",
            "minecraft:infested_stone",
            "minecraft:white_concrete_powder",
            "minecraft:crafting_table",
            "minecraft:sticky_piston",
            "minecraft:white_glazed_terracotta",
            "minecraft:cobblestone_wall",
            "minecraft:diamond_pickaxe",
            "mod:stone",
        ] {
            assert!(!material(name), "{name}");
        }
        assert!(fill_material("minecraft:sand") && fill_material("minecraft:gravel"));
        assert!(!fill_material("minecraft:white_concrete_powder"));
        assert!(rank("minecraft:cobblestone") < rank("minecraft:oak_planks"));
        assert!(rank("minecraft:sand") < rank("minecraft:white_concrete"));
        assert!(
            Job::new(
                Config {
                    length: 65,
                    ..config(Kind::Bridge)
                },
                [0, 0, 0],
                false
            )
            .is_none()
        );
        assert!(
            Job::new(
                Config {
                    width: 3,
                    ..config(Kind::Bridge)
                },
                [0, 0, 0],
                false
            )
            .is_none()
        );
    }
}
