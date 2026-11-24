// A system or client chat line about to be recorded.
fn claim_borders_chat_capture_line(
    game: &mut crate::game::Game<'_>,
    content: &sunrise_client::text::Text,
    source: sunrise_client::chat::ChatSource,
) -> core::ops::ControlFlow<bool> {
    crate::game::claim_borders_chat_capture::horizon_chat_capture(false, game, content, source)
}
