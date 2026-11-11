fn claim_borders_player_look(x: f64, y: f64) {
    if x != 0.0 || y != 0.0 {
        crate::game::claim_borders_navigation_view::player_looked();
    }
}
