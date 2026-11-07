fn claim_borders_list(
    game: &crate::game::Game<'_>,
    mods: &mut Vec<crate::game::native_hooks::NativeMod>,
) {
    let language = crate::game::claim_borders_chat_view::language(game);
    mods.push(crate::game::claim_borders_client::native_mod(language));
}
