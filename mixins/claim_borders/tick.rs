fn claim_borders_tick(game: &mut crate::game::Game<'_>) {
    crate::game::claim_borders_client::tick(game);
}
