fn draimhorizons_clickthrough_local(
    game: &mut crate::game::Game<'_>,
) -> core::ops::ControlFlow<horizon_client::Result<Option<horizon_client::first_person::Hand>>> {
    match crate::game::remote::claim_borders_clickthrough_view::local(game) {
        Some(result) => core::ops::ControlFlow::Break(result),
        None => core::ops::ControlFlow::Continue(()),
    }
}
