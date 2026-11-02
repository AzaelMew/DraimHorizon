// Child of game::scoreboard, with the same display names and number formats as the HUD.
use super::*;
pub(in crate::game) fn read(game: &Game<'_>) -> Vec<(String, String)> {
    let board = &game.score.board;
    let Some(objective) = game.sidebar_objective() else {
        return Vec::new();
    };
    let format =
        objective.format_or_default(horizon_client::scoreboard::NumberFormat::sidebar_default());
    let mut scores = board.list_scores(&objective.name);
    scores.sort_by(|a, b| {
        b.value.cmp(&a.value).then_with(|| {
            a.owner
                .to_ascii_lowercase()
                .cmp(&b.owner.to_ascii_lowercase())
        })
    });
    scores
        .into_iter()
        .filter(|e| !e.is_hidden())
        .take(15)
        .map(|e| {
            let name = match board.players_team(&e.owner) {
                Some(team) => team.formatted_name(e.owner_name()),
                None => e.owner_name(),
            }
            .resolve_legacy()
            .plain();
            let value = e.format_value(&format).resolve_legacy().plain();
            (name, value)
        })
        .collect()
}

pub(in crate::game) fn prepare_tab(
    game: &mut Game<'_>,
) -> Vec<horizon_client::tab_list::TabPlayer> {
    game.flush_skin_slots();
    let Some((players, _, _)) = game.remote_tab_players() else {
        return Vec::new();
    };
    if game.tab_list.set_visible(true) {
        game.say_system_now(&horizon_client::tab_list::narration(
            &players,
            &game.score.board,
        ));
    }
    players
}
