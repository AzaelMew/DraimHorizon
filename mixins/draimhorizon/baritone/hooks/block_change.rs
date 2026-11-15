fn baritone_block_change(game: &mut crate::game::Game<'_>, pos: [i32; 3], state: i32) {
    crate::game::baritone::block_changed(game, pos, state);
}
