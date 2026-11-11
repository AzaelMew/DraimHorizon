fn claim_borders_outgoing_chat(
    game: &mut crate::game::Game<'_>,
    message: &str,
) -> core::ops::ControlFlow<()> {
    // `/farm pos1`, `pos2`, `save`, `clear`, `list`: the mod's own, never sent.
    if crate::game::claim_borders_farm_plots_view::command(game, message)
        || crate::game::claim_borders_outgoing_view::intercept(game, message)
    {
        core::ops::ControlFlow::Break(())
    } else {
        core::ops::ControlFlow::Continue(())
    }
}
