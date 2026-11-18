fn claim_borders_ghosts_needs(needs: &mut crate::render::RenderNeeds) {
    if mod_draimhorizon::ghosts::wanted() {
        needs.world_depth = true;
    }
}
