fn claim_borders_live_inventory(game: &crate::game::Game<'_>) -> core::ops::ControlFlow<bool> {
    if crate::game::claim_borders_commerce_view::active(game) {
        // `draw_inventory` returns whether the inventory painter ran; here it did not.
        core::ops::ControlFlow::Break(false)
    } else {
        core::ops::ControlFlow::Continue(())
    }
}
