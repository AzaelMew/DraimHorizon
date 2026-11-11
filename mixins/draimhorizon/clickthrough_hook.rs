fn draimhorizons_clickthrough(
    game: &mut crate::game::Game<'_>,
    button: i32,
    hand: horizon_client::first_person::Hand,
    swing: &mut bool,
    canceled: &mut bool,
) {
    crate::game::remote::claim_borders_clickthrough_view::protect_vines(
        game, button, swing, canceled,
    );
    crate::game::remote::claim_borders_clickthrough_view::remote(
        game, button, hand, swing, canceled,
    );
}
