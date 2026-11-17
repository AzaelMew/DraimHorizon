fn claim_borders_shulker_peek_tooltip(
    game: &crate::game::Game<'_>,
    batch: &mut sunrise_client::hud::HudBatch,
    stack: Option<&sunrise_client::items::ItemStack>,
    at: (f32, f32),
) -> ::core::ops::ControlFlow<Option<(f32, f32)>> {
    if let Some(stack) = stack
        && crate::game::claim_borders_shulker_peek_view::draw_tooltip(
            game, stack, batch, at.0, at.1,
        )
    {
        return ::core::ops::ControlFlow::Break(None);
    }
    ::core::ops::ControlFlow::Continue(())
}
