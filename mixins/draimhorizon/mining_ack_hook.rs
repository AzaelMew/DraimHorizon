fn claim_borders_mining_ack(_value: (), game: &crate::game::Game<'_>, sequence: i32) {
    crate::game::claim_borders_navigation_view::mining_ack(game, sequence);
}
