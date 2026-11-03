fn claim_borders_menu_mouse(
    game: &crate::game::Game<'_>,
    screen: &crate::game::native_hooks::NativeScreenView<'_>,
    button: i32,
    canceled: &mut bool,
) {
    crate::game::claim_borders_economy_client::cancel_scan();
    if crate::game::claim_borders_ui_view::mouse(game, screen, button, true, canceled) {
        return;
    }
    if crate::game::claim_borders_outgoing_view::toggle_mouse(game, screen, button, true, canceled) {return;}
    if crate::game::claim_borders_waypoint_view::mouse(game, screen, button, canceled) {
        return;
    }
    if crate::game::claim_borders_economy_view::mouse(game, screen, button, true, canceled) {
        return;
    }
    crate::game::claim_borders_teleport_view::mouse(game, screen, button, canceled);
    if *canceled {
        return;
    }
    if crate::game::claim_borders_inventory_search_view::mouse(game, screen, button, canceled) {
        return;
    }
    crate::game::claim_borders_inventory_sort_view::mouse(game, screen, button, canceled);
    if *canceled {
        return;
    }
    crate::game::claim_borders_chat_view::mouse(game, screen, button, canceled);
    if crate::game::claim_borders_map_view::mouse(game, screen, button, true, canceled) {
        return;
    }
    crate::game::claim_borders_menu_view::mouse(game, screen, button, true, canceled);
}
