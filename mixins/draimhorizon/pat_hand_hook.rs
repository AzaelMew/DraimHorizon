fn claim_borders_pat_hand(
    game: &crate::game::Game<'_>,
    hand: horizon_client::first_person::Hand,
    x: &mut f32,
    y: &mut f32,
    z: &mut f32,
    pitch: &mut f32,
    yaw: &mut f32,
    roll: &mut f32,
    scale: &mut f32,
) {
    crate::game::claim_borders_pat_view::hand_pose(game, hand, [x, y, z, pitch, yaw, roll, scale]);
}
