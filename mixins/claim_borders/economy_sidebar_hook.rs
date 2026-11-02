fn draimhorizons_economy_sidebar(game: &crate::game::Game<'_>) -> core::ops::ControlFlow<()> {
    if crate::game::claim_borders_economy_view::hide_sidebar(game) {
        core::ops::ControlFlow::Break(())
    } else {
        core::ops::ControlFlow::Continue(())
    }
}
