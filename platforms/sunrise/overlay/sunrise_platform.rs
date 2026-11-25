// Sunrise's spelling of the host calls the shared sources make on `Game`.
//
// The mod is written against Horizon's `Game::native_*` helpers. Sunrise dropped the prefix
// (`on`, `setting`, `mod_screen`, ...) or keeps the data elsewhere, so each helper is forwarded here.
use super::Game;

// Horizon's Native* records are Sunrise's mod_api records (the minimap one has no counterpart since
// Nightly ed3ac19 removed the engine minimap).
pub type NativeScreenView<'a> = mod_api::ScreenView<'a>;

/// Snapshot used by the shared navigation and map code.
pub(crate) struct NativePlayer {
    pub x: f64,
    pub y: f64,
    pub z: f64,
    pub yaw: f32,
    pub pitch: f32,
    pub dimension: &'static str,
}

/// A native mod as the shared sources describe it: id, title, description and its keys
/// (name, label, default key).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct NativeMod {
    pub id: String,
    pub title: String,
    pub description: String,
    pub keys: Vec<[String; 3]>,
}

/// A system toast drawn by the mod itself, with the 32-pixel slot it took (Horizon's toast kept
/// its slot as a field).
pub(crate) struct SlotToast {
    toast: sunrise_client::toasts::SystemToast,
    pub slot: Option<usize>,
}
impl From<sunrise_client::toasts::SystemToast> for SlotToast {
    fn from(toast: sunrise_client::toasts::SystemToast) -> Self {
        Self { toast, slot: None }
    }
}
impl std::ops::Deref for SlotToast {
    type Target = sunrise_client::toasts::SystemToast;
    fn deref(&self) -> &Self::Target {
        &self.toast
    }
}
impl std::ops::DerefMut for SlotToast {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.toast
    }
}

/// Horizon's `Game::native_*` host calls, implemented on Sunrise's `Game`. An extension trait
/// because a Sunrise native mod's modules cannot add inherent methods to the game's types.
pub(crate) trait SunriseGame {
    fn native_chat(&mut self, text: &str);
    fn native_player(&self) -> Option<NativePlayer>;
    fn native_data_dir(&self, mod_id: &str) -> std::path::PathBuf;
    fn native_config_dir() -> std::io::Result<std::path::PathBuf>;
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
    /// Horizon's `game.left` / `game.right`: whether the attack (`right == false`) or use button is
    /// held, which Sunrise keeps as the `key.attack` / `key.use` mapping's `isDown`.
    fn sunrise_held(&self, right: bool) -> bool;
    /// Holds or releases that button, as Horizon's assignment to `game.left` / `game.right` did.
    fn sunrise_set_held(&mut self, right: bool, down: bool);
    /// Horizon's `game.keys.insert` / `remove` of a movement key: Sunrise's movement reads the
    /// mapping whose default key `key` is (`key.forward` for W), so this sets that mapping's
    /// `isDown` and answers whether it was down.
    fn sunrise_move_key(&mut self, key: &winit::keyboard::KeyCode, down: bool) -> bool;
    /// `ToastManager.SLOT_COUNT`.
    const TOAST_SLOTS: usize = 5;
    /// The 32-pixel toast slots the game's visible toasts hold, as bits.
    fn toast_slots_used(&self) -> u32;
    /// A tooltip's text top-left corner and box size: beside the cursor, kept on screen by
    /// `DefaultTooltipPositioner`. `extra` is [minimum width, height after the text].
    fn tooltip_geometry(
        atlas: &sunrise_client::hud::HudAtlas,
        lines: &[(crate::text::Text, u32)],
        at: (f32, f32),
        screen: (f32, f32),
        extra: [f32; 2],
    ) -> (f32, f32, f32, f32);
    /// `ClientTextTooltip.renderText` for each line: the title, then 2 pixels more before the rest.
    fn tooltip_text(
        batch: &mut sunrise_client::hud::HudBatch,
        atlas: &sunrise_client::hud::HudAtlas,
        lines: &[(crate::text::Text, u32)],
        x: f32,
        y: f32,
    );
}

impl SunriseGame for Game<'_> {
    fn native_player(&self) -> Option<NativePlayer> {
        let position = self.player.as_ref()?.position;
        Some(NativePlayer {
            x: position.x,
            y: position.y,
            z: position.z,
            yaw: self.yaw,
            pitch: self.pitch,
            dimension: self.dim.id(),
        })
    }
    fn native_data_dir(&self, mod_id: &str) -> std::path::PathBuf {
        Self::mod_data_root().join(mod_id)
    }
    fn native_config_dir() -> std::io::Result<std::path::PathBuf> {
        // Sunrise creates this shared directory at startup, before mods run.
        Ok(Self::game_dir().join("config"))
    }
    fn native_chat(&mut self, text: &str) {
        self.add_chat_line(
            crate::text::Text::literal(text),
            sunrise_client::chat::ChatSource::Client,
        );
    }
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
    fn sunrise_held(&self, right: bool) -> bool {
        self.mapped_key_down(if right { "key.use" } else { "key.attack" })
    }
    fn sunrise_set_held(&mut self, right: bool, down: bool) {
        self.set_mapping_held(if right { "key.use" } else { "key.attack" }, down);
    }
    fn sunrise_move_key(&mut self, key: &winit::keyboard::KeyCode, down: bool) -> bool {
        sunrise_client::keybinds::VanillaMappingSlot::by_default_key(*key).is_some_and(|slot| {
            std::mem::replace(&mut self.vanilla_mappings.state_mut(slot).is_down, down)
        })
    }
    fn toast_slots_used(&self) -> u32 {
        use super::toast_manager::Toast;
        self.toast_manager.visible.iter().fold(0, |used, entry| {
            // ToastManager.update: a toast holds `occupiedSlotCount` slots from its first one.
            let count = match &entry.toast {
                Toast::Tutorial(t) => t.slots(),
                Toast::Mod(t) => t.layout.slots(),
                _ => 1,
            };
            used | ((1 << count) - 1) << entry.slot
        })
    }
    fn tooltip_geometry(
        atlas: &sunrise_client::hud::HudAtlas,
        lines: &[(crate::text::Text, u32)],
        (x, y): (f32, f32),
        (width, height): (f32, f32),
        extra: [f32; 2],
    ) -> (f32, f32, f32, f32) {
        let w = lines
            .iter()
            .map(|(t, _)| atlas.font.width_text(t))
            .fold(0.0, f32::max)
            .max(extra[0]);
        // ClientTextTooltip.getHeight: 10 a line, and the title's 2 extra pixels when more follow;
        // the last line's descent (2) is not part of the box.
        let text_h = match lines.len() {
            0 => 0.0,
            1 => 8.0,
            n => 8.0 + 2.0 + (n - 1) as f32 * 10.0,
        };
        let (w, h) = (w.ceil(), (text_h + extra[1]).ceil());
        // Placed as the game places an item's own tooltip (draw_stack_tooltip_lines): the cursor
        // truncated to whole GUI pixels, then DefaultTooltipPositioner.
        let at = mod_api::ClientTooltipPositioner::position_tooltip(
            &mod_api::DefaultTooltipPositioner,
            [width as i32, height as i32],
            [x as i32, y as i32],
            [w as i32, h as i32],
        );
        (at[0] as f32, at[1] as f32, w, h)
    }
    fn tooltip_text(
        batch: &mut sunrise_client::hud::HudBatch,
        atlas: &sunrise_client::hud::HudAtlas,
        lines: &[(crate::text::Text, u32)],
        x: f32,
        y: f32,
    ) {
        let mut ty = y;
        for (i, (text, color)) in lines.iter().enumerate() {
            batch.text_styled(atlas, text, x, ty, *color, true);
            ty += if i == 0 { 12.0 } else { 10.0 };
        }
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
