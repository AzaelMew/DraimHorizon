// Mod crate template: tools/sunrise.py smod appends `pub mod <name>;` for overlay/crate/*.rs.
// Mod crate: runs in the game process. Game-side modules may call it as `mod_draimhorizon::...`.
use mod_api::{
    self, ClientHost, Hooks, RegisterClientCommandsEvent, Registrar,
    RegistrationContext, RenderHandEvent, ScreenCharTypedPreEvent, ScreenRenderPostEvent,
};

// The relay's key-exchange crates, for the game-side relay module (the prebuilt game does not link them).
pub use chacha20poly1305;
pub use curve25519_dalek;
pub use hkdf;
pub use sha2;

#[derive(Default)]
struct Mod;

#[mod_api::horizon_mod]
impl Hooks for Mod {
    fn register(&self, registrar: &mut dyn Registrar, _: &RegistrationContext) {
        mod_entry::register(registrar);
    }
    fn register_client_commands(
        &self,
        _: &mut dyn ClientHost,
        event: &mut RegisterClientCommandsEvent<'_>,
    ) {
        mod_entry::commands(event.commands);
    }
    // Horizon builds the screen-render view (and calls `native_screen_render_post`, where the
    // `menu-render` injection draws the mod screens) only while a mod listens to the hook.
    fn screen_render_post(&self, _: &dyn ClientHost, _: &mut ScreenRenderPostEvent<'_>) {}
    // `native_screen_char_typed_pre` (menu-char) and `native_render_hand` (pat-hand) are only
    // reached while a mod listens; the hooks themselves do the work.
    fn screen_char_typed_pre(&self, _: &dyn ClientHost, _: &mut ScreenCharTypedPreEvent<'_>) -> mod_api::Flow {
        mod_api::Flow::Continue
    }
    fn render_hand(&self, _: &dyn ClientHost, _: &mut RenderHandEvent<'_, '_>) -> mod_api::Flow {
        mod_api::Flow::Continue
    }
}

/// (subject DER, SPKI DER, optional name-constraints DER) of each Mozilla root.
pub fn tls_roots_der() -> Vec<(&'static [u8], &'static [u8], Option<&'static [u8]>)> {
    webpki_roots::TLS_SERVER_ROOTS
        .iter()
        .map(|t| {
            (
                t.subject.as_ref(),
                t.subject_public_key_info.as_ref(),
                t.name_constraints.as_ref().map(|n| n.as_ref()),
            )
        })
        .collect()
}
