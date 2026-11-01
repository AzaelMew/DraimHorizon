fn claim_borders_command(game: &mut crate::game::Game<'_>, command: &str, ran: &mut bool) {
    use crate::game::claim_borders as m;
    if !game.native_on(m::ID) {
        m::with_state(|state| state.clear());
        return;
    }
    if *ran {
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
        m::with_state(|state| state.clear());
        game.native_chat("[Claim Borders] Claims may change. Run /claims to refresh borders.");
        return;
    }
    if name != "claimborders" {
        return;
    }
    *ran = true;
    let action = words.next().unwrap_or("toggle");
    if words.next().is_some() {
        game.native_chat("[Claim Borders] /claimborders [toggle|on|off|refresh|status]");
        return;
    }
    match action {
        "refresh" => {
            if game.remote.is_none() || game.native_player().is_none() {
                game.native_chat("[Claim Borders] Join a server before refreshing.");
                return;
            }
            m::with_state(|state| state.begin_refresh());
            game.send_unattended_command("claims");
            game.native_chat("[Claim Borders] Requested /claims from the server.");
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
                "[Claim Borders] Borders ON. Run /claims or /claimborders refresh to load claims."
            } else {
                "[Claim Borders] Borders OFF."
            });
        }
        "status" => {
            let (enabled, count) = m::with_state(|state| (state.enabled, state.count()));
            game.native_chat(&format!(
                "[Claim Borders] Borders {}. {} claimed chunks loaded. /claims refreshes server data.",
                if enabled { "ON" } else { "OFF" }, count
            ));
        }
        _ => game.native_chat("[Claim Borders] /claimborders [toggle|on|off|refresh|status]"),
    }
}
