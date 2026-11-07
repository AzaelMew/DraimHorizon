// Append outer claim cages to Horizon's depth-tested or always-on-top lines.
fn claim_borders_render(game: &crate::game::Game<'_>, gizmos: &mut horizon_client::gizmos::Gizmos) {
    use crate::game::{
        claim_borders as m, claim_borders_client as client, claim_borders_visualization as visual,
    };
    if !game.native_on(m::ID) {
        m::with_state(|state| state.clear());
        return;
    }
    crate::game::claim_borders_ore_view::render(game, gizmos);
    crate::game::claim_borders_navigation_view::render(game, gizmos);
    if game.title.is_some() || game.ui.hide_gui || !m::with_state(|state| state.enabled) {
        return;
    }
    let Some(player) = game.native_player() else {
        return;
    };
    let settings = client::visual_settings();
    let dimension = if player.dimension.contains(':') {
        player.dimension.to_owned()
    } else {
        format!("minecraft:{}", player.dimension)
    };
    let position = [player.x, player.y, player.z];
    let own = if settings.own_visible {
        m::with_state(|state| state.shared_claims()).unwrap_or_default()
    } else {
        Vec::new()
    };
    let friends = if settings.friends_visible {
        client::territories()
    } else {
        Vec::new()
    };
    let known: std::collections::BTreeSet<_> = own.iter().chain(friends.iter()).cloned().collect();
    let observed = if settings.friends_visible {
        client::stranger_territories()
            .into_iter()
            .filter(|c| !known.contains(c))
            .collect::<Vec<_>>()
    } else {
        Vec::new()
    };
    for (claims, color) in [
        (&friends, settings.styled_color(true)),
        (&own, settings.styled_color(false)),
        (
            &observed,
            (settings.styled_color(true) & 0xff000000) | 0xe45b65,
        ),
    ] {
        let segments = visual::geometry_with_spacing(
            claims
                .iter()
                .filter(|c| c.dimension == dimension)
                .map(|c| (c.x, c.z)),
            position,
            &settings,
            crate::game::claim_borders_map_view::settings(game).grid_step,
        );
        for (start, end) in segments {
            if settings.through_walls {
                gizmos.on_top.push(horizon_client::gizmos::Line {
                    start,
                    end,
                    color,
                    width: settings.line_width,
                });
            } else {
                gizmos.line(start, end, color, settings.line_width);
            }
        }
    }
}
