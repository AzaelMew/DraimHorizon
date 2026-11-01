fn claim_borders_menu_char(game: &crate::game::Game<'_>, c: char, canceled: &mut bool) {
    crate::game::claim_borders_menu_view::character(game, c, canceled);
}
