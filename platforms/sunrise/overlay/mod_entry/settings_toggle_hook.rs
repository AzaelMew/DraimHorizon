fn claim_borders_settings_toggle(
    game: &mut crate::game::Game<'_>,
    mod_id: &str,
    key: &str,
) -> ::core::ops::ControlFlow<()> {
    if mod_id != crate::game::claim_borders::ID || key != "open_menu" {
        return ::core::ops::ControlFlow::Continue(());
    }
    // The toggle is a button: put it back off, then do what the Horizon settings button does.
    game.store_setting(mod_id, key, "false");
    crate::game::claim_borders_client::open_menu_from_options(game);
    crate::game::claim_borders_menu_view::open_settings();
    ::core::ops::ControlFlow::Break(())
}
