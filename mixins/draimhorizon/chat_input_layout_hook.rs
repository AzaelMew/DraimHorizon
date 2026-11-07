fn claim_borders_chat_input_layout(
    game: &mut crate::game::Game<'_>,
    batch: &horizon_client::hud::HudBatch,
) {
    if !game.native_on(crate::game::claim_borders::ID) || game.screen != crate::game::Screen::Chat {
        return;
    }
    let m = horizon_client::screen_draw::Measure(&game.gui.atlas.font);
    let c = horizon_client::chat::Commands {
        dispatcher: crate::game::chat::chat_tree(&game.remote),
        registries: &game.ui.registries,
    };
    if let Some(screen) = game.ui.chat_screen.as_mut() {
        screen.resize(&m, &c, batch.width as i32, batch.height as i32);
        // Start 4 px after the translation button, derived from its geometry.
        let button =
            crate::game::claim_borders_outgoing::translation_button([batch.width, batch.height]);
        let start = (button[0] + button[2]).ceil() as i32 + 4;
        screen.input.x = start;
        screen.input.width = (batch.width as i32 - start - 4).max(32);
    }
    crate::game::claim_borders_chat_view::prepare_nickname_list(game, false);
}
