fn claim_borders_command(game: &mut crate::game::Game<'_>, command: &str, ran: &mut bool) {
    use crate::game::claim_borders as m;
    if !game.native_on(m::ID) {
        m::with_state(|state| state.clear());
        return;
    }
    crate::game::claim_borders_claim_observations_view::command(game, command);
    crate::game::claim_borders_teleport_view::command(command);
    crate::game::claim_borders_teleport_view::trade_command(command);
    crate::game::claim_borders_auto_trust_view::manual(game, command);
    crate::game::claim_borders_economy_client::command(command);
    if *ran {
        return;
    }
    let (verb, body) = command.split_once(' ').unwrap_or((command, ""));
    if matches!(verb, "r" | "reply") {
        *ran = true;
        crate::game::claim_borders_chat_view::reply(game, body.trim());
        return;
    }
    if crate::game::claim_borders_client::command(game, command) {
        *ran = true;
        return;
    }
    let mut words = command.split_whitespace();
    let Some(name) = words.next() else {
        return;
    };
    if name == "claims" && words.next().is_none() {
        m::with_state(|state| state.begin_refresh());
        return; // Preserve the actual server command and its response.
    }
    if matches!(name, "claim" | "unclaim") && words.next().is_none() {
        crate::game::claim_borders_claim_memory::command(game, command);
        return;
    }
    if name != "claimborders" {
        return;
    }
    *ran = true;
    let action = words.next().unwrap_or("toggle");
    if words.next().is_some() {
        game.native_chat("[DraimHorizons] /claimborders [toggle|on|off|refresh|status]");
        return;
    }
    match action {
        "refresh" => {
            if game.remote.is_none() || game.native_player().is_none() {
                game.native_chat("[DraimHorizons] Join a server before refreshing.");
                return;
            }
            m::with_state(|state| state.begin_refresh());
            game.send_unattended_command("claims");
            game.native_chat("[DraimHorizons] Requested /claims from the server.");
        }
        "toggle" | "on" | "off" => {
            let enabled = m::with_state(|state| {
                state.enabled = match action {
                    "on" => true,
                    "off" => false,
                    _ => !state.enabled,
                };
                state.enabled
            });
            game.native_chat(if enabled {
                "[DraimHorizons] Borders ON. Run /claims or /claimborders refresh to load claims."
            } else {
                "[DraimHorizons] Borders OFF."
            });
        }
        "status" => {
            let (enabled, count) = m::with_state(|state| (state.enabled, state.count()));
            game.native_chat(&format!(
                "[DraimHorizons] Borders {}. {} claimed chunks loaded. /claims refreshes server data.",
                if enabled { "ON" } else { "OFF" }, count
            ));
        }
        _ => game.native_chat("[DraimHorizons] /claimborders [toggle|on|off|refresh|status]"),
    }
}
