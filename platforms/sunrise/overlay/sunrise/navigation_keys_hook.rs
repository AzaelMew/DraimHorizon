fn claim_borders_navigation_keys(game: &Game<'_>, keys: &mut sunrise_client::sprint::Keys) {
    let original_keys = [keys.forward, keys.backward, keys.left, keys.right];
    let [mut strafe, mut forward] = keys.direction();
    let original_axes = [strafe, forward];
    crate::game::claim_borders_navigation_view::movement(
        game,
        &mut forward,
        &mut strafe,
        &mut keys.jump,
        &mut keys.shift,
    );
    if [keys.forward, keys.backward, keys.left, keys.right] != original_keys {
        return;
    }
    if [strafe, forward] != original_axes {
        if forward != original_axes[1] {
            keys.forward = forward > 0.0;
            keys.backward = forward < 0.0;
        }
        if strafe != original_axes[0] {
            keys.left = strafe > 0.0;
            keys.right = strafe < 0.0;
        }
        keys.analog = Some([strafe, forward]);
    }
}
