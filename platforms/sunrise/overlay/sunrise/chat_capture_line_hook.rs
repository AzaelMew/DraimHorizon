// A system or client chat line about to be recorded.
fn claim_borders_chat_capture_line(
    game: &mut crate::game::Game<'_>,
    content: &sunrise_client::text::Text,
    source: sunrise_client::chat::ChatSource,
) -> core::ops::ControlFlow<bool> {
    if source == sunrise_client::chat::ChatSource::Client {
        // Keep local notices out of server-message parsing and capture them once.
        if game.ui.chat.shows(source) {
            crate::game::claim_borders_chat_view::capture_local(game, &content.plain());
        }
        return core::ops::ControlFlow::Continue(());
    }
    crate::game::claim_borders_chat_capture::horizon_chat_capture(false, game, content, source)
}
