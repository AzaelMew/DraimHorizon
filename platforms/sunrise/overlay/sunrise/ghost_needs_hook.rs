fn claim_borders_ghosts_needs(mut needs: RenderNeeds) -> RenderNeeds {
    if crate::render_slots::claim_borders_ghosts::wanted() {
        needs.world_depth = true;
    }
    needs
}
