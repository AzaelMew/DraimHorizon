// The shulker box preview: the contents of a shulker box in its item tooltip, wherever the game
// shows one (inventory, chests, the creative tabs, the recipe book, chat). The three hooks below
// are asked each frame a tooltip shows.
use crate::game::{Game, claim_borders as claims, claim_borders_shulker_peek as peek};
use horizon_client::{hud::HudBatch, items::ItemStack};
/// The preview is on unless the player switched it off.
pub(super) fn enabled(game: &Game<'_>) -> bool {
    game.native_on(claims::ID)
        && game
            .native_setting(claims::SETTINGS_ID, "shulker_peek")
            .as_deref()
            != Some("false")
}
/// A shulker box that holds something: the only stack that gets the preview.
fn previewed(game: &Game<'_>, stack: &ItemStack) -> bool {
    enabled(game) && peek::is_shulker(&stack.item) && !stack.container().is_empty()
}
/// Reserves room for the grid under the tooltip's text.
pub(super) fn measure(game: &Game<'_>, stack: &ItemStack, size: &mut Option<[f32; 2]>) {
    if previewed(game, stack) {
        *size = Some(peek::SIZE);
    }
}
/// Draws the 27 slots and the items in them at `origin`, inside the tooltip's background.
pub(super) fn draw(game: &Game<'_>, stack: &ItemStack, batch: &mut HudBatch, origin: [f32; 2]) {
    if !previewed(game, stack) {
        return;
    }
    for slot in 0..27u8 {
        let Some([x, y]) = peek::slot_origin(slot) else {
            continue;
        };
        let (x, y) = (origin[0] + x, origin[1] + y);
        // A darker well with a faint rim, like an inventory slot.
        game.native_fill(
            batch,
            [x, y, peek::SLOT - 1.0, peek::SLOT - 1.0],
            0x40ffffff,
        );
        game.native_fill(
            batch,
            [x + 1.0, y + 1.0, peek::SLOT - 3.0, peek::SLOT - 3.0],
            0xc0101018,
        );
    }
    for (slot, held) in stack.container() {
        if let Some([x, y]) = peek::slot_origin(*slot) {
            game.draw_item(batch, held, origin[0] + x + 1.0, origin[1] + y + 1.0);
        }
    }
}
/// Tints the tooltip box with the colour of the shulker box's dye.
pub(super) fn colors(
    game: &Game<'_>,
    stack: Option<&ItemStack>,
    background: &mut u32,
    border_top: &mut u32,
    border_bottom: &mut u32,
) {
    let Some(stack) = stack.filter(|s| previewed(game, s)) else {
        return;
    };
    (*background, *border_top, *border_bottom) = peek::colors(&stack.item);
}
