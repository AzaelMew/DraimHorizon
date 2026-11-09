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
/// The game's own list of what a box holds ("6 x Iron Ingot ... and 20 more...") duplicates the
/// grid: with the preview on, those lines are taken out, leaving the name and any real lore.
pub(super) fn strip_contents(
    game: &Game<'_>,
    stack: &ItemStack,
    lines: Vec<(horizon_client::text::Text, u32)>,
) -> Vec<(horizon_client::text::Text, u32)> {
    if !previewed(game, stack) {
        return lines;
    }
    let hover_name = |s: &ItemStack| {
        s.custom_name()
            .or_else(|| s.item_name())
            .unwrap_or_else(|| game.pack.names.display_item(&s.item))
    };
    let translate = |k: &str| game.pack.names.display_get(k);
    let mut listed = horizon_client::item_tooltip::container_lines(stack, &hover_name, &translate);
    lines
        .into_iter()
        .filter(|(text, _)| {
            let plain = text.plain();
            match listed.iter().position(|line| *line == plain) {
                Some(at) => {
                    listed.remove(at);
                    false
                }
                None => true,
            }
        })
        .collect()
}
/// A shulker box that holds something gets its whole tooltip drawn here, in the game's own
/// style: the name (and any real lore) over the grid, without the game's list of the contents.
/// True when it was drawn, so the game skips its own.
pub(super) fn draw_tooltip(
    game: &Game<'_>,
    stack: &ItemStack,
    batch: &mut HudBatch,
    x: f32,
    y: f32,
) -> bool {
    if !previewed(game, stack) {
        return false;
    }
    let lines = strip_contents(game, stack, game.stack_tooltip(stack));
    if lines.is_empty() {
        return false;
    }
    let lines: Vec<_> = lines
        .into_iter()
        .map(|(text, color)| (text.resolve_legacy(), color))
        .collect();
    let [grid_w, grid_h] = peek::SIZE;
    let atlas = &game.gui.atlas;
    let (left, top, w, h) = Game::tooltip_geometry(
        atlas,
        &lines,
        (x, y),
        (batch.width, batch.height),
        [grid_w, grid_h + 2.0],
    );
    game.tooltip_box(batch, Some(stack), left, top, w, h);
    Game::tooltip_text(batch, atlas, &lines, left, top);
    draw(game, stack, batch, [left, top + h - grid_h]);
    true
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
