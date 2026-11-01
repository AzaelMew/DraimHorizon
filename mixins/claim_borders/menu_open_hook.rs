fn claim_borders_menu_open(game: &mut crate::game::Game<'_>, key: &str, pressed: bool) {
    use crate::game::claim_borders;
    if pressed
        && game.native_playing()
        && game.native_on(claim_borders::ID)
        && key == game.native_mod_key(claim_borders::ID, "menu", "key.keyboard.m")
    {
        crate::game::claim_borders_client::open_menu(game);
    }
}
