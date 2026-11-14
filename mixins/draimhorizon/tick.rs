fn claim_borders_tick(game: &mut crate::game::Game<'_>) {
    use crate::game::claim_borders_baritone_profile::time;
    time("mod tick: baritone bridge", || {
        crate::game::claim_borders_baritone_bridge::tick(game)
    });
    time("mod tick: workshop", || {
        crate::game::claim_borders_workshop_view::tick(game)
    });
    time("mod tick: construction", || {
        crate::game::claim_borders_construction_view::tick(game)
    });
    time("mod tick: navigation", || {
        crate::game::claim_borders_navigation_view::tick(game)
    });
    time("mod tick: client", || {
        crate::game::claim_borders_client::tick(game)
    });
    time("mod tick: ghosts", || {
        crate::game::claim_borders_schematic_ghosts::tick(game)
    });
    time("mod tick: verifier", || {
        crate::game::claim_borders_schematic_verify::tick(game)
    });
}
