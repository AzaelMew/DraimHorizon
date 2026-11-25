// Both typed/UI commands and direct remote commands pass here before the wire.
fn claim_borders_client_command(
    game: &mut crate::game::Game<'_>,
    command: &str,
) -> core::ops::ControlFlow<Option<crate::game::native_server::ClientCommandOutcome>> {
    let mut ran = false;
    crate::game::claim_borders_command::claim_borders_command(game, command, &mut ran);
    crate::game::claim_borders_bar_command::baritone_command(game, command, &mut ran);
    if ran {
        core::ops::ControlFlow::Break(Some(
            crate::game::native_server::ClientCommandOutcome::Success(1),
        ))
    } else {
        core::ops::ControlFlow::Continue(())
    }
}
