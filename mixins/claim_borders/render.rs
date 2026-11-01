// Append depth-tested world-space geometry; Horizon handles camera transforms.
fn claim_borders_render(game: &crate::game::Game<'_>, gizmos: &mut horizon_client::gizmos::Gizmos) {
    use crate::game::claim_borders as m;
    if !game.native_on(m::ID) {
        m::with_state(|state| state.clear());
        return;
    }
    if game.title.is_some() || game.ui.hide_gui {
        return;
    }
    let Some(player) = game.native_player() else {
        return;
    };
    let segments =
        m::with_state(|state| state.boundaries(player.dimension, [player.x, player.y, player.z]));
    for (start, end) in segments {
        gizmos.line(start, end, 0xFF55_FF88, 2.5);
    }
}
