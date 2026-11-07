// Clear before disconnect/reconnect, even when the mod was disabled in settings.
fn claim_borders_reset() {
    crate::game::claim_borders_navigation_view::reset();
    crate::game::claim_borders_waypoint_view::reset();
    crate::game::claim_borders_inventory_search_view::reset();
    crate::game::claim_borders_ui_view::reset();
    crate::game::claim_borders_outgoing_view::reset();
    crate::game::claim_borders_teleport_view::reset();
    crate::game::claim_borders_server_tps_view::reset();
    crate::game::claim_borders_claim_observations_view::reset();
    crate::game::claim_borders_claim_memory::reset();
    crate::game::claim_borders_auto_trust_view::reset();
    crate::game::claim_borders_economy_client::reset();
    crate::game::claim_borders_client::reset();
    crate::game::claim_borders_inventory_sort_view::reset();
    crate::game::claim_borders_map_view::reset();
    crate::game::claim_borders_chat_view::reset();
    crate::game::claim_borders_ore_view::reset();
    crate::game::claim_borders::with_state(|state| state.clear());
}
