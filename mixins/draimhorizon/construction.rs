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
    pub fn vector(self) -> [i32; 2] {
        match self {
            Self::North => [0, -1],
            Self::East => [1, 0],
            Self::South => [0, 1],
            Self::West => [-1, 0],
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
pub fn material(name: &str) -> bool {
    matches!(
        name,
        "minecraft:cobblestone"
            | "minecraft:cobbled_deepslate"
            | "minecraft:stone"
            | "minecraft:deepslate"
            | "minecraft:dirt"
            | "minecraft:netherrack"
            | "minecraft:andesite"
            | "minecraft:granite"
            | "minecraft:diorite"
            | "minecraft:stone_bricks"
    )
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
    pub fn next(&self, mut block: impl FnMut(nav::Pos) -> Block) -> Action {
        if self.completed >= self.config.length {
            return Action::Done;
        }
        let at = self.station(self.completed + 1);
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
                nav::Cell::Unknown => return Action::Stop(Stop::Unknown),
                nav::Cell::Hazard => return Action::Stop(Stop::Hazard),
                _ => return Action::Stop(Stop::Floor),
            }
            for y in (0..i32::from(self.config.height)).rev() {
                let pos = [x, at[1] + y, z];
                let b = block(pos);
                match b.cell {
                    nav::Cell::Air => continue,
                    nav::Cell::Unknown => return Action::Stop(Stop::Unknown),
                    nav::Cell::Hazard => return Action::Stop(Stop::Hazard),
                    _ => {}
                }
                if self.config.kind == Kind::Bridge {
                    return Action::Stop(Stop::Blocked);
                }
                if b.falling || block([x, pos[1] + 1, z]).falling {
                    return Action::Stop(Stop::Falling);
                }
                if !b.diggable {
                    return Action::Stop(Stop::Protected);
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
                        nav::Cell::Unknown => return Action::Stop(Stop::Unknown),
                        nav::Cell::Hazard => return Action::Stop(Stop::Hazard),
                        _ => {}
                    }
                }
                breaking.get_or_insert(pos);
            }
        }
        if let Some(pos) = breaking {
            Action::Break(pos)
        } else if let Some(pos) = floor {
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
    fn edge_movement_is_bounded_and_materials_exclude_gravity_and_containers() {
        let job = Job::new(config(Kind::Bridge), [0, 0, 0], false).unwrap();
        assert!(job.edge_safe(job.edge()));
        assert!(!job.edge_safe([1.4, 0., 0.5]));
        assert!(!job.edge_safe([1.1, 0., 0.8]));
        assert!(material("minecraft:cobblestone"));
        for name in [
            "minecraft:sand",
            "minecraft:chest",
            "minecraft:tnt",
            "minecraft:gravel",
        ] {
            assert!(!material(name));
        }
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
