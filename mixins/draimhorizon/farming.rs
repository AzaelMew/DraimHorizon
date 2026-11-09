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
}
impl Config {
    pub fn valid(self) -> bool {
        (1..=12).contains(&self.radius) && self.limit <= 64 && self.crops & Crop::MASK != 0
    }
    /// The selected crops, in menu order.
    pub fn selected(self) -> impl Iterator<Item = Crop> {
        Crop::ALL
            .into_iter()
            .filter(move |c| self.crops & c.bit() != 0)
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
    fn several_crops_can_be_selected_but_not_none() {
        let both = Config {
            crops: Crop::Wheat.bit() | Crop::Beetroot.bit(),
            radius: 6,
            limit: 0,
            replant: true,
            bonemeal: true,
            autosell: false,
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
