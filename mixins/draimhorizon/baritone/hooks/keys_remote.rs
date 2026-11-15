fn baritone_keys_remote(game: &mut crate::game::Game<'_>) {
    crate::game::baritone_input::through(game);
    crate::game::baritone::look_pre(game);
}
