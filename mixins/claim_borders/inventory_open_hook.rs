fn draimhorizons_inventory_open(
    game: &crate::game::Game<'_>,
    packet: &horizon_protocol::packets::play::OpenScreen,
) {
    crate::game::remote::claim_borders_clickthrough_view::opened(game, packet);
}
