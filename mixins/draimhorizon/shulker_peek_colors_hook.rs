fn claim_borders_shulker_peek_colors(
    _value: (),
    game: &crate::game::Game<'_>,
    stack: Option<&horizon_client::items::ItemStack>,
    background: &mut u32,
    border_top: &mut u32,
    border_bottom: &mut u32,
) {
    crate::game::claim_borders_shulker_peek_view::colors(
        game,
        stack,
        background,
        border_top,
        border_bottom,
    );
}
