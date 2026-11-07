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
    pub crop: Crop,
    pub radius: u8,
    pub limit: u16,
    pub replant: bool,
}
impl Config {
    pub fn valid(self) -> bool {
        (1..=12).contains(&self.radius) && self.limit <= 64
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
}
