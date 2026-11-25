//! Camera zoom on the typed API: while the `zoom` key is held in play, the view zooms by the
//! chosen factor, and the mouse wheel steps through the factors instead of the hotbar.
use mod_api::{ClientHost, LivingRef, ModKeyMappingRef};
use std::cell::Cell;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};

/// The mod's `sunrise-mod.toml` id.
const ID: &str = "draimhorizon";
pub const DEFAULT_KEY: &str = "key.keyboard.z";
const FACTORS: [f32; 8] = [2.0, 2.5, 3.0, 4.0, 5.0, 6.0, 8.0, 10.0];
static LEVEL: AtomicUsize = AtomicUsize::new(3);
/// Whether this mod supplied a FOV modifier in the last frame. The camera hook
/// skips first-person spyglasses and non-player cameras.
static SUPPLIED: AtomicBool = AtomicBool::new(false);

thread_local! {
    /// The `zoom` mapping's handle, resolved again when a new registry generation refuses it.
    static KEY: Cell<Option<ModKeyMappingRef>> = const { Cell::new(None) };
}

fn key_down(host: &dyn ClientHost) -> bool {
    KEY.with(|key| {
        if let Some(mapping) = key.get()
            && let Ok(down) = mapping.is_down(host)
        {
            return down;
        }
        let Ok(mapping) = host.mod_key_mapping(ID, "zoom") else {
            return false;
        };
        key.set(Some(mapping));
        mapping.is_down(host).unwrap_or(false)
    })
}

/// The local player as a living handle.
fn local_living(host: &dyn ClientHost) -> Option<LivingRef> {
    host.local_player()
        .and_then(|player| host.living(player.entity()))
}

/// In play with nothing over the world, the player awake, the mod on and no `zoom` mod on to
/// take the key, and the key held. A first-person spyglass keeps its own FOV and turn speed.
fn held(host: &dyn ClientHost) -> bool {
    host.on(ID)
        && !(host.is_loaded("zoom") && host.on("zoom"))
        && host.window_focused()
        && host.playing()
        && host.observed_screen_id().is_none()
        && local_living(host).is_some_and(|living| living.is_sleeping(host) != Some(true))
        && !scoping(host)
        && key_down(host)
}

fn scoping(host: &dyn ClientHost) -> bool {
    host.camera_type().is_first_person()
        && host.local_item_use().is_some_and(|using| {
            local_living(host).and_then(|living| {
                living.with_hand_item(host, using.hand, |stack| {
                    mod_api::item_id(stack.item()) == Some("minecraft:spyglass")
                })
            }) == Some(true)
        })
}

pub fn fov(host: &dyn ClientHost, modifier: &mut f32) {
    let active = held(host);
    if active {
        *modifier /= FACTORS[LEVEL.load(Ordering::Relaxed)];
    }
    SUPPLIED.store(active, Ordering::Relaxed);
}

pub fn turn(host: &dyn ClientHost, x: &mut f64, y: &mut f64) {
    if SUPPLIED.load(Ordering::Relaxed) && held(host) {
        let scale = f64::from(FACTORS[LEVEL.load(Ordering::Relaxed)]).recip();
        *x *= scale;
        *y *= scale;
    }
}

/// The start of a frame: FOV extraction sets whether this mod supplied a zoom.
pub fn frame_started() {
    SUPPLIED.store(false, Ordering::Relaxed);
}

/// Steps the factor by the wheel's whole steps; true when the wheel was used for zoom.
/// Only after this mod supplied a FOV modifier, with no competing zoom mod or spyglass.
pub fn scroll(host: &dyn ClientHost, steps: i32) -> bool {
    if !SUPPLIED.load(Ordering::Relaxed) || !held(host) {
        return false;
    }
    let level = LEVEL.load(Ordering::Relaxed);
    LEVEL.store(
        level
            .saturating_add_signed(steps as isize)
            .min(FACTORS.len() - 1),
        Ordering::Relaxed,
    );
    true
}
