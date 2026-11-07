fn draimhorizons_inventory_block(
    game: &crate::game::Game<'_>,
    hand: horizon_client::first_person::Hand,
) {
    crate::game::remote::claim_borders_clickthrough_view::block_use(game, hand);
}
