fn claim_borders_private_chat(
    game: &mut crate::game::Game<'_>,
    content: &horizon_client::text::Text,
) -> std::ops::ControlFlow<bool> {
    if crate::game::claim_borders_client::private_chat(game, &content.component()) {
        std::ops::ControlFlow::Break(false)
    } else {
        std::ops::ControlFlow::Continue(())
    }
}
