// A validation error line show_queued_chat is about to show (it shows nothing for a hidden sender,
// with no player or with chat hidden). A captured one is not recorded or narrated, and the
// answer is the capture's, as Horizon's signed_chat_line gave it.
fn claim_borders_chat_capture_error(
    game: &mut crate::game::Game<'_>,
    chat: &crate::game::remote::chat_received::QueuedChat,
) -> core::ops::ControlFlow<bool> {
    use crate::game::remote::chat_received::QueuedChatKind;
    let QueuedChatKind::Error { chat_type } = &chat.kind else {
        return core::ops::ControlFlow::Continue(());
    };
    let source = sunrise_client::chat::ChatSource::Player;
    let has_player = game.remote.as_ref().is_some_and(|r| r.has_logical_player);
    if game.social_hides(chat.sender) || !has_player || !game.ui.chat.shows(source) {
        return core::ops::ControlFlow::Continue(());
    }
    // The line show_queued_chat builds: `chat.validation_error`, red and italic.
    let mut error = serde_json::Map::new();
    error.insert("translate".into(), "chat.validation_error".into());
    error.insert("color".into(), "red".into());
    error.insert("italic".into(), true.into());
    let line = game.decorate_chat(
        chat_type,
        &serde_json::Value::Object(error),
        crate::game::remote::chat::Decoration::Chat,
    );
    crate::game::claim_borders_chat_capture::horizon_chat_capture(true, game, &line, source)
}
