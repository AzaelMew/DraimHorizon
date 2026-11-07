fn horizon_chat_draw(
    game: &mut crate::game::Game<'_>,
    batch: &mut horizon_client::hud::HudBatch,
) -> core::ops::ControlFlow<()> {
    let modern = crate::game::claim_borders_chat_view::draw(game, batch);
    crate::game::claim_borders_teleport_view::draw(game, batch);
    if modern {
        core::ops::ControlFlow::Break(())
    } else {
        core::ops::ControlFlow::Continue(())
    }
}
