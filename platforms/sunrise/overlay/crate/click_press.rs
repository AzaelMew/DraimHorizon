//! One click of a mouse button as the game takes a real one: a click of the `key.attack` or
//! `key.use` mapping, which the next client tick's interaction step acts on (it hits the mob
//! under the crosshair or uses the item once). Holding the button alone only goes on breaking a
//! block, so the auto-clicker would never hit a mob.
use mod_api::ClientHost;

pub fn press(host: &mut dyn ClientHost, right: bool) {
    let name = if right { "key.use" } else { "key.attack" };
    // Both names are vanilla mappings, so only a host without client input refuses them; the
    // auto-clicker then has nothing to click, as the game had nothing to take the click.
    if let Ok(mapping) = host.vanilla_key_mapping(name) {
        let _ = mapping.click(host);
    }
}
