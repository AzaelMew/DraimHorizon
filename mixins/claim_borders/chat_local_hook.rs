fn horizon_chat_local(game: &mut crate::game::Game<'_>, text: &str) {
    crate::game::claim_borders_chat_view::capture(game, text);
}
