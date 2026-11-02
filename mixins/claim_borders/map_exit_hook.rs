fn claim_borders_map_exit() {
    crate::game::extras::claim_borders_map_backend::finish();
    crate::game::claim_borders_economy_client::flush();
}
