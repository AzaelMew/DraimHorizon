// Pure rules for the shulker box preview in item tooltips: what is a shulker box, where the
// slots of its 9x3 grid go and which colours the tooltip takes from the box's dye.
pub const COLS: usize = 9;
pub const ROWS: usize = 3;
/// One slot of the grid, in GUI pixels (a vanilla inventory slot).
pub const SLOT: f32 = 18.0;
/// The size of the grid, and so of the tooltip component.
pub const SIZE: [f32; 2] = [COLS as f32 * SLOT, ROWS as f32 * SLOT];
/// Whether an item id is a shulker box of any colour (or the undyed one).
pub fn is_shulker(item: &str) -> bool {
    item == "minecraft:shulker_box"
        || item
            .strip_prefix("minecraft:")
            .is_some_and(|name| name.ends_with("_shulker_box"))
}
/// The top left corner of a slot of the container, relative to the grid; `None` past the 27.
pub fn slot_origin(slot: u8) -> Option<[f32; 2]> {
    let slot = usize::from(slot);
    (slot < COLS * ROWS).then_some([(slot % COLS) as f32 * SLOT, (slot / COLS) as f32 * SLOT])
}
/// The dye colour of a shulker box (`DyeColor.getTextureDiffuseColor`), the box's own purple for
/// the undyed one.
pub fn dye(item: &str) -> u32 {
    let name = item.strip_prefix("minecraft:").unwrap_or(item);
    match name.strip_suffix("_shulker_box").unwrap_or("") {
        "white" => 0xf9fffe,
        "orange" => 0xf9801d,
        "magenta" => 0xc74ebd,
        "light_blue" => 0x3ab3da,
        "yellow" => 0xfed83d,
        "lime" => 0x80c71f,
        "pink" => 0xf38baa,
        "gray" => 0x474f52,
        "light_gray" => 0x9d9d97,
        "cyan" => 0x169c9c,
        "purple" => 0x8932b8,
        "blue" => 0x3c44aa,
        "brown" => 0x835432,
        "green" => 0x5e7c16,
        "red" => 0xb02e26,
        "black" => 0x1d1d21,
        _ => 0x8e618e,
    }
}
fn mix(a: u32, b: u32, t: f32) -> u32 {
    let channel = |shift: u32| {
        let (a, b) = (((a >> shift) & 255) as f32, ((b >> shift) & 255) as f32);
        (a + (b - a) * t).round().clamp(0.0, 255.0) as u32
    };
    (channel(16) << 16) | (channel(8) << 8) | channel(0)
}
/// The tooltip colours for a shulker box, as ARGB: the background and the top and bottom of the
/// border. A dark base tinted by the box's dye, so the white text stays readable and the dye
/// shows at a glance; a black box (nearly black dye) gets a neutral grey tint.
pub fn colors(item: &str) -> (u32, u32, u32) {
    let mut dye = dye(item);
    let luminance = ((dye >> 16) & 255) + ((dye >> 8) & 255) + (dye & 255);
    if luminance < 150 {
        dye = 0x6b6b73;
    }
    let background = mix(0x14141c, dye, 0.32);
    let top = mix(background, dye, 0.65);
    let bottom = mix(background, 0x000000, 0.45);
    (
        0xf0_000000 | background,
        0xf0_000000 | top,
        0xf0_000000 | bottom,
    )
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn every_colour_of_shulker_box_is_recognised_and_nothing_else() {
        assert!(is_shulker("minecraft:shulker_box"));
        assert!(is_shulker("minecraft:cyan_shulker_box"));
        assert!(is_shulker("minecraft:light_gray_shulker_box"));
        assert!(!is_shulker("minecraft:chest"));
        assert!(!is_shulker("minecraft:ender_chest"));
        assert!(!is_shulker("shulker_box"));
        assert!(!is_shulker("othermod:shulker_box_lid"));
    }
    #[test]
    fn the_grid_is_nine_by_three_slots_of_eighteen_pixels() {
        assert_eq!(SIZE, [162.0, 54.0]);
        assert_eq!(slot_origin(0), Some([0.0, 0.0]));
        assert_eq!(slot_origin(8), Some([144.0, 0.0]));
        assert_eq!(slot_origin(9), Some([0.0, 18.0]));
        assert_eq!(slot_origin(26), Some([144.0, 36.0]));
        assert_eq!(slot_origin(27), None);
    }
    #[test]
    fn the_tooltip_takes_the_colour_of_the_dye() {
        let cyan = colors("minecraft:cyan_shulker_box");
        let red = colors("minecraft:red_shulker_box");
        let black = colors("minecraft:black_shulker_box");
        assert_ne!(cyan.0, red.0);
        assert_ne!(cyan.0, black.0);
        // Opaque, and the cyan box is greener-blue than red.
        assert_eq!(cyan.0 >> 24, 0xf0);
        assert!(((cyan.0 >> 8) & 255) > ((red.0 >> 8) & 255));
        assert!(((red.0 >> 16) & 255) > ((cyan.0 >> 16) & 255));
        // The undyed box has a colour of its own, and the border is lighter on top.
        assert_ne!(colors("minecraft:shulker_box").0, black.0);
        assert!((cyan.1 & 0xffffff) > (cyan.2 & 0xffffff));
        assert_eq!(dye("minecraft:white_shulker_box"), 0xf9fffe);
    }
}
