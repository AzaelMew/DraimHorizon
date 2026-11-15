// A chat line starting with Baritone's prefix (#) is a Baritone command: the server never sees it.
fn baritone_chat(game: &mut crate::game::Game<'_>, message: &str) -> std::ops::ControlFlow<()> {
    use crate::game::baritone as baritone;
    if baritone::enabled(game) && baritone::chat(game, message) {
        std::ops::ControlFlow::Break(())
    } else {
        std::ops::ControlFlow::Continue(())
    }
}
