//! Client command names, Controls rows and the settings button of Options > Mods.
use mod_api::{Command, KeyRegistration, Registrar, SettingSpec};

pub const COMMANDS: [&str; 9] = [
    "claimborders",
    "horizonmenu",
    "party",
    "friend",
    "wp",
    "waypoint",
    "r",
    "reply",
    "dfarm",
];

/// (id, default key, label).
const KEYS: [(&str, &str, &str); 22] = [
    (
        "ore_experiment",
        "key.keyboard.o",
        "Experiments: ore highlight",
    ),
    ("trades", "key.keyboard.b", "DraimHorizons: Trades"),
    (
        "waypoint",
        "key.keyboard.x",
        "DraimHorizon: waypoint / coordinates",
    ),
    ("map", "key.keyboard.m", "Seed Map · three dimensions"),
    ("menu", "key.keyboard.j", "DraimHorizons: open menu"),
    ("pat", "key.keyboard.k", "DraimHorizons: pat player"),
    (
        "autoclick",
        "key.keyboard.unknown",
        "DraimHorizons: auto click",
    ),
    ("zoom", "key.keyboard.z", "DraimHorizons: camera zoom"),
    (
        "navigation_preview",
        "key.keyboard.keypad.1",
        "Baritone: preview",
    ),
    (
        "navigation_start",
        "key.keyboard.keypad.2",
        "Baritone: start / resume",
    ),
    (
        "navigation_pause",
        "key.keyboard.keypad.3",
        "Baritone: pause",
    ),
    ("navigation_stop", "key.keyboard.keypad.4", "Baritone: stop"),
    (
        "schematic_toggle",
        "key.keyboard.insert",
        "DraimHorizons: Schematic: show/hide",
    ),
    (
        "schematic_remove",
        "key.keyboard.delete",
        "DraimHorizons: Schematic: remove preview",
    ),
    (
        "schematic_rotate",
        "key.keyboard.home",
        "DraimHorizons: Schematic: rotate",
    ),
    (
        "schematic_here",
        "key.keyboard.end",
        "DraimHorizons: Schematic: move to me",
    ),
    (
        "schematic_forward",
        "key.keyboard.up",
        "DraimHorizons: Schematic: forward",
    ),
    (
        "schematic_back",
        "key.keyboard.down",
        "DraimHorizons: Schematic: back",
    ),
    (
        "schematic_left",
        "key.keyboard.left",
        "DraimHorizons: Schematic: left",
    ),
    (
        "schematic_right",
        "key.keyboard.right",
        "DraimHorizons: Schematic: right",
    ),
    (
        "schematic_up_down",
        "key.keyboard.page.up",
        "DraimHorizons: Schematic: up (Shift: down)",
    ),
    (
        "schematic_down",
        "key.keyboard.page.down",
        "DraimHorizons: Schematic: down",
    ),
];

pub fn register(registrar: &mut dyn Registrar) {
    for (id, key, label) in KEYS {
        registrar.register_key_mapping(KeyRegistration::new(id, key).label(label));
    }
    // A button in effect: the game-side hook turns it back off and opens the menu.
    let _ = registrar.register_setting(SettingSpec::toggle(
        "open_menu",
        "DraimHorizons settings (in world)",
        false,
    ));
}

pub fn commands(out: &mut Vec<Command>) {
    out.extend(COMMANDS.iter().map(|name| Command::new(name)));
}
