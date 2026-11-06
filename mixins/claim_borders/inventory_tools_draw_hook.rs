// Container controls belong below the carried stack and vanilla item tooltips.
fn draimhorizons_inventory_tools(
    game: &crate::game::Game<'_>,
    batch: &mut horizon_client::hud::HudBatch,
) {
    let screen = game.screen_view(batch, game.screen);
    crate::game::claim_borders_inventory_search_view::render(game, batch, &screen);
    if !crate::game::claim_borders_inventory_sort_view::editing() {
        crate::game::claim_borders_inventory_sort_view::render(game, batch, &screen);
    }
}
