fn draim_camera_zoom_scroll(
    game: &crate::game::Game<'_>,
    accumulated: [i32; 2],
    canceled: &mut bool,
) {
    crate::game::claim_borders_camera_zoom::scroll(game, accumulated[1], canceled);
}
