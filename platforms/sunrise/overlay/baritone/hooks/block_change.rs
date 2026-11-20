fn baritone_block_change(game: &mut crate::game::Game<'_>, pos: [i32; 3]) {
    // The target hands the new state as a `BlockStateId`; the world already holds it by the time
    // this hook runs (it is the write's own result), and the port's cache keys on the state id.
    let state = game.world.block(pos[0], pos[1], pos[2]).0;
    crate::game::baritone::block_changed(game, pos, state as i32);
}
