fn claim_borders_list(mods: &mut Vec<crate::game::native_hooks::NativeMod>) {
    use crate::game::claim_borders as m;
    mods.push(crate::game::native_hooks::NativeMod {
        id: m::ID.into(),
        title: format!("{} {}", m::NAME, m::VERSION),
        description: m::DESCRIPTION.into(),
        keys: vec![
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
                "Карта приватов".into(),
                "key.keyboard.j".into(),
            ],
            [
                "menu".into(),
                "DraimHorizons: открыть меню".into(),
                "key.keyboard.m".into(),
            ],
        ],
    });
}
