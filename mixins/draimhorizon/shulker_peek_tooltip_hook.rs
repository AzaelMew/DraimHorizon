fn claim_borders_shulker_peek_tooltip(
    _value: (),
    game: &crate::game::Game<'_>,
    stack: &horizon_client::items::ItemStack,
    batch: &mut horizon_client::hud::HudBatch,
    x: f32,
    y: f32,
    cancel: &mut bool,
) {
    if !*cancel
        && crate::game::claim_borders_shulker_peek_view::draw_tooltip(game, stack, batch, x, y)
    {
        *cancel = true;
    }
}
