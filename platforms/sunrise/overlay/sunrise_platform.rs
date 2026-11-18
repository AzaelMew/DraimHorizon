// Sunrise's spelling of the host calls the shared sources make on `Game`.
//
// The mod is written against Horizon's `Game::native_*` helpers. Sunrise dropped the prefix
// (`on`, `setting`, `mod_screen`, ...) or keeps the data elsewhere, so each helper is forwarded here.
use super::Game;

// Horizon's Native* records are Sunrise's mod_api records.
pub type NativeZoom = mod_api::Zoom;
pub type NativeMinimap = mod_api::Minimap;
pub type NativeScreenView<'a> = mod_api::ScreenView<'a>;

/// A native mod as the shared sources describe it: id, title, description and its keys
/// (name, label, default key).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct NativeMod {
    pub id: String,
    pub title: String,
    pub description: String,
    pub keys: Vec<[String; 3]>,
}

/// Horizon's `Game::native_*` host calls, implemented on Sunrise's `Game`. An extension trait
/// because a Sunrise native mod's modules cannot add inherent methods to the game's types.
pub(crate) trait SunriseGame {
    /// Whether the player left the mod on; a mod turned off rests until the next build.
    fn native_on(&self, mod_id: &str) -> bool;
    /// The mod's own data files are one file per key in its data folder, in both games.
    fn native_setting(&self, mod_id: &str, key: &str) -> Option<String>;
    fn native_set_setting(&self, mod_id: &str, key: &str, value: &str);
    fn native_playing(&self) -> bool;
    fn native_cursor(&self) -> [f32; 2];
    fn native_sneaking(&self) -> bool;
    fn native_mod_screen(&self) -> Option<&str>;
    fn native_mod_screen_open(&mut self, mod_id: &str) -> bool;
    fn native_mod_screen_close(&mut self, mod_id: &str);
    /// The key the player chose for the mod's key `name`, else `default`. Modifiers are not used.
    fn native_mod_key(&self, mod_id: &str, name: &str, default: &str) -> String;
    fn native_set_mod_key(&mut self, mod_id: &str, name: &str, key: Option<&str>);
    fn native_block(&self, pos: [i32; 3]) -> String;
    fn native_block_property(&self, pos: [i32; 3], key: &str) -> Option<String>;
    fn native_client_action_bar(&mut self, text: crate::text::Text, tint: bool);
    fn native_send_chat(&mut self, message: &str) -> Result<(), String>;
    fn sunrise_text_width(&self, text: &str) -> f32;
}

impl SunriseGame for Game<'_> {
    /// Whether the player left the mod on; a mod turned off rests until the next build.
    fn native_on(&self, mod_id: &str) -> bool {
        self.on(mod_id)
    }
    /// The mod's own data files are one file per key in its data folder, in both games.
    fn native_setting(&self, mod_id: &str, key: &str) -> Option<String> {
        let path = self.native_data_dir(mod_id).join(key);
        std::fs::read_to_string(path)
            .ok()
            .map(|v| v.trim().to_owned())
    }
    fn native_set_setting(&self, mod_id: &str, key: &str, value: &str) {
        let dir = self.native_data_dir(mod_id);
        let _ = std::fs::create_dir_all(&dir).and_then(|()| std::fs::write(dir.join(key), value));
    }
    fn native_playing(&self) -> bool {
        self.playing()
    }
    fn native_cursor(&self) -> [f32; 2] {
        self.cursor()
    }
    fn native_sneaking(&self) -> bool {
        self.keys.held(sunrise_client::actions::Action::Sneak)
    }
    fn native_mod_screen(&self) -> Option<&str> {
        self.mod_screen()
    }
    fn native_mod_screen_open(&mut self, mod_id: &str) -> bool {
        self.mod_screen_open(mod_id)
    }
    fn native_mod_screen_close(&mut self, mod_id: &str) {
        self.mod_screen_close(mod_id)
    }
    /// The key the player chose for the mod's key `name`, else `default`. Modifiers are not used.
    fn native_mod_key(&self, mod_id: &str, name: &str, default: &str) -> String {
        self.settings.mods.binding(mod_id, name, (default, 0)).0
    }
    fn native_set_mod_key(&mut self, mod_id: &str, name: &str, key: Option<&str>) {
        match key {
            Some(key) => self.settings.mods.set_binding(mod_id, name, key, 0),
            None => self.settings.mods.reset_binding(mod_id, name),
        }
        self.settings.save(&Game::options_path());
    }
    fn native_block(&self, pos: [i32; 3]) -> String {
        let state = self.world.block(pos[0], pos[1], pos[2]);
        self.res.facts().get(state).name.to_string()
    }
    fn native_block_property(&self, pos: [i32; 3], key: &str) -> Option<String> {
        let state = self.world.block(pos[0], pos[1], pos[2]);
        self.pack
            .assets
            .registry
            .state(state)?
            .properties
            .get(key)
            .cloned()
    }
    fn native_client_action_bar(&mut self, text: crate::text::Text, tint: bool) {
        self.titles.set_overlay(text, tint);
    }
    fn native_send_chat(&mut self, message: &str) -> Result<(), String> {
        self.send_chat(message)
    }
    fn sunrise_text_width(&self, text: &str) -> f32 {
        crate::hud::HudBatch::text_width(&self.gui.atlas, text)
    }
}

/// The plain characters of an API text: literals and translation fallbacks, children appended.
pub fn api_plain(text: &mod_api::Text) -> String {
    let mut out = String::new();
    match &text.content {
        mod_api::text::TextContent::Literal(literal) => out.push_str(literal),
        mod_api::text::TextContent::Translatable { key, fallback, .. } => {
            out.push_str(fallback.as_deref().unwrap_or(key));
        }
        _ => {}
    }
    for child in &text.children {
        out.push_str(&api_plain(child));
    }
    out
}
