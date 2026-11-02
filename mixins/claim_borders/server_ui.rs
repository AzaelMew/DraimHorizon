// Optional presentation of server-owned data. No invented balance or player positions.
use crate::game::{
    Game, Screen, claim_borders as claims, claim_borders_economy_client as data,
    claim_borders_economy_view::label,
};
use horizon_client::hud::HudBatch;
use std::sync::atomic::{AtomicUsize, Ordering};
static PAGE: AtomicUsize = AtomicUsize::new(0);
pub(super) fn page(game: &Game<'_>, key: &str) -> bool {
    if game.native_on(claims::ID)
        && game.screen == Screen::Playing
        && game.keys.contains(&winit::keyboard::KeyCode::Tab)
        && data::read(|_, p, _, _| p.flag("custom_tab"))
    {
        match key {
            "key.keyboard.page.up" => {
                let _ = PAGE.try_update(Ordering::Relaxed, Ordering::Relaxed, |p| {
                    Some(p.saturating_sub(1))
                });
                true
            }
            "key.keyboard.page.down" => {
                PAGE.fetch_add(1, Ordering::Relaxed);
                true
            }
            _ => false,
        }
    } else {
        false
    }
}
pub(super) fn sidebar(
    game: &Game<'_>,
    batch: &mut HudBatch,
    corners: &mut horizon_client::extras_hud::Corners,
) {
    if game.ui.hide_gui || game.native_player().is_none() {
        return;
    }
    data::read(|book, p, status, _| {
        if !p.flag("custom_sidebar") {
            return;
        }
        let mut lines: Vec<String> = status
            .sidebar
            .iter()
            .map(|(a, b)| format!("{a} {b}"))
            .collect();
        if lines.is_empty() {
            return;
        }
        if p.flag("sidebar_limits") && p.flag("enabled") {
            lines.push(format!(
                "Продать сегодня: {}",
                book.coin_left.map_or(
                    "?".into(),
                    |(n, _)| crate::game::claim_borders_economy::fmt(n)
                )
            ));
        }
        let scale = p.get("sidebar_scale") as f32 / 100.0;
        let width = (p.get("sidebar_width") as f32 * scale).min(batch.width - 8.0);
        let limit = (((batch.height - 60.0) / scale - 23.0) / 11.0).max(1.0) as usize;
        lines.truncate(limit);
        let height = (23.0 + lines.len() as f32 * 11.0) * scale;
        let (x, y) = corners.place(
            crate::game::claim_borders_map_view::corner(p.get("sidebar_corner") as u8),
            width,
            height,
        );
        let start = batch.vertices.len();
        game.native_fill(
            batch,
            [x / scale, y / scale, width / scale, height / scale],
            ((p.get("sidebar_opacity") as u32 * 255 / 100) << 24) | 0x101b27,
        );
        label(
            game,
            batch,
            "DraimHorizons",
            [x / scale + 6.0, y / scale + 5.0],
            width / scale - 12.0,
            0xe7c47b,
        );
        for (i, text) in lines.iter().enumerate() {
            label(
                game,
                batch,
                text,
                [x / scale + 6.0, y / scale + 19.0 + i as f32 * 11.0],
                width / scale - 12.0,
                0xc9d9e5,
            );
        }
        batch.scale_since(start, 0.0, 0.0, scale, scale);
    });
}
pub(super) fn tab(game: &mut Game<'_>, batch: &mut HudBatch) -> bool {
    if !game.native_on(claims::ID)
        || game.remote.is_none()
        || game.screen != Screen::Playing
        || !game.keys.contains(&winit::keyboard::KeyCode::Tab)
        || !data::read(|_, p, _, _| p.flag("custom_tab"))
    {
        return false;
    }
    let mut players = crate::game::scoreboard::claim_borders_economy_sidebar::prepare_tab(game);
    let friends = crate::game::claim_borders_client::friend_ids();
    let is_friend = |id| friends.iter().any(|(uuid, _)| *uuid == id);
    data::read(|book, p, _, _| {
        players.sort_by(|a, b| {
            let friend = if p.flag("tab_friends") {
                is_friend(b.uuid).cmp(&is_friend(a.uuid))
            } else {
                std::cmp::Ordering::Equal
            };
            friend.then_with(|| {
                a.name
                    .to_ascii_lowercase()
                    .cmp(&b.name.to_ascii_lowercase())
            })
        });
        let columns = (p.get("tab_columns") as usize)
            .min(((batch.width - 16.0) / 140.0).max(1.0) as usize)
            .max(1);
        let rows = (((batch.height - 76.0) / 16.0).floor() as usize).max(1);
        let per = columns * rows;
        let pages = players.len().div_ceil(per).max(1);
        let page = PAGE.load(Ordering::Relaxed).min(pages - 1);
        PAGE.store(page, Ordering::Relaxed);
        let width = (columns as f32 * 190.0).min(batch.width - 12.0);
        let cw = width / columns as f32;
        let shown = players.len().saturating_sub(page * per).min(per);
        let height = 50.0 + shown.div_ceil(columns) as f32 * 16.0;
        let x = (batch.width - width) / 2.0;
        let y = 8.0;
        game.native_fill(batch, [x, y, width, height], 0xee101b27);
        label(
            game,
            batch,
            &format!(
                "DraimHorizons · Онлайн {} · {}/{}",
                players.len(),
                page + 1,
                pages
            ),
            [x + 6.0, y + 5.0],
            width - 12.0,
            0xe7c47b,
        );
        label(
            game,
            batch,
            &format!(
                "Баланс {} · PgUp/PgDn — страницы",
                book.balance.map_or(
                    "?".into(),
                    |(n, _)| crate::game::claim_borders_economy::fmt(n)
                )
            ),
            [x + 6.0, y + 18.0],
            width - 12.0,
            0x94b3c9,
        );
        let me = game.native_player();
        for (i, player) in players.iter().skip(page * per).take(per).enumerate() {
            let col = i % columns;
            let row = i / columns;
            let left = x + col as f32 * cw + 5.0;
            let top = y + 34.0 + row as f32 * 16.0;
            let skin = game.face_skin(player.uuid, &player.skin);
            crate::game::claim_borders_avatar_draw::face(
                game,
                batch,
                &skin,
                [left + 5.0, top + 5.0],
                10.0,
                0xffffffff,
            );
            let mut suffix = String::new();
            if p.flag("tab_distance")
                && is_friend(player.uuid)
                && let (Some(me), Some(pos)) = (
                    &me,
                    crate::game::claim_borders_client::entity_position(game, player.uuid),
                )
            {
                let d =
                    ((me.x - pos[0]).powi(2) + (me.y - pos[1]).powi(2) + (me.z - pos[2]).powi(2))
                        .sqrt();
                suffix.push_str(&format!(" {d:.0}м"));
            }
            if p.flag("tab_ping") {
                suffix.push_str(&if player.latency < 0 {
                    " ?ms".into()
                } else {
                    format!(" {}ms", player.latency)
                });
            }
            let sw = game.native_text_width(&suffix).min(cw * 0.48);
            label(
                game,
                batch,
                &player.name,
                [left + 14.0, top + 1.0],
                cw - sw - 24.0,
                if is_friend(player.uuid) {
                    0x88dfb7
                } else {
                    0xc9d9e5
                },
            );
            label(
                game,
                batch,
                &suffix,
                [left + cw - sw - 8.0, top + 1.0],
                sw,
                0x94b3c9,
            );
        }
    });
    true
}
