// Every item tooltip goes through draw_stack_tooltip_lines: a previewed shulker box draws its own
// tooltip, text and slot grid in one box, in place of the game's.
fn claim_borders_shulker_peek_tooltip(
    game: &crate::game::Game<'_>,
    batch: &mut sunrise_client::hud::HudBatch,
    stack: &sunrise_client::items::ItemStack,
    x: f32,
    y: f32,
) -> ::core::ops::ControlFlow<()> {
    if crate::game::claim_borders_shulker_peek_view::draw_tooltip(game, stack, batch, x, y) {
        return ::core::ops::ControlFlow::Break(());
    }
    ::core::ops::ControlFlow::Continue(())
}
