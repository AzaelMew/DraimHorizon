//! Camera zoom on the typed API: while the `zoom` key is held in play, the view zooms by the
//! chosen factor, and the mouse wheel steps through the factors instead of the hotbar.
use mod_api::{ClientHost, LivingRef, ModKeyMappingRef};
use std::cell::{Cell, RefCell};
use std::sync::OnceLock;

/// The mod's `sunrise-mod.toml` id.
const ID: &str = "draimhorizon";
pub const DEFAULT_KEY: &str = "key.keyboard.z";

thread_local! {
    /// The `zoom` mapping's handle, resolved again when a new registry generation refuses it.
    static KEY: Cell<Option<ModKeyMappingRef>> = const { Cell::new(None) };
    static ZOOM: RefCell<crate::zoom_state::Zoom> = RefCell::new(Default::default());
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

/// In play with nothing over the world, the player awake and no competing zoom mod.
/// A first-person spyglass keeps its own FOV and turn speed.
fn allowed(host: &dyn ClientHost) -> bool {
    host.on(ID)
        && !(host.is_loaded("zoom") && host.on("zoom"))
        && host.window_focused()
        && host.playing()
        && host.observed_screen_id().is_none()
        && local_living(host).is_some_and(|living| living.is_sleeping(host) != Some(true))
        && !scoping(host)
}

fn scoping(host: &dyn ClientHost) -> bool {
    static SPYGLASS: OnceLock<mod_api::Identifier> = OnceLock::new();
    host.camera_type().is_first_person()
        && host.local_item_use().is_some_and(|using| {
            local_living(host).and_then(|living| {
                living.with_hand_item(host, using.hand, |stack| {
                    let id = SPYGLASS.get_or_init(|| {
                        mod_api::Identifier::parse("minecraft:spyglass")
                            .expect("valid spyglass identifier")
                    });
                    host.item_key(id) == Some(stack.item())
                })
            }) == Some(true)
        })
}

pub fn fov(host: &dyn ClientHost, degrees: &mut f32) {
    if allowed(host) {
        *degrees = (*degrees * ZOOM.with(|zoom| zoom.borrow().scale())).max(1.0);
    }
}

pub fn turn(host: &dyn ClientHost, x: &mut f64, y: &mut f64) {
    if allowed(host) {
        let scale = f64::from(ZOOM.with(|zoom| zoom.borrow().scale()));
        *x *= scale;
        *y *= scale;
    }
}

/// Advance once per rendered frame; FOV can be queried several times or not at all.
pub fn frame(host: &dyn ClientHost, elapsed: std::time::Duration) {
    let allowed = allowed(host);
    let held = allowed && key_down(host);
    ZOOM.with(|zoom| zoom.borrow_mut().frame(elapsed.as_secs_f32(), allowed, held));
}

/// Steps the factor by the wheel's whole steps; true when the wheel was used for zoom.
/// Key state decides ownership even before the first frame or between game ticks.
pub fn scroll(host: &dyn ClientHost, steps: i32) -> bool {
    let held = allowed(host) && key_down(host);
    ZOOM.with(|zoom| zoom.borrow_mut().scroll(steps, held))
}
