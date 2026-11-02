// Native HUD drawing through the existing atlas, with no additional GPU pipeline.
use crate::game::Game;
use crate::game::claim_borders_overlay as model;
use horizon_client::extras::Corner;
use horizon_client::extras_hud::Corners;
use horizon_client::hud::{HudBatch, rgba};
use horizon_client::text::Text;

pub(super) fn draw(
    game: &mut Game<'_>,
    batch: &mut HudBatch,
    corners: &mut Corners,
    friends: &[model::FriendRow],
    markers: &[model::Marker],
) {
    if !game.native_on(crate::game::claim_borders::ID)
        || game.title.is_some()
        || game.ui.hide_gui
        || game.native_player().is_none()
    {
        return;
    }
    if game.native_corner_shown(Corner::TopLeft) && !friends.is_empty() {
        let displayed: Vec<String> = friends.iter().take(8).map(model::friend_label).collect();
        let mut labels = vec!["Party".to_owned()];
        labels.extend(displayed);
        if friends.len() > 8 {
            labels.push(format!("+{} online", friends.len() - 8));
        }
        let width = labels
            .iter()
            .map(|s| game.native_text_width(s))
            .fold(0.0, f32::max)
            + 6.0;
        let height = labels.len() as f32 * 10.0 + 4.0;
        let (x, y) = corners.place(Corner::TopLeft, width, height);
        let visible_height = height.min((batch.height - y - 2.0).max(0.0));
        if visible_height >= 14.0 {
            game.native_fill(batch, [x, y, width, visible_height], 0x8800_0000);
            for (i, label) in labels.iter().enumerate() {
                let line_y = y + 2.0 + i as f32 * 10.0;
                if line_y + 9.0 > y + visible_height {
                    break;
                }
                // Literal text: a player name cannot inject legacy formatting codes.
                batch.text_styled(
                    &game.gui.atlas,
                    &Text::literal(label),
                    x + 3.0,
                    line_y,
                    rgba(225, 245, 255, 255),
                    true,
                );
            }
        }
    }
    // This is the same previous-frame camera snapshot the vanilla locator bar uses.
    let Some((camera, yaw, pitch, fov)) = game.locator_camera else {
        return;
    };
    let viewport = [batch.width, batch.height];
    for marker in markers.iter().take(96) {
        if !marker.alpha.is_finite() || marker.alpha <= 0.0 {
            continue;
        }
        let Some([x, y]) = model::project(camera, yaw, pitch, fov, viewport, marker.position)
        else {
            continue;
        };
        let Some(distance) = model::distance_label(marker.distance) else {
            continue;
        };
        let alpha = (marker.alpha.clamp(0.0, 1.0) * 255.0).round() as u8;
        if alpha == 0 {
            continue;
        }
        let [_, r, g, b] = marker.color.to_be_bytes();
        let color = rgba(r, g, b, alpha);
        let Some(layout) = model::marker_layout(viewport, [x, y]) else {
            continue;
        };
        let [icon_x, icon_y, icon_w, icon_h] = layout.icon;
        batch.fill(&game.gui.atlas, icon_x, icon_y, icon_w, icon_h, color);
        let [left, top, width, height] = layout.bounds;
        let center_x = left + width / 2.0;
        for (label, line_y, line_color) in [
            (marker.label.as_str(), top, color),
            (distance.as_str(), top + 8.0, rgba(225, 225, 225, alpha)),
        ] {
            let label = model::marker_label(label, |s| game.native_text_width(s));
            let text_width = game.native_text_width(&label);
            let start = batch.vertices.len();
            batch.text_styled(
                &game.gui.atlas,
                &Text::literal(&label),
                center_x - text_width / 2.0,
                line_y,
                line_color,
                true,
            );
            batch.scale_since(
                start,
                center_x,
                line_y,
                model::MARKER_TEXT_SCALE,
                model::MARKER_TEXT_SCALE,
            );
            // Keep shadows and unusually large resource-pack glyphs inside
            // the same fixed screen-space box as the normal font.
            batch.clip_since(start, left, top, left + width, top + height);
        }
    }
}
