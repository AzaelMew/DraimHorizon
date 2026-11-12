fn claim_borders_villager_trade(value: bool, game: &mut crate::game::Game<'_>) -> bool {
    crate::game::claim_borders_villager_trade_view::clicked(game, value);
    value
}
