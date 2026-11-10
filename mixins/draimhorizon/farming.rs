// Fixed crop identities and growth stages; never harvest young crops.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Crop {
    #[default]
    Wheat,
    Carrots,
    Potatoes,
    Beetroot,
}
impl Crop {
    pub fn code(self) -> u8 {
        250 + self as u8
    }
    pub fn from_code(code: u8) -> Option<Self> {
        [Self::Wheat, Self::Carrots, Self::Potatoes, Self::Beetroot]
            .get(usize::from(code.checked_sub(250)?))
            .copied()
    }
    pub const ALL: [Self; 4] = [Self::Wheat, Self::Carrots, Self::Potatoes, Self::Beetroot];
    /// Every crop as a bit of a selection mask.
    pub const MASK: u8 = 0b1111;
    pub fn bit(self) -> u8 {
        1 << (self as u8)
    }
    /// The item whose icon stands for the crop in the menu.
    pub fn icon(self) -> &'static str {
        match self {
            Self::Wheat => "minecraft:wheat",
            Self::Carrots => "minecraft:carrot",
            Self::Potatoes => "minecraft:potato",
            Self::Beetroot => "minecraft:beetroot",
        }
    }
    pub fn next(self) -> Self {
        match self {
            Self::Wheat => Self::Carrots,
            Self::Carrots => Self::Potatoes,
            Self::Potatoes => Self::Beetroot,
            Self::Beetroot => Self::Wheat,
        }
    }
    pub fn block(self) -> &'static str {
        match self {
            Self::Wheat => "minecraft:wheat",
            Self::Carrots => "minecraft:carrots",
            Self::Potatoes => "minecraft:potatoes",
            Self::Beetroot => "minecraft:beetroots",
        }
    }
    pub fn seed(self) -> &'static str {
        match self {
            Self::Wheat => "minecraft:wheat_seeds",
            Self::Carrots => "minecraft:carrot",
            Self::Potatoes => "minecraft:potato",
            Self::Beetroot => "minecraft:beetroot_seeds",
        }
    }
    pub fn mature(self, age: Option<&str>) -> bool {
        age == Some(if self == Self::Beetroot { "3" } else { "7" })
    }
    pub fn named(name: &str) -> Option<Self> {
        [Self::Wheat, Self::Carrots, Self::Potatoes, Self::Beetroot]
            .into_iter()
            .find(|c| c.block() == name)
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Config {
    /// The crops to farm, one bit each (`Crop::bit`); at least one.
    pub crops: u8,
    pub radius: u8,
    pub limit: u16,
    pub replant: bool,
    /// Bone meal is used on crops that are still growing (it comes from its own container).
    pub bonemeal: bool,
    /// A full harvest container is not the end: the harvest is sold with `/sell hand`.
    pub autosell: bool,
    /// A field on several levels (terraces, floors): beds this many blocks above or below the
    /// start belong to it. 0 is a flat field (a block of unevenness either way).
    pub levels: u8,
}
/// The choices of `Config::levels` in the menu, in order.
pub const LEVELS: [u8; 6] = [0, 2, 3, 4, 6, 8];
impl Config {
    pub fn valid(self) -> bool {
        (1..=12).contains(&self.radius)
            && self.limit <= 64
            && self.crops & Crop::MASK != 0
            && self.levels <= 8
    }
    /// How far above and below the start beds are looked for.
    pub fn reach_y(self) -> i32 {
        i32::from(self.levels.max(1))
    }
    /// The selected crops, in menu order.
    pub fn selected(self) -> impl Iterator<Item = Crop> {
        Crop::ALL
            .into_iter()
            .filter(move |c| self.crops & c.bit() != 0)
    }
    /// The crop sown at `dx, dz` from the start of the field, so that the field is shared fairly
    /// between the selected crops: one crop has it all, two take a half each (west and east),
    /// three a third each (strips from west to east), four a quarter each (the four corners of
    /// the square). Cells outside the field go to the nearest part.
    pub fn zone_crop(self, dx: i32, dz: i32) -> Option<Crop> {
        let crops: Vec<Crop> = self.selected().collect();
        let r = i32::from(self.radius);
        let width = 2 * r + 1;
        let part = |d: i32, parts: i32| ((d.clamp(-r, r) + r) * parts / width) as usize;
        let index = match crops.len() {
            0 => return None,
            1 => 0,
            4 => part(dz, 2) * 2 + part(dx, 2),
            n => part(dx, n as i32),
        };
        crops.get(index).copied()
    }
}
#[cfg(test)]
mod tests {

    use super::*;
    #[test]
    fn harvests_only_mature_supported_crops() {
        for crop in [Crop::Wheat, Crop::Carrots, Crop::Potatoes, Crop::Beetroot] {
            assert_eq!(Crop::from_code(crop.code()), Some(crop));
            assert!(!crop.mature(None));
            assert!(!crop.mature(Some("0")));
            assert!(crop.mature(Some(if crop == Crop::Beetroot { "3" } else { "7" })));
            assert!(crop.seed().starts_with("minecraft:"));
        }
        assert_eq!(Crop::named("minecraft:grass"), None);
        for code in [0, 249, 254, 255] {
            assert_eq!(Crop::from_code(code), None);
        }
    }
    #[test]
    fn the_field_is_shared_fairly_between_the_selected_crops() {
        let config = |crops: u8| Config {
            crops,
            radius: 6,
            limit: 0,
            replant: true,
            bonemeal: false,
            autosell: false,
            levels: 0,
        };
        let count = |config: Config| {
            let mut seen = [0usize; 4];
            for dz in -6..=6 {
                for dx in -6..=6 {
                    seen[config.zone_crop(dx, dz).unwrap() as usize] += 1;
                }
            }
            seen
        };
        // All four: the four corners of the square, about a quarter each.
        let all = count(config(Crop::MASK));
        assert!(all.iter().all(|n| (36..=49).contains(n)), "{all:?}");
        assert_eq!(config(Crop::MASK).zone_crop(-6, -6), Some(Crop::Wheat));
        assert_eq!(config(Crop::MASK).zone_crop(6, -6), Some(Crop::Carrots));
        assert_eq!(config(Crop::MASK).zone_crop(-6, 6), Some(Crop::Potatoes));
        assert_eq!(config(Crop::MASK).zone_crop(6, 6), Some(Crop::Beetroot));
        // Two: west and east halves; three: strips.
        let two = count(config(Crop::Wheat.bit() | Crop::Potatoes.bit()));
        assert!(two[0] >= 78 && two[2] >= 78, "{two:?}");
        let three = count(config(Crop::MASK & !Crop::Beetroot.bit()));
        assert!(
            three[..3].iter().all(|n| (52..=65).contains(n)),
            "{three:?}"
        );
        // One crop: everywhere.
        assert_eq!(count(config(Crop::Carrots.bit()))[1], 169);
        assert_eq!(config(0).zone_crop(0, 0), None);
    }
    #[test]
    fn several_crops_can_be_selected_but_not_none() {
        let both = Config {
            crops: Crop::Wheat.bit() | Crop::Beetroot.bit(),
            radius: 6,
            limit: 0,
            replant: true,
            bonemeal: true,
            autosell: false,
            levels: 0,
        };
        assert!(both.valid());
        assert_eq!(
            both.selected().collect::<Vec<_>>(),
            [Crop::Wheat, Crop::Beetroot]
        );
        assert!(!Config { crops: 0, ..both }.valid());
        assert!(
            Config {
                crops: Crop::MASK,
                ..both
            }
            .valid()
        );
    }
}
