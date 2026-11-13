// Access the normal model renderer without its potion early-out. The entity is borrowed
// unchanged, so resource-pack poses still see the real server metadata and effects.
pub fn render(
    mob: &crate::mobs::Mob,
    alpha: f32,
    camera: glam::DVec3,
    atlas: super::Atlas,
    light: u32,
    data: &crate::mobs::MobData,
    out: &mut Vec<crate::mesher::Vertex>,
) {
    super::render_visible(
        mob,
        alpha,
        super::base_transform(mob, alpha, camera),
        atlas,
        light,
        data,
        &[],
        true,
        out,
        None,
    );
}

#[cfg(test)]
mod tests {
    use crate::mobs::{Mob, MobData, Part};
    use horizon_core::geometry::Vec3;
    use std::sync::Arc;

    #[test]
    fn invisible_players_and_mobs_draw_without_changing_effects_or_metadata() {
        // An authored quad, not an extracted Minecraft model or game asset.
        let model = Arc::new(Part::parse(
            "root",
            &serde_json::json!({
                "polygons": [[[0,0,0,0,0], [1,0,0,1,0], [1,1,0,1,1], [0,1,0,0,1]]]
            }),
        ));
        let data = MobData {
            states: Vec::new(),
            placements: Default::default(),
            heightmaps: Default::default(),
            animations: Default::default(),
            biome_tags: Default::default(),
            entity_tags: Default::default(),
            villager_hats: Default::default(),
            cem: Default::default(),
            random_textures: Default::default(),
            emissive: Default::default(),
            blinks: Default::default(),
            pois: Default::default(),
            species_placements: Default::default(),
        };
        let atlas = |_: &str| Some([0.0, 0.0, 1.0, 1.0]);
        for id in ["minecraft:player", "minecraft:cow"] {
            let mut kind = (*crate::mobs::sensor_scan_tests::kind()).clone();
            kind.id = id.into();
            kind.model = model.clone();
            let mut mob = Mob::new(42, Arc::new(kind), Vec3::ZERO, 0.0);
            mob.invisible = true;
            mob.add_effect("minecraft:invisibility", 600, 0);
            let effects = mob.extra.effects.clone();
            let mut normal = Vec::new();
            super::super::render(
                &mob,
                0.5,
                glam::DVec3::ZERO,
                &atlas,
                0xf0f0,
                &data,
                &mut normal,
            );
            assert!(normal.is_empty());
            let mut revealed = Vec::new();
            super::render(
                &mob,
                0.5,
                glam::DVec3::ZERO,
                &atlas,
                0xf0f0,
                &data,
                &mut revealed,
            );
            assert!(!revealed.is_empty(), "{id} must have visible body geometry");
            assert!(mob.invisible);
            assert_eq!(mob.extra.effects, effects);
        }
    }
}
