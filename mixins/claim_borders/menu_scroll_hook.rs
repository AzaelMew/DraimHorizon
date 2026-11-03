fn claim_borders_menu_scroll(
    game: &crate::game::Game<'_>,
    screen: &crate::game::native_hooks::NativeScreenView<'_>,
    scroll: [f64; 2],
    canceled: &mut bool,
) {
    if crate::game::claim_borders_ui_view::editing(){*canceled=true;return;}
    if crate::game::claim_borders_inventory_sort_view::editing() {
        *canceled = true;
        return;
    }
    if crate::game::claim_borders_economy_view::scroll(scroll[1], canceled) {
        return;
    }
    crate::game::claim_borders_chat_view::scroll(game, screen, scroll[1], canceled);
    if crate::game::claim_borders_map_view::scroll(game, screen, scroll[1], canceled) {
        return;
    }
    crate::game::claim_borders_menu_view::scroll(game, screen, scroll, canceled);
}
