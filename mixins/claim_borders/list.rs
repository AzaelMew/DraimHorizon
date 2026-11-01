fn claim_borders_list(mods: &mut Vec<crate::game::native_hooks::NativeMod>) {
    use crate::game::claim_borders as m;
    mods.push(crate::game::native_hooks::NativeMod {
        id: m::ID.into(),
        title: format!("{} {}", m::NAME, m::VERSION),
        description: m::DESCRIPTION.into(),
        keys: vec![[
            "menu".into(),
            "Claim Borders: открыть меню".into(),
            "key.keyboard.m".into(),
        ]],
    });
}
