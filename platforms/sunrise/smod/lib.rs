// Mod crate template: tools/sunrise.py smod appends `pub mod <name>;` for overlay/crate/*.rs.
// Mod crate: runs in the game process. Game-side modules may call it as `mod_draimhorizon::...`.
use mod_api::{
    self, ClientHost, ComputeFovModifierEvent, Hooks, HumanoidSetupAnimEvent, MouseScrollingEvent,
    PlayerTurnEvent,
    RegisterClientCommandsEvent, RegisterClientTooltipComponentFactoriesEvent, Registrar,
    RegistrationContext, RenderFramePreEvent, RenderHandEvent, RenderTooltipGatherComponentsEvent,
    RenderTooltipTextureEvent, ScreenCharTypedPreEvent, ScreenRenderPostEvent,
};

#[derive(Default)]
struct Mod;

#[mod_api::sunrise_mod]
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
    fn compute_fov_modifier(&self, host: &dyn ClientHost, event: &mut ComputeFovModifierEvent) {
        camera_zoom::fov(host, &mut event.modifier);
    }
    fn player_turn(&self, host: &dyn ClientHost, event: &mut PlayerTurnEvent) {
        camera_zoom::turn(host, &mut event.x, &mut event.y);
    }
    fn render_frame_pre(&self, _: &mut dyn ClientHost, _: &mut RenderFramePreEvent) {
        camera_zoom::frame_started();
    }
    // The wheel steps the zoom factor while zoomed, in place of the hotbar.
    fn mouse_scrolling(
        &self,
        host: &mut dyn ClientHost,
        event: &mut MouseScrollingEvent,
    ) -> mod_api::Flow {
        if camera_zoom::scroll(&*host, event.accumulated[1]) {
            mod_api::Flow::Cancel
        } else {
            mod_api::Flow::Continue
        }
    }
    fn register_client_tooltip_component_factories(
        &self,
        _: &RegistrationContext,
        event: &mut RegisterClientTooltipComponentFactoriesEvent<'_>,
    ) {
        event
            .factories
            .register::<shulker_peek_tooltip::Contents, shulker_peek_tooltip::Grid>(|contents| {
                shulker_peek_tooltip::Grid(contents.0.clone())
            });
    }
    fn render_tooltip_gather_components(
        &self,
        host: &dyn ClientHost,
        event: &mut RenderTooltipGatherComponentsEvent<'_>,
    ) -> mod_api::Flow {
        shulker_peek_tooltip::gather(host, event.stack, &mut event.elements);
        mod_api::Flow::Continue
    }
    fn render_tooltip_texture(
        &self,
        host: &dyn ClientHost,
        event: &mut RenderTooltipTextureEvent<'_>,
    ) {
        shulker_peek_tooltip::tint(
            host,
            event.stack,
            &mut event.background,
            &mut event.border_top,
            &mut event.border_bottom,
        );
    }
    // Sunrise builds the screen-render view (and calls `native_screen_render_post`, where the
    // `menu-render` injection draws the mod screens) only while a mod listens to the hook.
    fn screen_render_post(&self, _: &dyn ClientHost, _: &mut ScreenRenderPostEvent<'_>) {}
    // Likewise the humanoid observer, where the `pat-arm` injection blends the patting arm.
    fn humanoid_setup_anim(&self, _: &dyn ClientHost, _: &mut HumanoidSetupAnimEvent<'_>) {}
    // `native_screen_char_typed_pre` (menu-char) and `native_render_hand` (pat-hand) are only
    // reached while a mod listens; the hooks themselves do the work.
    fn screen_char_typed_pre(
        &self,
        _: &dyn ClientHost,
        _: &mut ScreenCharTypedPreEvent<'_>,
    ) -> mod_api::Flow {
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
