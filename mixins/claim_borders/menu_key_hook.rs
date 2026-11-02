fn claim_borders_menu_key(
    game: &crate::game::Game<'_>,
    key: &str,
    modifiers: u32,
    canceled: &mut bool,
) {
    if crate::game::claim_borders_map_view::key(game, key, canceled) {
        return;
    }
    crate::game::claim_borders_menu_view::key(game, key, modifiers, canceled);
}
