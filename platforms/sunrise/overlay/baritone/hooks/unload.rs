fn baritone_unload(
    game: &crate::game::Game<'_>,
    gone: &sunrise_client::fast_hash::FxHashSet<(i32, i32)>,
) {
    if !crate::game::baritone_unload::is_on() {
        return;
    }
    for &(x, z) in gone {
        if let Some(chunk) = game.world.chunks.resident_get(&(x, z)) {
            crate::game::baritone_unload::push(x, z, chunk.clone());
        }
    }
}
