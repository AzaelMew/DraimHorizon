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
        // "Global chat off": public chat of anyone but friends and important players is dropped,
        // whether a player's signed chat or a line a server plugin sends as plain text.
        if crate::game::claim_borders_chat_view::global_hidden(game, &text) {
            return core::ops::ControlFlow::Break(false);
        }
        if game.native_on(crate::game::claim_borders::ID)
            && matches!(
                source,
                horizon_client::chat::ChatSource::Server | horizon_client::chat::ChatSource::Player
            )
        {
            // Every shown message counts toward the routine-reply window; only a server line
            // that is the automatic command's own output is hidden by it.
            let routine = crate::game::claim_borders_economy_client::deliver(&text);
            let claims = crate::game::claim_borders_claim_replies::on_shown_line(
                &text,
                crate::game::claim_borders_economy_client::quiet_enabled(),
            );
            let trust = crate::game::claim_borders_auto_trust_view::deliver(&text);
            let info = crate::game::claim_borders_claim_observations_view::deliver(&text);
            // Every one of these decides about this delivery alone and consumes its own state;
            // there is no shared "recently hidden" text list any more.
            if source == horizon_client::chat::ChatSource::Server
                && (routine || claims || trust || info)
            {
                return core::ops::ControlFlow::Break(false);
            }
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
