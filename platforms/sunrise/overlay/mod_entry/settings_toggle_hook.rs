fn claim_borders_settings_toggle(
    game: &mut crate::game::Game<'_>,
    mod_id: &str,
    spec: &mod_api::SettingSpec,
    value: mod_api::SettingValue,
) -> core::ops::ControlFlow<Result<bool, SettingAccessError>> {
    if mod_id != crate::game::claim_borders::ID
        || spec.key != "open_menu"
        || value != mod_api::SettingValue::Toggle(true)
    {
        return ::core::ops::ControlFlow::Continue(());
    }
    // A button: intercept the attempted true write, leaving its live value off.
    crate::game::claim_borders_client::open_menu_from_options(game);
    crate::game::claim_borders_menu_view::open_settings();
    core::ops::ControlFlow::Break(Ok(false))
}
