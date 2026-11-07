fn claim_borders_list(mods: &mut Vec<crate::game::native_hooks::NativeMod>) {
    use crate::game::claim_borders as m;
    let language = crate::game::claim_borders_chat_view::current_language();
    mods.push(crate::game::native_hooks::NativeMod {
        id: m::ID.into(),
        title: format!("{} {}", m::NAME, m::VERSION),
        description: m::description_in(language).into(),
        keys: vec![
            [
                "ore_experiment".into(),
                language
                    .text("Эксперименты: подсветка руд", "Experiments: ore highlight")
                    .into(),
                crate::game::claim_borders_experimental::DEFAULT_KEY.into(),
            ],
            [
                "trades".into(),
                language
                    .text("DraimHorizons: Торги", "DraimHorizons: Trades")
                    .into(),
                "key.keyboard.b".into(),
            ],
            [
                "waypoint".into(),
                language
                    .text(
                        "DraimHorizons: метка у себя",
                        "DraimHorizons: waypoint here",
                    )
                    .into(),
                "key.keyboard.x".into(),
            ],
            [
                "map".into(),
                language
                    .text("Seed Map · три измерения", "Seed Map · three dimensions")
                    .into(),
                "key.keyboard.m".into(),
            ],
            [
                "menu".into(),
                language
                    .text("DraimHorizons: открыть меню", "DraimHorizons: open menu")
                    .into(),
                "key.keyboard.j".into(),
            ],
            [
                "zoom".into(),
                language
                    .text("DraimHorizons: зум камеры", "DraimHorizons: camera zoom")
                    .into(),
                crate::game::claim_borders_camera_zoom::DEFAULT_KEY.into(),
            ],
        ],
    });
}
