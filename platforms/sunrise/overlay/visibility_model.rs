// Draws a body the normal renderer skips for its invisibility effect, opaque like Horizon does.
// The normal model pass early-outs on `is_invisible` (and draws a spectator body translucent), so
// the borrowed entity is cloned with those two flags cleared for this one draw; the real entity,
// its effects and its server metadata stay untouched.
pub fn render(
    mob: &sunrise_client::mobs::Mob,
    alpha: f32,
    camera: glam::DVec3,
    atlas: sunrise_client::mobs::render::Atlas<'_>,
    light: u32,
    data: &sunrise_client::mobs::MobData,
    out: &mut Vec<sunrise_client::mesher::Vertex>,
) {
    let mut shown = mob.clone();
    shown.invisible = false;
    shown.extra.spectator = false;
    sunrise_client::mobs::render::render_with_hidden(
        &shown,
        alpha,
        camera,
        atlas,
        light,
        data,
        &[],
        Default::default(),
        None,
        out,
    );
}
