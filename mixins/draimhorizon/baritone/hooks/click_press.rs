// A mouse button pressed, on any screen: the click screen of `#click` remembers the block it went down on.
fn baritone_click_press(game: &mut crate::game::Game<'_>) {
    crate::game::baritone::click_press(game);
}
