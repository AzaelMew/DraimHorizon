//! Camera zoom on the typed API: while the `zoom` key is held in play, the view zooms by the
//! chosen factor, and the mouse wheel steps through the factors instead of the hotbar.
use mod_api::{ClientHost, LivingRef, ModKeyMappingRef, Zoom};
use std::cell::Cell;
use std::sync::atomic::{AtomicUsize, Ordering};

/// The mod's `sunrise-mod.toml` id.
const ID: &str = "draimhorizon";
pub const DEFAULT_KEY: &str = "key.keyboard.z";
const FACTORS: [f32; 8] = [2.0, 2.5, 3.0, 4.0, 5.0, 6.0, 8.0, 10.0];
static LEVEL: AtomicUsize = AtomicUsize::new(3);

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
/// take the key, and the key held. The game itself fires zoom only in play and not while
/// looking through a spyglass.
fn held(host: &dyn ClientHost) -> bool {
    host.on(ID)
        && !(host.is_loaded("zoom") && host.on("zoom"))
        && host.window_focused()
        && host.playing()
        && host.observed_screen_id().is_none()
        && local_living(host).is_some_and(|living| living.is_sleeping(host) != Some(true))
        && key_down(host)
}

pub fn zoom(host: &dyn ClientHost) -> Option<Zoom> {
    held(host).then(|| {
        let factor = FACTORS[LEVEL.load(Ordering::Relaxed)];
        Zoom {
            factor,
            smooth: true,
            turn: factor.recip(),
        }
    })
}

/// Steps the factor by the wheel's whole steps; true when the wheel was used for zoom.
/// Looking through a spyglass (`LocalPlayer.isScoping`), the wheel stays the hotbar's, as the
/// zoom it would step is not shown then.
pub fn scroll(host: &dyn ClientHost, steps: i32) -> bool {
    let scoping = local_living(host).is_some_and(|living| {
        living.used_item_hand(host).is_some_and(|hand| {
            living
                .with_hand_item(host, hand, |stack| {
                    stack.item().id(host) == Some("minecraft:spyglass")
                })
                .unwrap_or(false)
        })
    });
    if scoping || !held(host) {
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
