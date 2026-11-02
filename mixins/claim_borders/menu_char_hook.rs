fn claim_borders_menu_char(game: &crate::game::Game<'_>, c: char, canceled: &mut bool) {
    if crate::game::claim_borders_economy_view::character(game, c, canceled) {
        return;
    }
    if crate::game::claim_borders_map_view::visible() {
        *canceled = true;
        return;
    }
    crate::game::claim_borders_menu_view::character(game, c, canceled);
}
