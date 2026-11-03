fn claim_borders_menu_mouse(
    game: &crate::game::Game<'_>,
    screen: &crate::game::native_hooks::NativeScreenView<'_>,
    button: i32,
    canceled: &mut bool,
) {
    if crate::game::claim_borders_ui_view::mouse(game, screen, button, false, canceled) {
        return;
    }
    if crate::game::claim_borders_waypoint_view::release(button,canceled){return;}
    if crate::game::claim_borders_inventory_search_view::release(button,canceled){return;}
    crate::game::claim_borders_inventory_sort_view::release(button, canceled);
    if *canceled {
        return;
    }
    if crate::game::claim_borders_economy_view::mouse(game, screen, button, false, canceled) {
        return;
    }
    if crate::game::claim_borders_map_view::mouse(game, screen, button, false, canceled) {
        return;
    }
    crate::game::claim_borders_menu_view::mouse(game, screen, button, false, canceled);
}
