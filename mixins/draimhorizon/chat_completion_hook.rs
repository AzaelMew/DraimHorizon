fn claim_borders_nickname_completion(
    game: &mut crate::game::Game<'_>,
    key: winit::keyboard::KeyCode,
) -> core::ops::ControlFlow<()> {
    if crate::game::claim_borders_chat_view::complete_nickname(game, key) {
        core::ops::ControlFlow::Break(())
    } else {
        core::ops::ControlFlow::Continue(())
    }
}
