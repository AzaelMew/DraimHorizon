// /b and /baritone, before the server and Horizon's commands see them.
fn baritone_command(game: &mut crate::game::Game<'_>, command: &str, ran: &mut bool) {
    use crate::game::baritone as baritone;
    if !*ran && baritone::enabled(game) {
        baritone::command(game, command, ran);
    }
}
