fn claim_borders_break_sent(
    game: &crate::game::Game<'_>,
    packet: &horizon_protocol::packets::play::Serverbound,
) {
    use horizon_protocol::packets::play::Serverbound as S;
    match packet {
        S::PlayerAction(p) if matches!(p.action, 0 | 1 | 3) => {
            crate::game::claim_borders_claim_observations_view::break_attempt(game, Some(p.pos))
        }
        S::UseItemOn(_) | S::UseItem(_) | S::Interact(_) => {
            crate::game::claim_borders_claim_observations_view::break_attempt(game, None)
        }
        _ => {}
    }
}
