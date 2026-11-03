fn claim_borders_menu_key(
    game: &crate::game::Game<'_>,
    key: &str,
    modifiers: u32,
    canceled: &mut bool,
) {
    if key == "key.keyboard.escape" {
        crate::game::claim_borders_economy_client::cancel_scan();
    }
    if crate::game::claim_borders_economy_view::key(game, key, modifiers, canceled) {
        return;
    }
    if crate::game::claim_borders_inventory_sort_view::key(game, key, canceled) {
        return;
    }
    if crate::game::claim_borders_map_view::key(game, key, canceled) {
        return;
    }
    crate::game::claim_borders_menu_view::key(game, key, modifiers, canceled);
}
