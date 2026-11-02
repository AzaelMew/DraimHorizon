fn horizon_chat_capture(
    trust: &crate::game::remote::ChatTrust,
    game: &mut crate::game::Game<'_>,
    content: &horizon_client::text::Text,
    source: horizon_client::chat::ChatSource,
) {
    let allowed = !matches!(trust, crate::game::remote::ChatTrust::NotSecure)
        || !game.settings.file.bool("onlyShowSecureChat");
    if allowed && game.ui.chat.shows(source) {
        let text = content.plain();
        if source == horizon_client::chat::ChatSource::Server
            && crate::game::claim_borders_teleport::parse(&text).is_some()
        {
            crate::game::claim_borders_teleport_view::capture(&text);
        } else {
            crate::game::claim_borders_chat_view::capture_content(game, content, source);
        }
    }
}
