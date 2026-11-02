// Float-positioned skin faces: avoid integer GUI rounding that makes moving heads jump.
use crate::game::Game;
use horizon_client::hud::HudBatch;
pub(super) fn face(
    game: &Game<'_>,
    batch: &mut HudBatch,
    skin: &str,
    center: [f32; 2],
    size: f32,
    color: u32,
) {
    let mut draws = Vec::new();
    horizon_client::tab_list::face(&mut draws, skin, 0, 0, 8, true, false, color);
    let start = batch.vertices.len();
    horizon_client::hud_layout::paint(batch, &game.gui.atlas, &draws);
    for vertex in &mut batch.vertices[start..] {
        vertex.pos[0] = center[0] - size * 0.5 + vertex.pos[0] * size / 8.0;
        vertex.pos[1] = center[1] - size * 0.5 + vertex.pos[1] * size / 8.0;
    }
}
