fn claim_borders_menu_key(
    game: &crate::game::Game<'_>,
    key: &str,
    modifiers: u32,
    canceled: &mut bool,
) {
    crate::game::claim_borders_menu_view::key(game, key, modifiers, canceled);
}
