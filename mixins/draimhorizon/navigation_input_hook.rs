fn claim_borders_navigation_input(
    game: &crate::game::Game<'_>,
    forward: &mut f32,
    strafe: &mut f32,
    jump: &mut bool,
    sneak: &mut bool,
) {
    crate::game::claim_borders_navigation_view::movement(game, forward, strafe, jump, sneak);
}
