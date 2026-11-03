fn claim_borders_menu_settings_click(
    game: &mut crate::game::Game<'_>,mod_id: &str,button: usize,
) {
    if mod_id==crate::game::claim_borders::ID&&(button==0||button==1) {
        crate::game::claim_borders_client::open_menu_from_options(game);
        if button==1&&game.native_mod_screen()==Some(crate::game::claim_borders::ID){
            crate::game::claim_borders_ui_view::open_settings(game);
        }
    }
}
