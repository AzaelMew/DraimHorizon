fn claim_borders_menu_scroll(
    game: &crate::game::Game<'_>,
    screen: &crate::game::native_hooks::NativeScreenView<'_>,
    scroll: [f64; 2],
    canceled: &mut bool,
) {
    crate::game::claim_borders_chat_view::scroll(game, screen, scroll[1], canceled);
    crate::game::claim_borders_menu_view::scroll(game, screen, scroll, canceled);
}
