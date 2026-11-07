fn horizon_claim_map_update(game: &mut crate::game::Game<'_>) -> core::ops::ControlFlow<()> {
    if crate::game::extras::claim_borders_map_backend::update(game) {
        core::ops::ControlFlow::Break(())
    } else {
        core::ops::ControlFlow::Continue(())
    }
}
