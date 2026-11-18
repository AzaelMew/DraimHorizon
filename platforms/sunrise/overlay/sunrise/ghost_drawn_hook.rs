fn claim_borders_ghosts_drawn(world: &mut crate::render::WorldDrawn<'_>) {
    // The ghost renderer sits in the game crate, which names the engine's own frame record: rebuild it
    // from the mod record's fields and hand it over type-erased.
    let mut frame = sunrise_client_render::render_slots::WorldDrawn {
        device: world.device,
        queue: world.queue,
        encoder: &mut *world.encoder,
        color: world.color,
        color_view: world.color_view,
        color_format: world.color_format,
        size: world.size,
        depth: world.depth,
        depth_size: world.depth_size,
        view_proj: world.view_proj,
        jittered: world.jittered,
        camera: world.camera,
        entities: world.entities,
        spans: world.spans,
        motion: world.motion,
        atlas: world.atlas,
        quad_indices: world.quad_indices,
        block_sprites: sunrise_client_render::render_slots::BlockSpriteArrays {
            views: world.block_sprites.views,
            split: world.block_sprites.split,
            size: world.block_sprites.size,
            nearest: world.block_sprites.nearest,
            filtered: world.block_sprites.filtered,
        },
    };
    mod_draimhorizon::ghosts::draw(
        (&mut frame as *mut sunrise_client_render::render_slots::WorldDrawn<'_>).cast::<()>(),
    );
}
