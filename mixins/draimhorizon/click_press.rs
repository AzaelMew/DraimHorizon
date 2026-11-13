// One click of a mouse button as the game takes a real one: on a server it goes into the
// interaction input (the click queue the next tick reads, which attacks the mob under the
// crosshair or uses the item), in single player into the tick's click count. Holding the button
// alone only goes on breaking a block, so the auto-clicker never hit a mob.
use crate::game::Game;
use horizon_client::interaction_input::Button;
pub fn press(game: &mut Game<'_>, right: bool) {
    match game.remote.as_mut() {
        Some(remote) => remote
            .input
            .press(if right { Button::Use } else { Button::Attack }),
        None if right => game.use_clicks += 1,
        None => game.attack_clicks += 1,
    }
}
