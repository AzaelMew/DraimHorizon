fn claim_borders_menu_settings(mod_id: &str, buttons: &mut Vec<String>) {
    if mod_id == crate::game::claim_borders::ID {
        buttons.push("Настройки DraimHorizons (в мире)".into());
    }
}
