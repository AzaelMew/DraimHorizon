use super::{Game, claim_borders as claims, claim_borders_visibility as visibility};
use horizon_client::{mesher::Vertex, mobs::Species};

pub fn render(game: &Game<'_>, camera: glam::DVec3, alpha: f64, out: &mut Vec<Vertex>) {
    if !game.native_window()
        || !visibility::enabled(
            game.native_on(claims::ID),
            game.native_setting(claims::SETTINGS_ID, "show_invisible_entities")
                .as_deref(),
        )
    {
        return;
    }
    let atlas = &game.gui.atlas;
    let lookup = |name: &str| atlas.sprites.get(name).map(|r| [r.u0, r.v0, r.u1, r.v1]);
    let scale = game.view_scale();
    for mob in &game.mobs {
        if !visibility::reveal(
            mob.invisible,
            mob.has_effect("minecraft:invisibility"),
            mob.removed,
            game.hides_as_camera(mob.id),
        ) {
            continue;
        }
        let (w, h, _) = mob.dims();
        let player = &*mob.kind.id == "minecraft:player";
        let range = (w + h + w) / 3.0 * if player { 10.0 } else { 1.0 } * 64.0 * scale;
        let position = glam::DVec3::new(mob.position.x, mob.position.y, mob.position.z);
        if (position - camera).length_squared() >= range * range {
            continue;
        }
        if let Some(view) = &game.view_frustum {
            let (lo, hi) = super::entity_culling::cull_box(mob, alpha);
            let lo = (lo - camera - glam::DVec3::splat(0.5)).as_vec3();
            let hi = (hi - camera + glam::DVec3::splat(0.5)).as_vec3();
            if !view.intersects(lo, hi) {
                continue;
            }
        }
        let probe = mob.light_probe(alpha);
        let light = game.world.light(probe[0], probe[1], probe[2]);
        let mut packed = (u32::from(light >> 4) * 16) | ((u32::from(light & 15) * 16) << 8);
        if mob.fire_ticks > 0
            || matches!(
                mob.species(),
                Species::Allay
                    | Species::Blaze
                    | Species::MagmaCube
                    | Species::Vex
                    | Species::Wither
            )
        {
            packed = (packed & 0xFF) | ((15 * 16) << 8);
        }
        let start = out.len();
        game.mark_motion(start, (1 << 32) | u64::from(mob.id));
        game.mark_entity(start, &mob.kind.id);
        // A player's worn armour and held items remain in the original pass; restore
        // the missing model here with normal depth testing, not a through-wall overlay.
        horizon_client::mobs::render::claim_borders_visibility_model::render(
            mob,
            alpha as f32,
            camera,
            &lookup,
            packed,
            game.res.mob_data(),
            out,
        );
        game.cull_entity_vertices(out, start);
    }
    game.mark_motion(out.len(), 0);
}
