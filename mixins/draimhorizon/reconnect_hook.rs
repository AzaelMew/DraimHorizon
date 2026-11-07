fn draim_reconnect(
    game: &mut crate::game::Game<'_>,
    reason: &str,
    key: Option<&str>,
) -> core::ops::ControlFlow<Option<Retry>> {
    if !game.native_on(crate::game::claim_borders::ID) {
        return core::ops::ControlFlow::Continue(());
    }
    if !game.settings.extras.auto_reconnect
        || !crate::game::claim_borders_reconnect::allowed(reason, key)
    {
        game.extras.reconnect.reset();
        return core::ops::ControlFlow::Break(None);
    }
    // Keep Horizon's address/account, cancel button, stability reset, backoff and attempt cap.
    core::ops::ControlFlow::Break(game.extras.reconnect.ended(
        "Connection lost",
        None,
        false,
        std::time::Instant::now(),
    ))
}
