// Only the player's chat input may authorize protocol-message signatures.
// Server-provided RunCommand links take click_command and cannot reach this gate.
fn claim_borders_typed_chat(
    game: &mut crate::game::Game<'_>,
    message: &str,
) -> std::ops::ControlFlow<()> {
    if game.chat_abilities().allows(message)
        && crate::game::claim_borders_client::typed_relay(game, message)
    {
        std::ops::ControlFlow::Break(())
    } else {
        std::ops::ControlFlow::Continue(())
    }
}
