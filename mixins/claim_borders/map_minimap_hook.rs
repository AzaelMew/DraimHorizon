fn horizon_claim_minimap(
    game: &crate::game::Game<'_>,
    minimap: &mut Option<crate::game::native_hooks::NativeMinimap>,
) {
    if minimap.is_none() {
        *minimap = crate::game::claim_borders_map_view::wanted(game);
    }
}
