fn visibility(
    _value: (),
    game: &crate::game::Game<'_>,
    camera: glam::DVec3,
    alpha: f64,
    out: &mut Vec<horizon_client::mesher::Vertex>,
) {
    crate::game::claim_borders_visibility_view::render(game, camera, alpha, out);
}
