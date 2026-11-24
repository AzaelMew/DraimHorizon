fn claim_borders_nickname_completion(
    game: &mut crate::game::Game<'_>,
    key: winit::keyboard::KeyCode,
) -> core::ops::ControlFlow<bool> {
    if crate::game::claim_borders_chat_view::complete_nickname(game, key) {
        // `chat_key_code` returns whether it handled the key.
        core::ops::ControlFlow::Break(true)
    } else {
        core::ops::ControlFlow::Continue(())
    }
}
