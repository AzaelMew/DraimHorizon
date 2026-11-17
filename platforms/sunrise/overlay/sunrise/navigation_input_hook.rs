fn claim_borders_navigation_input(
    game: &Game<'_>,
    mut call: mod_api::__private::MovementInputCall<'_>,
    operation: impl FnOnce(&Game<'_>, mod_api::__private::MovementInputCall<'_>) -> (),
) {
    crate::game::claim_borders_navigation_view::movement(
        game,
        &mut *call.forward,
        &mut *call.strafe,
        &mut *call.jump,
        &mut *call.sneak,
    );
    operation(game, call)
}
