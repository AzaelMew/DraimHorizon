fn horizon_chat_draw(
    game: &mut crate::game::Game<'_>,
    batch: &mut horizon_client::hud::HudBatch,
) -> core::ops::ControlFlow<()> {
    if crate::game::claim_borders_chat_view::draw(game, batch) {
        core::ops::ControlFlow::Break(())
    } else {
        core::ops::ControlFlow::Continue(())
    }
}
