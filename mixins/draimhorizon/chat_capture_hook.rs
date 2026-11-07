fn horizon_chat_capture(
    trust: &crate::game::remote::ChatTrust,
    game: &mut crate::game::Game<'_>,
    content: &horizon_client::text::Text,
    source: horizon_client::chat::ChatSource,
) -> core::ops::ControlFlow<bool> {
    let allowed = !matches!(trust, crate::game::remote::ChatTrust::NotSecure)
        || !game.settings.file.bool("onlyShowSecureChat");
    if allowed && game.ui.chat.shows(source) {
        let text = content.plain();
        if game.native_on(crate::game::claim_borders::ID)
            && source == horizon_client::chat::ChatSource::Server
            && crate::game::claim_borders_economy_client::quiet(&text)
        {
            return core::ops::ControlFlow::Break(false);
        }
        if source == horizon_client::chat::ChatSource::Server
            && let Some((_, accept, _)) = crate::game::claim_borders_teleport::parse(&text)
        {
            crate::game::claim_borders_teleport_view::capture(&text);
            if accept.starts_with("trade ") {
                crate::game::claim_borders_chat_view::capture_content(game, content, source);
            }
        } else if matches!(trust, crate::game::remote::ChatTrust::Error) {
            crate::game::claim_borders_chat_view::capture(game, &text);
        } else {
            crate::game::claim_borders_chat_view::capture_content(game, content, source);
        }
    }
    core::ops::ControlFlow::Continue(())
}
