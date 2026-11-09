fn claim_borders_shulker_peek_measure(
    _value: (),
    game: &crate::game::Game<'_>,
    stack: &horizon_client::items::ItemStack,
    size: &mut Option<[f32; 2]>,
) {
    crate::game::claim_borders_shulker_peek_view::measure(game, stack, size);
}
