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
        screen.input.x = 70;
        screen.input.width = (batch.width as i32 - 74).max(32);
    }
    crate::game::claim_borders_chat_view::prepare_nickname_list(game, false);
}
