fn claim_borders_menu_render(
    game: &crate::game::Game<'_>,
    batch: &mut horizon_client::hud::HudBatch,
    screen: &crate::game::native_hooks::NativeScreenView<'_>,
) {
    crate::game::claim_borders_economy_view::container_overlay(game, batch, screen);
    if crate::game::claim_borders_economy_view::render(game, batch, screen) {
        return;
    }
    if crate::game::claim_borders_map_view::render(game, batch, screen) {
        return;
    }
    crate::game::claim_borders_menu_view::render(game, batch, screen);
}
