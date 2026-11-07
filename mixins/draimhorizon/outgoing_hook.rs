fn claim_borders_outgoing_chat(
    game: &mut crate::game::Game<'_>,
    message: &str,
) -> core::ops::ControlFlow<()> {
    if crate::game::claim_borders_outgoing_view::intercept(game, message) {
        core::ops::ControlFlow::Break(())
    } else {
        core::ops::ControlFlow::Continue(())
    }
}
