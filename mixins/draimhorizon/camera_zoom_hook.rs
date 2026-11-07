fn draim_camera_zoom(
    game: &crate::game::Game<'_>,
    zoom: &mut Option<crate::game::native_hooks::NativeZoom>,
) {
    crate::game::claim_borders_camera_zoom::apply(game, zoom);
}
