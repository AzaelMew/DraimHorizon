fn claim_borders_menu_mouse(
    game: &crate::game::Game<'_>,
    screen: &crate::game::native_hooks::NativeScreenView<'_>,
    button: i32,
    canceled: &mut bool,
) {
    crate::game::claim_borders_chat_view::mouse(game, screen, button, canceled);
    if crate::game::claim_borders_map_view::mouse(game, screen, button, true, canceled) {
        return;
    }
    crate::game::claim_borders_menu_view::mouse(game, screen, button, true, canceled);
}
