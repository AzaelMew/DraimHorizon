fn claim_borders_pat_hand(game: &crate::game::Game<'_>, render: &mut mod_api::HandRender<'_>) {
    use sunrise_client::first_person::Hand;
    let hand = match render.hand {
        mod_api::Hand::Main => Hand::Main,
        _ => Hand::Off,
    };
    let [x, y, z] = &mut render.translation;
    let [pitch, yaw, roll] = &mut render.rotation;
    let scale = &mut render.scale;
    crate::game::claim_borders_pat_view::hand_pose(game, hand, [x, y, z, pitch, yaw, roll, scale]);
}
