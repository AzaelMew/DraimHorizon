fn claim_borders_menu_frame(game: &mut crate::game::Game<'_>) {
    // Migrate only the former standard pair; preserve custom bindings and later edits.
    static CHECKED: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);
    let id = crate::game::claim_borders::ID;
    if game.native_on(id)
        && !CHECKED.swap(true, std::sync::atomic::Ordering::Relaxed)
        && game
            .native_setting(crate::game::claim_borders::SETTINGS_ID, "seed_keys_v1")
            .as_deref()
            != Some("1")
    {
        let map = game.native_mod_key(id, "map", "key.keyboard.j");
        let menu = game.native_mod_key(id, "menu", "key.keyboard.m");
        if map == "key.keyboard.j" && menu == "key.keyboard.m" {
            game.native_set_mod_key(id, "map", Some("key.keyboard.m"));
            game.native_set_mod_key(id, "menu", Some("key.keyboard.j"));
        }
        game.native_set_setting(crate::game::claim_borders::SETTINGS_ID, "seed_keys_v1", "1");
    }
    crate::game::claim_borders_client::frame(game);
}
