fn claim_borders_navigation_input(
    game: &crate::game::Game<'_>,
    call: mod_api::__private::MovementInputCall<'_>,
) {
    crate::game::claim_borders_navigation_view::movement(
        game,
        call.forward,
        call.strafe,
        call.jump,
        call.sneak,
    );
}
