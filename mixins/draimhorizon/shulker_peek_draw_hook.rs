fn claim_borders_shulker_peek_draw(
    _value: (),
    game: &crate::game::Game<'_>,
    stack: &horizon_client::items::ItemStack,
    batch: &mut horizon_client::hud::HudBatch,
    origin: [f32; 2],
) {
    crate::game::claim_borders_shulker_peek_view::draw(game, stack, batch, origin);
}
