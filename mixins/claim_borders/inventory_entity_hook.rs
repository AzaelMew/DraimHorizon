fn draimhorizons_inventory_entity(
    game: &crate::game::Game<'_>,
    hand: horizon_client::first_person::Hand,
) {
    crate::game::remote::claim_borders_clickthrough_view::entity_use(game,hand);
}
