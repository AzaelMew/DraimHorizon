fn claim_borders_menu_settings(mod_id: &str, buttons: &mut Vec<String>) {
    if mod_id == crate::game::claim_borders::ID {
        buttons.push(
            crate::game::claim_borders_chat_view::current_language()
                .text(
                    "Настройки DraimHorizons (в мире)",
                    "DraimHorizons settings (in world)",
                )
                .into(),
        );
    }
}
