// Native HUD drawing through the existing atlas, with no additional GPU pipeline.
use crate::game::Game;
use crate::game::claim_borders_overlay as model;
use horizon_client::extras_hud::Corners;
use horizon_client::hud::{HudBatch, rgba};
use horizon_client::text::Text;

pub(super) fn draw(
    game: &mut Game<'_>,
    batch: &mut HudBatch,
    _corners: &mut Corners,
    _friends: &[model::FriendRow],
    markers: &[model::Marker],
) {
    if !game.native_on(crate::game::claim_borders::ID)
        || game.title.is_some()
        || game.ui.hide_gui
        || game.native_player().is_none()
    {
        return;
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
        let mut position = marker.position;
        if let Some(uuid) = marker.player
            && let Some(pos) = crate::game::claim_borders_client::entity_position(game, uuid)
        {
            position = [pos[0], pos[1] + 2.2, pos[2]];
        }
        let Some([x, y]) = model::project(camera, yaw, pitch, fov, viewport, position) else {
            continue;
        };
        if let Some(uuid) = marker.player {
            let alpha = (marker.alpha.clamp(0.15, 1.0) * 255.0) as u8;
            let x = (x - 6.0).clamp(1.0, (batch.width - 13.0).max(1.0));
            let y = (y - 6.0).clamp(1.0, (batch.height - 13.0).max(1.0));
            game.native_fill(
                batch,
                [x - 1.0, y - 1.0, 14.0, 14.0],
                ((alpha as u32) << 24) | 0x77c7ef,
            );
            let skin = crate::game::claim_borders_client::avatar(uuid);
            crate::game::claim_borders_avatar_draw::face(
                game,
                batch,
                &skin,
                [x + 6.0, y + 6.0],
                12.0,
                rgba(255, 255, 255, alpha),
            );
            let mouse = if game.screen == crate::game::Screen::Playing {
                [batch.width / 2.0, batch.height / 2.0]
            } else {
                game.native_cursor()
            };
            if (mouse[0] - (x + 6.0)).abs() < 8.0 && (mouse[1] - (y + 6.0)).abs() < 8.0 {
                let distance = crate::game::claim_borders_client::entity_position(game, uuid)
                    .and_then(|pos| {
                        game.native_player().map(|p| {
                            ((pos[0] - p.x).powi(2)
                                + (pos[1] - p.y).powi(2)
                                + (pos[2] - p.z).powi(2))
                            .sqrt()
                        })
                    })
                    .unwrap_or(marker.distance);
                let text = format!("{distance:.0} м");
                let width = game.native_text_width(&text) + 8.0;
                let left = (x + 6.0 - width / 2.0).clamp(1.0, (batch.width - width - 1.0).max(1.0));
                let top = (y + 15.0).min(batch.height - 13.0);
                game.native_fill(batch, [left, top, width, 12.0], 0xdd10202c);
                game.native_text(batch, &text, [left + 4.0, top + 2.0], 0xbce8d7, false);
            }
            continue;
        }
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
        let cx = icon_x + icon_w * 0.5;
        let cy = icon_y + icon_h * 0.5;
        batch.fill_quad(
            &game.gui.atlas,
            [
                [cx, cy - 3.0],
                [cx + 3.0, cy],
                [cx, cy + 3.0],
                [cx - 3.0, cy],
            ],
            color,
        );
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
