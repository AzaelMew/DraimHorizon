// Clear before disconnect/reconnect, even when the mod was disabled in settings.
fn claim_borders_reset() {
    crate::game::claim_borders_client::reset();
    crate::game::claim_borders_inventory_sort_view::reset();
    crate::game::claim_borders_map_view::reset();
    crate::game::claim_borders_chat_view::reset();
    crate::game::claim_borders::with_state(|state| state.clear());
}
