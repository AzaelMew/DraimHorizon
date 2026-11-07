fn claim_borders_tick(game: &mut crate::game::Game<'_>) {
    crate::game::claim_borders_workshop_view::tick(game);
    crate::game::claim_borders_construction_view::tick(game);
    crate::game::claim_borders_navigation_view::tick(game);
    crate::game::claim_borders_client::tick(game);
}
