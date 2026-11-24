// The chat validation error line about to be recorded. A captured one is not recorded; its
// narration is the game's, after this call.
fn claim_borders_chat_capture_error(
    game: &mut crate::game::Game<'_>,
    content: &sunrise_client::text::Text,
    source: sunrise_client::chat::ChatSource,
    tag: &Option<sunrise_client::chat::Tag>,
) -> core::ops::ControlFlow<()> {
    if tag.as_ref() != Some(&sunrise_client::chat::Tag::chat_error()) {
        return core::ops::ControlFlow::Continue(());
    }
    match crate::game::claim_borders_chat_capture::horizon_chat_capture(true, game, content, source) {
        core::ops::ControlFlow::Break(_) => core::ops::ControlFlow::Break(()),
        core::ops::ControlFlow::Continue(()) => core::ops::ControlFlow::Continue(()),
    }
}
