fn claim_borders_ghosts_needs(needs: &mut crate::gfx::native_render::RenderNeeds) {
    if crate::gfx::native_render::claim_borders_ghosts::wanted() {
        needs.world_depth = true;
    }
}
