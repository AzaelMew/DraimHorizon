fn draimhorizons_inventory_search(
    game: &crate::game::Game<'_>,
    stack: &horizon_client::items::ItemStack,
    x: f32,
    y: f32,
    batch: &mut horizon_client::hud::HudBatch,
) {
    crate::game::claim_borders_inventory_search_view::decorate(game, stack, x, y, batch);
}
