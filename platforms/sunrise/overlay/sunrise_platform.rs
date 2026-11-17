// Sunrise's spelling of the host calls the shared sources make on `Game`.
//
// The mod is written against Horizon's `Game::native_*` helpers. Sunrise dropped the prefix
// (`on`, `setting`, `mod_screen`, ...) or keeps the data elsewhere, so each helper is forwarded here.
use super::Game;

impl Game<'_> {
    /// Whether the player left the mod on; a mod turned off rests until the next build.
    pub(in crate::game) fn native_on(&self, mod_id: &str) -> bool {
        self.on(mod_id)
    }
    /// The mod's own data files are one file per key in its data folder, in both games.
    pub(in crate::game) fn native_setting(&self, mod_id: &str, key: &str) -> Option<String> {
        self.stored_setting(mod_id, key)
    }
    pub(in crate::game) fn native_set_setting(&self, mod_id: &str, key: &str, value: &str) {
        let _ = self.store_setting(mod_id, key, value);
    }
    pub(in crate::game) fn native_playing(&self) -> bool {
        self.playing()
    }
    pub(in crate::game) fn native_cursor(&self) -> [f32; 2] {
        self.cursor()
    }
    pub(in crate::game) fn native_sneaking(&self) -> bool {
        self.sneaking
    }
    pub(in crate::game) fn native_mod_screen(&self) -> Option<&str> {
        self.mod_screen()
    }
    pub(in crate::game) fn native_mod_screen_open(&mut self, mod_id: &str) -> bool {
        self.mod_screen_open(mod_id)
    }
    pub(in crate::game) fn native_mod_screen_close(&mut self, mod_id: &str) {
        self.mod_screen_close(mod_id)
    }
    /// The key the player chose for the mod's key `name`, else `default`. Modifiers are not used.
    pub(in crate::game) fn native_mod_key(&self, mod_id: &str, name: &str, default: &str) -> String {
        self.settings.mods.binding(mod_id, name, (default, 0)).0
    }
    pub(in crate::game) fn native_set_mod_key(&mut self, mod_id: &str, name: &str, key: Option<&str>) {
        match key {
            Some(key) => self.settings.mods.set_binding(mod_id, name, key, 0),
            None => self.settings.mods.reset_binding(mod_id, name),
        }
        self.settings.save(&Game::options_path());
    }
    pub(in crate::game) fn native_block(&self, pos: [i32; 3]) -> String {
        let state = self.world.block(pos[0], pos[1], pos[2]);
        self.res.facts().get(state).name.to_string()
    }
    pub(in crate::game) fn native_block_property(&self, pos: [i32; 3], key: &str) -> Option<String> {
        let state = self.world.block(pos[0], pos[1], pos[2]);
        self.pack.assets.registry.state(state)?.properties.get(key).cloned()
    }
    pub(in crate::game) fn native_client_action_bar(&mut self, text: crate::text::Text, tint: bool) {
        self.titles.set_overlay(text, tint);
    }
    pub(in crate::game) fn native_send_chat(&mut self, message: &str) -> Result<(), String> {
        self.send_chat(message)
    }
    pub(in crate::game) fn sunrise_text_width(&self, text: &str) -> f32 {
        crate::hud::HudBatch::text_width(&self.gui.atlas, text)
    }
}
