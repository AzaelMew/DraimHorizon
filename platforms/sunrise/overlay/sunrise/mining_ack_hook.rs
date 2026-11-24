fn claim_borders_mining_ack(
    value: Result<()>,
    game: &crate::game::Game<'_>,
    sequence: i32,
) -> Result<()> {
    crate::game::claim_borders_navigation_view::mining_ack(game, sequence);
    crate::game::claim_borders_construction_view::ack(game, sequence);
    value
}
