// Inspect unwrapped system text; player chat cannot impersonate a claims reply.
fn claim_borders_chat(
    game: &mut crate::game::Game<'_>,
    content: &horizon_client::text::Text,
    source: horizon_client::chat::ChatSource,
) {
    use crate::game::claim_borders as m;
    if game.native_on(m::ID)
        && game.remote.is_some()
        && source == horizon_client::chat::ChatSource::Server
    {
        m::with_state(|state| {
            state.receive_chat(&content.plain());
        });
        crate::game::claim_borders_claim_observations_view::server(&content.plain());
        crate::game::claim_borders_claim_memory::server(&content.plain());
        crate::game::claim_borders_auto_trust_view::server(&content.plain());
        crate::game::claim_borders_economy_client::server(&content.plain());
        // A claims list is only remembered for this delivery; whether it is hidden is decided
        // when it is shown, and only for the mod's own automatic refresh.
        crate::game::claim_borders_claim_replies::on_server_line(&content.plain());
    }
}
