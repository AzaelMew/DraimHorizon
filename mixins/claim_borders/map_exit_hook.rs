fn claim_borders_map_exit() {
    crate::game::extras::claim_borders_map_backend::finish();
    crate::game::claim_borders_economy_client::flush();
    crate::game::claim_borders_claim_observations_view::flush();
}
