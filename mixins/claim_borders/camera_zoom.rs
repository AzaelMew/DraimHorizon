use super::{Game, claim_borders, native_hooks::NativeZoom};
use horizon_client::keybinds::{key_code, mouse_button};
use std::sync::atomic::{AtomicUsize, Ordering};

pub(super) const DEFAULT_KEY: &str = "key.keyboard.z";
const FACTORS: [f32; 8] = [2.0, 2.5, 3.0, 4.0, 5.0, 6.0, 8.0, 10.0];
static LEVEL: AtomicUsize = AtomicUsize::new(3);

fn held(game: &Game<'_>) -> bool {
    if !game.native_window()
        || !game.focused
        || !game.native_playing()
        || game.screen_over_playing()
        || game.player.is_none()
        || game.sleeping.is_some()
        || game.scoping()
        || !game.native_on(claim_borders::ID)
        || (game.native_on("zoom") && game.settings.mods.installed.iter().any(|m| m.id == "zoom"))
    {
        return false;
    }
    let key = game.native_mod_key(claim_borders::ID, "zoom", DEFAULT_KEY);
    key_code(&key).is_some_and(|key| game.physical_keys.contains(&key))
        || mouse_button(&key).is_some_and(|button| game.physical_buttons.contains(&button))
}

pub(super) fn apply(game: &Game<'_>, zoom: &mut Option<NativeZoom>) {
    if zoom.is_none() && held(game) {
        let factor = FACTORS[LEVEL.load(Ordering::Relaxed)];
        *zoom = Some(NativeZoom {
            factor,
            smooth: true,
            turn: factor.recip(),
        });
    }
}

pub(super) fn scroll(game: &Game<'_>, steps: i32, canceled: &mut bool) {
    if *canceled || !held(game) {
        return;
    }
    let level = LEVEL.load(Ordering::Relaxed);
    LEVEL.store(
        level
            .saturating_add_signed(steps as isize)
            .min(FACTORS.len() - 1),
        Ordering::Relaxed,
    );
    *canceled = true;
}
