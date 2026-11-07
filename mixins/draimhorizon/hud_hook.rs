fn claim_borders_hud(
    game: &mut crate::game::Game<'_>,
    batch: &mut horizon_client::hud::HudBatch,
    corners: &mut horizon_client::extras_hud::Corners,
) {
    if !game.native_on(crate::game::claim_borders::ID) {
        return;
    }
    crate::game::claim_borders_map_view::hud(game, batch, corners);
    crate::game::claim_borders_navigation_view::hud(game, batch);
    crate::game::claim_borders_server_ui::sidebar(game, batch, corners);
    crate::game::claim_borders_economy_view::hud(game, batch, corners);
    let (friends, markers, _) = crate::game::claim_borders_client::visuals(game);
    crate::game::claim_borders_overlay_view::draw(game, batch, corners, &friends, &markers);
}
