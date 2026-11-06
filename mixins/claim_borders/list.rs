fn claim_borders_list(mods: &mut Vec<crate::game::native_hooks::NativeMod>) {
    use crate::game::claim_borders as m;
    mods.push(crate::game::native_hooks::NativeMod {
        id: m::ID.into(),
        title: format!("{} {}", m::NAME, m::VERSION),
        description: m::DESCRIPTION.into(),
        keys: vec![
            [
                "ore_experiment".into(),
                "Эксперименты: подсветка руд".into(),
                crate::game::claim_borders_experimental::DEFAULT_KEY.into(),
            ],
            [
                "trades".into(),
                "DraimHorizons: Торги".into(),
                "key.keyboard.b".into(),
            ],
            [
                "waypoint".into(),
                "DraimHorizons: метка у себя".into(),
                "key.keyboard.x".into(),
            ],
            [
                "map".into(),
                "Seed Map · три измерения".into(),
                "key.keyboard.m".into(),
            ],
            [
                "menu".into(),
                "DraimHorizons: открыть меню".into(),
                "key.keyboard.j".into(),
            ],
            [
                "zoom".into(),
                "DraimHorizons: зум камеры".into(),
                crate::game::claim_borders_camera_zoom::DEFAULT_KEY.into(),
            ],
        ],
    });
}
