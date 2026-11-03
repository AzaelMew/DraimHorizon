fn claim_borders_menu_settings(mod_id: &str, buttons: &mut Vec<String>) {
    if mod_id == crate::game::claim_borders::ID {
        buttons.push("Управление (после входа в мир)".into());
        buttons.push("Оформление интерфейса (в мире)".into());
    }
}
