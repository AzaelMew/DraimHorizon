fn claim_borders_menu_open(game: &mut crate::game::Game<'_>, key: &str, pressed: bool) {
    use crate::game::claim_borders;
    crate::game::claim_borders_ore_view::key(game, key, pressed);
    if pressed
        && game.native_playing()
        && game.native_on(claim_borders::ID)
        && key == game.native_mod_key(claim_borders::ID, "trades", "key.keyboard.b")
    {
        crate::game::claim_borders_client::open_trades(game);
        return;
    }
    if pressed && crate::game::claim_borders_server_ui::page(game, key) {
        return;
    }
    static WAYPOINT_HELD: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);
    let waypoint_key = key == game.native_mod_key(claim_borders::ID, "waypoint", "key.keyboard.x");
    let first = waypoint_key && !WAYPOINT_HELD.swap(pressed, std::sync::atomic::Ordering::Relaxed);
    if pressed
        && first
        && game.native_playing()
        && game.native_on(claim_borders::ID)
        && key == game.native_mod_key(claim_borders::ID, "waypoint", "key.keyboard.x")
    {
        crate::game::claim_borders_client::quick_waypoint(game);
        return;
    }

    if pressed
        && game.native_playing()
        && game.native_on(claim_borders::ID)
        && key == game.native_mod_key(claim_borders::ID, "map", "key.keyboard.m")
    {
        crate::game::claim_borders_client::open_map(game);
        return;
    }
    if pressed
        && game.native_playing()
        && game.native_on(claim_borders::ID)
        && key == game.native_mod_key(claim_borders::ID, "menu", "key.keyboard.j")
    {
        crate::game::claim_borders_client::open_menu(game);
    }
}
