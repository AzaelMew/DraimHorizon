// A player's chat line the mods let through: a captured one is neither recorded nor narrated.
fn claim_borders_chat_capture_player(
    value: bool,
    game: &mut crate::game::Game<'_>,
    system: bool,
    line: &sunrise_client::text::Text,
) -> bool {
    if !value || system {
        return value;
    }
    let source = sunrise_client::chat::ChatSource::Player;
    match crate::game::claim_borders_chat_capture::horizon_chat_capture(false, game, line, source) {
        core::ops::ControlFlow::Break(shown) => shown,
        core::ops::ControlFlow::Continue(()) => true,
    }
}
