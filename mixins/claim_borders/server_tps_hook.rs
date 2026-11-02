fn draimhorizons_server_time(
    game: &crate::game::Game<'_>,
    packet: &horizon_protocol::packets::play::Clientbound,
) {
    crate::game::claim_borders_server_tps_view::packet(game, packet);
}
