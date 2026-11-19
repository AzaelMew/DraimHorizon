fn horizon_claim_map_update(game: &mut crate::game::Game<'_>) {
    // The engine's `Extras::update_minimap` is gone: the map images refresh once a frame here.
    crate::game::extras::claim_borders_map_backend::update(game);
}
