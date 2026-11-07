use crate::game::{Game, claim_borders as claims, claim_borders_server_tps::Sampler};
use std::{
    sync::{Mutex, OnceLock},
    time::Instant,
};
struct State {
    clock: Instant,
    sampler: Sampler,
}
impl Default for State {
    fn default() -> Self {
        Self {
            clock: Instant::now(),
            sampler: Sampler::default(),
        }
    }
}
fn state<T>(f: impl FnOnce(&mut State) -> T) -> T {
    static S: OnceLock<Mutex<State>> = OnceLock::new();
    f(&mut S
        .get_or_init(|| Mutex::new(State::default()))
        .lock()
        .unwrap_or_else(|e| e.into_inner()))
}
pub(super) fn reset() {
    state(|s| *s = State::default());
}
pub(super) fn packet(game: &Game<'_>, packet: &horizon_protocol::packets::play::Clientbound) {
    use horizon_protocol::packets::play::Clientbound as C;
    if !matches!(packet, C::SetTime(_) | C::TickingState(_) | C::Respawn(_))
        || !game.native_on(claims::ID)
        || game.remote.is_none()
    {
        return;
    }
    state(|s| match packet {
        C::SetTime(p) => s
            .sampler
            .sample(s.clock.elapsed().as_millis() as u64, p.game_time),
        C::TickingState(p) => s.sampler.rate(p.rate, p.frozen),
        C::Respawn(_) => s.sampler = Sampler::default(),
        _ => {}
    });
}
pub(super) fn label() -> String {
    state(|s| {
        if s.sampler.frozen {
            crate::game::claim_borders_chat_view::current_language()
                .text("TPS: пауза", "TPS: paused")
                .into()
        } else {
            s.sampler
                .value(s.clock.elapsed().as_millis() as u64)
                .map_or("TPS: —".into(), |n| format!("TPS ≈ {n:.1}"))
        }
    })
}
