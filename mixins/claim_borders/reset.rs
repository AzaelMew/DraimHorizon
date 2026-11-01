// Clear before disconnect/reconnect, even when the mod was disabled in settings.
fn claim_borders_reset() {
    crate::game::claim_borders::with_state(|state| state.clear());
}
